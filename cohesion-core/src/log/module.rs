//! [`Module`], the subsystem every log line names.

use std::fmt;

/// The subsystem a log line comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Module {
    Server,
    Http,
    WebSocket,
    UnixSocket,
    Database,
    Script,
    Cli,
    Intelligence,
    Extensions,
    Feeds,
    Actors,
}

impl Module {
    /// Lowercase name used in log lines, e.g. `"http"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Server => "server",
            Self::Http => "http",
            Self::WebSocket => "websocket",
            Self::UnixSocket => "unixsocket",
            Self::Database => "database",
            Self::Script => "script",
            Self::Cli => "cli",
            Self::Intelligence => "intelligence",
            Self::Extensions => "extensions",
            Self::Feeds => "feeds",
            Self::Actors => "actors",
        }
    }
}

impl fmt::Display for Module {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}
