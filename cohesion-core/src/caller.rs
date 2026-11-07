//! Who a request comes from, so commands and logs can say whose action it was.

use std::fmt;

use serde::{Deserialize, Serialize};

/// The origin of a request: how it arrived and, once known, which client sent it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Caller {
    pub transport: Transport,
    /// Registered client, once known.
    pub client: Option<CallerClient>,
    /// Remote address for network transports, e.g. "10.0.0.5:51234".
    pub address: Option<String>,
}

/// The registered client behind a [`Caller`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallerClient {
    pub id: i64,
    pub name: String,
}

/// How a request reached cohesiond.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Transport {
    Http,
    WebSocket,
    UnixSocket,
    Internal,
}

impl fmt::Display for Transport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Http => "http",
            Self::WebSocket => "websocket",
            Self::UnixSocket => "unix socket",
            Self::Internal => "internal",
        })
    }
}

/// `"<name or 'unregistered'> via <transport>[ at <address>]"`.
impl fmt::Display for Caller {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.client {
            Some(client) => write!(f, "{}", client.name)?,
            None => f.write_str("unregistered")?,
        }
        write!(f, " via {}", self.transport)?;
        if let Some(address) = &self.address {
            write!(f, " at {address}")?;
        }
        Ok(())
    }
}
