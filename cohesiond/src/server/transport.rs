//! What a session needs from a connection: an [`Exchange`] that splits into
//! a half that receives text frames and a half that sends them, so a session
//! keeps reading while a send waits on a full socket, and, for a transport
//! that can't tell a peer has gone, pings to find out.

use std::time::Duration;

use cohesionkit::protocol::{ClientFrame, Request};
use serde::de::DeserializeOwned;

pub enum ExchangeError {
    ConnectionError(String),
}

pub type ExchangeResult<T> = Result<T, ExchangeError>;

pub trait Exchange {
    /// How often to ping a peer this transport can't tell is gone, such as a
    /// sleeping Mac at the far end of a half-open WebSocket. A client session
    /// that hears nothing, not even a pong, for twice this ends as dropped.
    /// `None` for a transport that reports a dropped peer itself.
    const KEEPALIVE: Option<Duration> = None;
    type Reader: ExchangeReader;
    type Writer: ExchangeWriter;
    fn split(self) -> (Self::Reader, Self::Writer);
}

/// The receiving half of an [`Exchange`].
pub trait ExchangeReader: Send + 'static {
    /// What the peer sent next; `None` once it has hung up.
    fn recv(&mut self) -> impl Future<Output = ExchangeResult<Option<Incoming>>> + Send;
}

/// The sending half of an [`Exchange`].
pub trait ExchangeWriter: Send + 'static {
    fn send(&mut self, frame: String) -> impl Future<Output = ExchangeResult<()>> + Send;

    /// Asks the peer for a pong. Sent only when the exchange has a
    /// [`KEEPALIVE`](Exchange::KEEPALIVE).
    fn ping(&mut self) -> impl Future<Output = ExchangeResult<()>> + Send {
        async { Ok(()) }
    }
}

/// What a reader reads.
pub enum Incoming {
    Frame(ReceivableMessage),
    /// The peer answered a ping: it is still there.
    Pong,
}

/// A frame that could not be read: the `id` when the frame is valid JSON
/// that carries one, so the error can be correlated, and why.
pub type BadFrame = (Option<u64>, String);

pub struct ReceivableMessage(pub(crate) String);
impl ReceivableMessage {
    /// The frame a session reads, and its wire `type` (for logs; a request's
    /// is its command's).
    pub(super) fn frame(self) -> Result<(ClientFrame, String), BadFrame> {
        self.parse()
    }

    /// A request alone, as a one-off HTTP command carries it; the other
    /// frames need a session.
    pub(super) fn request(self) -> Result<(Request, String), BadFrame> {
        self.parse()
    }

    fn parse<T: DeserializeOwned>(self) -> Result<(T, String), BadFrame> {
        let value: serde_json::Value =
            serde_json::from_str(&self.0).map_err(|e| (None, e.to_string()))?;
        let id = value.get("id").and_then(serde_json::Value::as_u64);
        let kind = value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let parsed = serde_json::from_value(value).map_err(|e| (id, e.to_string()))?;
        Ok((parsed, kind))
    }
}
