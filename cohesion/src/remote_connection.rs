use std::sync::Arc;

use cohesion_core::{
    Host,
    protocol::{Command, Frame, Outcome, RegisterRequest, Request, ServerMessage},
};
use futures_util::{SinkExt, StreamExt};
use rustls::ClientConfig;
use tokio::net::TcpStream;
use tokio_tungstenite::{Connector, MaybeTlsStream, WebSocketStream, tungstenite::Message};

use crate::{
    client_identity::ClientIdentity, client_tls, connection::is_reply,
    connection_error::ConnectionError,
};

/// A WebSocket session with cohesiond over mutual TLS.
pub type RemoteSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Clone)]
pub struct RemoteConnection {
    http: reqwest::Client,
    /// The same TLS config offering only HTTP/1.1, which WebSocket upgrades need.
    ws_tls: Arc<ClientConfig>,
    /// `host:port`, as a URL names the server.
    authority: String,
    pub(crate) base_url: String,
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

    /// Sends `body`, the JSON of a request, over a fresh session and returns the reply
    /// frame's JSON as sent.
    async fn send_raw(&self, body: String) -> Result<String, ConnectionError> {
        let mut socket = self.connect_ws().await?;
        socket
            .send(Message::text(body))
            .await
            .map_err(ConnectionError::WebSocket)?;

        // The hello and any invalidations can arrive before the reply.
        while let Some(message) = socket.next().await {
            let Message::Text(text) = message.map_err(ConnectionError::WebSocket)? else {
                continue;
            };
            if is_reply(&text) {
                // Best effort: the reply is in hand whether or not the close handshake lands.
                let _ = socket.close(None).await;
                return Ok(text.to_string());
            }
        }
        Err(ConnectionError::UnexpectedResponse)
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
