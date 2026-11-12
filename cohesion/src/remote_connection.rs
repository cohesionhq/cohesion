use std::sync::Arc;
use std::time::{Duration, Instant};

use cohesion_core::{
    Host,
    protocol::{Command, Frame, Outcome, RegisterRequest, Request, ServerMessage},
};
use futures_util::{SinkExt, StreamExt};
use rustls::ClientConfig;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_tungstenite::{Connector, MaybeTlsStream, WebSocketStream, tungstenite::Message};

use crate::{
    client_identity::ClientIdentity, client_tls, connection::is_reply,
    connection_error::ConnectionError,
};

/// A WebSocket session with cohesiond over mutual TLS.
pub type RemoteSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// How long a session may sit unused and still be sent the next request;
/// an older one is closed and a new one opened.
const SESSION_REUSE: Duration = Duration::from_secs(60);

/// The session requests go over, kept between them so a command making
/// several calls pays for one TLS handshake, and when it was last used.
struct OpenSession {
    socket: RemoteSocket,
    used: Instant,
}

#[derive(Clone)]
pub struct RemoteConnection {
    http: reqwest::Client,
    /// The same TLS config offering only HTTP/1.1, which WebSocket upgrades need.
    ws_tls: Arc<ClientConfig>,
    /// `host:port`, as a URL names the server.
    authority: String,
    pub(crate) base_url: String,
    /// The session the next request reuses, if one is open; one request at
    /// a time goes over it.
    session: Arc<Mutex<Option<OpenSession>>>,
}

/// Why an exchange over a session ended without a reply.
enum Exchange {
    /// Nothing at all came back: the request may never have been read.
    Unanswered(ConnectionError),
    /// Something came back first, so the request may have been carried out.
    Failed(ConnectionError),
}

impl Exchange {
    fn into_error(self) -> ConnectionError {
        match self {
            Exchange::Unanswered(error) | Exchange::Failed(error) => error,
        }
    }
}

/// Sends `body` over `socket` and reads frames until the reply, skipping
/// the hello and any invalidations that come first.
async fn exchange(socket: &mut RemoteSocket, body: &str) -> Result<String, Exchange> {
    socket
        .send(Message::text(body))
        .await
        .map_err(|error| Exchange::Unanswered(ConnectionError::WebSocket(error)))?;
    let mut heard = false;
    while let Some(message) = socket.next().await {
        let message = match message {
            Ok(message) => message,
            Err(error) if heard => return Err(Exchange::Failed(ConnectionError::WebSocket(error))),
            Err(error) => return Err(Exchange::Unanswered(ConnectionError::WebSocket(error))),
        };
        let Message::Text(text) = message else {
            continue;
        };
        heard = true;
        if is_reply(&text) {
            return Ok(text.to_string());
        }
    }
    Err(match heard {
        true => Exchange::Failed(ConnectionError::UnexpectedResponse),
        false => Exchange::Unanswered(ConnectionError::UnexpectedResponse),
    })
}

impl RemoteConnection {
    pub fn new(
        host: &Host,
        port: u16,
        server_fingerprint: String,
        identity: ClientIdentity,
    ) -> Result<RemoteConnection, ConnectionError> {
        let tls = client_tls::client_config(identity, server_fingerprint)
            .map_err(ConnectionError::InvalidIdentity)?;
        let mut ws_tls = tls.clone();
        ws_tls.alpn_protocols = vec![b"http/1.1".to_vec()];
        let http = reqwest::Client::builder()
            .tls_backend_preconfigured(tls)
            .build()
            .map_err(ConnectionError::CannotBuildClient)?;
        let authority = host.with_port(port);
        Ok(Self {
            http,
            ws_tls: Arc::new(ws_tls),
            base_url: format!("https://{authority}"),
            authority,
            session: Arc::default(),
        })
    }

    pub async fn send(&self, command: Command) -> Result<Outcome, ConnectionError> {
        let request = Request { id: None, command };
        let body = serde_json::to_string(&request).expect("requests always serialize");
        let text = self.send_raw(body).await?;
        let frame: Frame =
            serde_json::from_str(&text).map_err(|_| ConnectionError::UnexpectedResponse)?;
        match frame.message {
            ServerMessage::Reply(reply) => Ok(reply.outcome),
            _ => Err(ConnectionError::UnexpectedResponse),
        }
    }

    /// Sends `body`, the JSON of a request, and returns the reply frame's
    /// JSON as sent. It goes over the session the last request used when
    /// that is recent, else over a new one, which is kept for the next.
    async fn send_raw(&self, body: String) -> Result<String, ConnectionError> {
        let mut session = self.session.lock().await;
        let kept = session
            .take()
            .filter(|open| open.used.elapsed() < SESSION_REUSE);
        if let Some(mut open) = kept {
            match exchange(&mut open.socket, &body).await {
                Ok(text) => {
                    open.used = Instant::now();
                    *session = Some(open);
                    return Ok(text);
                }
                // Nothing came back over the kept session, so the daemon
                // closed it (it restarted) before reading the request: send
                // it again over a new one.
                Err(Exchange::Unanswered(_)) => {}
                Err(Exchange::Failed(error)) => return Err(error),
            }
        }
        let mut socket = self.connect_ws().await?;
        let text = exchange(&mut socket, &body)
            .await
            .map_err(Exchange::into_error)?;
        *session = Some(OpenSession {
            socket,
            used: Instant::now(),
        });
        Ok(text)
    }

    /// Opens a session on cohesiond's `/connect` WebSocket.
    pub async fn connect_ws(&self) -> Result<RemoteSocket, ConnectionError> {
        let connector = Connector::Rustls(self.ws_tls.clone());
        let (socket, _) = tokio_tungstenite::connect_async_tls_with_config(
            format!("wss://{}/connect", self.authority),
            None,
            false,
            Some(connector),
        )
        .await
        .map_err(ConnectionError::WebSocket)?;
        Ok(socket)
    }

    /// Asks cohesiond to trust this connection's certificate. Plain HTTP: the
    /// session routes refuse a client until it is registered.
    pub async fn register(&self, request: RegisterRequest) -> Result<Outcome, ConnectionError> {
        let response = self
            .http
            .post(format!("{}/register", self.base_url))
            .json(&request)
            .send()
            .await
            .map_err(ConnectionError::RequestFailed)?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ConnectionError::Refused(
                "the setup code wasn't accepted. Run `cohesion pair` on the server for a new one."
                    .to_string(),
            ));
        }
        let frame = response
            .error_for_status()
            .map_err(ConnectionError::RequestFailed)?
            .json::<Frame>()
            .await
            .map_err(|_| ConnectionError::UnexpectedResponse)?;

        match frame.message {
            ServerMessage::Reply(reply) => Ok(reply.outcome),
            _ => Err(ConnectionError::UnexpectedResponse),
        }
    }
}
