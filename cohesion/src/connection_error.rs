use std::{error::Error, fmt, io};

#[derive(Debug)]
pub enum ConnectionError {
    InvalidIdentity(rustls::Error),
    CannotBuildClient(reqwest::Error),
    RequestFailed(reqwest::Error),
    Socket(io::Error),
    WebSocket(tokio_tungstenite::tungstenite::Error),
    UnexpectedResponse,
    /// cohesiond refused the socket's handshake, saying why.
    Refused(String),
}

impl fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentity(err) => write!(f, "unusable client certificate: {err}"),
            Self::CannotBuildClient(err) => write!(f, "cannot build HTTP client: {err}"),
            Self::RequestFailed(err) => {
                // reqwest's own message is generic; the cause (e.g. a TLS alert) is in the chain.
                write!(f, "request failed: {err}")?;
                let mut source = err.source();
                while let Some(cause) = source {
                    write!(f, ": {cause}")?;
                    source = cause.source();
                }
                Ok(())
            }
            Self::Socket(err) => write!(f, "socket error: {err}"),
            Self::WebSocket(err) => write!(f, "WebSocket error: {err}"),
            Self::UnexpectedResponse => write!(f, "unexpected response from server"),
            Self::Refused(reason) => write!(f, "refused: {reason}"),
        }
    }
}
