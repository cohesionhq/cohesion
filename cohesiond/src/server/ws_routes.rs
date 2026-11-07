//! The `/connect` route: upgrades to a WebSocket and runs a session over it.

use std::time::Duration;

use axum::Extension;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use cohesionkit::{Caller, Client, CohesionContext, Transport};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};

use super::serve::BODY_LIMIT;
use super::session::Session;
use super::transport::{
    Exchange, ExchangeError, ExchangeReader, ExchangeResult, ExchangeWriter, Incoming,
    ReceivableMessage,
};

/// How often a session pings its WebSocket peer. A sleeping Mac leaves its
/// socket half-open, which only a missing pong reveals.
const KEEPALIVE: Duration = Duration::from_secs(30);

pub async fn handle_ws(
    State(context): State<CohesionContext>,
    Extension(caller): Extension<Caller>,
    Extension(client): Extension<Client>,
    upgrade: WebSocketUpgrade,
) -> Response {
    let caller = Caller {
        transport: Transport::WebSocket,
        ..caller
    };
    // A browser sends a message as one frame, so a frame may be as large.
    let upgrade = upgrade
        .max_message_size(BODY_LIMIT)
        .max_frame_size(BODY_LIMIT);
    upgrade.on_upgrade(move |socket| {
        let ws = WebSocketExchange { socket };
        let session = Session::new(ws, &context, caller, client);
        session.attach()
    })
}

struct WebSocketExchange {
    socket: WebSocket,
}
impl Exchange for WebSocketExchange {
    const KEEPALIVE: Option<Duration> = Some(KEEPALIVE);
    type Reader = SplitStream<WebSocket>;
    type Writer = SplitSink<WebSocket, Message>;
    fn split(self) -> (Self::Reader, Self::Writer) {
        let (sink, stream) = self.socket.split();
        (stream, sink)
    }
}

impl ExchangeReader for SplitStream<WebSocket> {
    async fn recv(&mut self) -> ExchangeResult<Option<Incoming>> {
        loop {
            match self.next().await {
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => return Ok(None),
                Some(Ok(Message::Text(msg))) => {
                    return Ok(Some(Incoming::Frame(ReceivableMessage(msg.to_string()))));
                }
                Some(Ok(Message::Pong(_))) => return Ok(Some(Incoming::Pong)),
                Some(Ok(_)) => continue,
            }
        }
    }
}

impl ExchangeWriter for SplitSink<WebSocket, Message> {
    async fn send(&mut self, frame: String) -> ExchangeResult<()> {
        SinkExt::send(self, Message::text(frame))
            .await
            .map_err(|e| ExchangeError::ConnectionError(e.to_string()))
    }

    async fn ping(&mut self) -> ExchangeResult<()> {
        SinkExt::send(self, Message::Ping(Default::default()))
            .await
            .map_err(|e| ExchangeError::ConnectionError(e.to_string()))
    }
}
