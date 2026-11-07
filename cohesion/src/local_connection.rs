#[cfg(not(unix))]
use std::io;
use std::path::PathBuf;

use cohesion_core::LocalIdentity;
use cohesion_core::protocol::{Command, Outcome};
#[cfg(unix)]
use cohesion_core::{
    LocalHandshakeReply,
    protocol::{Frame, Request, ServerMessage},
};
#[cfg(unix)]
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    net::{
        UnixStream,
        unix::{OwnedReadHalf, OwnedWriteHalf},
    },
};

#[cfg(unix)]
use crate::connection::is_reply;
use crate::connection_error::ConnectionError;

/// Talks to cohesiond over its Unix socket: newline-delimited JSON frames, one
/// connection per request, each identified as `identity` before its session.
#[derive(Clone)]
pub struct LocalConnection {
    pub(crate) socket: PathBuf,
    identity: LocalIdentity,
}

/// A session's lines as they arrive, and where to write it frames.
#[cfg(unix)]
pub type SocketSession = (Lines<BufReader<OwnedReadHalf>>, OwnedWriteHalf);

impl LocalConnection {
    pub fn new(socket: PathBuf, identity: LocalIdentity) -> Self {
        Self { socket, identity }
    }

    #[cfg(unix)]
    pub async fn send(&self, command: Command) -> Result<Outcome, ConnectionError> {
        let request = Request { id: None, command };
        let body = serde_json::to_string(&request).expect("requests always serialize");
        let line = self.send_raw(body).await?;
        let frame: Frame =
            serde_json::from_str(&line).map_err(|_| ConnectionError::UnexpectedResponse)?;
        match frame.message {
            ServerMessage::Reply(reply) => Ok(reply.outcome),
            _ => Err(ConnectionError::UnexpectedResponse),
        }
    }

    /// Sends `body`, the JSON of a request, and returns the reply frame's line as sent.
    #[cfg(unix)]
    async fn send_raw(&self, body: String) -> Result<String, ConnectionError> {
        let (mut lines, mut write) = self.connect_session().await?;
        write_line(&mut write, body).await?;
        // The hello comes before the reply.
        while let Some(line) = lines.next_line().await.map_err(ConnectionError::Socket)? {
            if is_reply(&line) {
                return Ok(line);
            }
        }
        Err(ConnectionError::UnexpectedResponse)
    }

    /// Opens cohesiond's socket and identifies this client, then hands back
    /// the session that follows. The handshake is the socket's own: nothing
    /// past this sees it.
    #[cfg(unix)]
    pub async fn connect_session(&self) -> Result<SocketSession, ConnectionError> {
        let stream = UnixStream::connect(&self.socket)
            .await
            .map_err(ConnectionError::Socket)?;
        let (read, mut write) = stream.into_split();
        let identity = serde_json::to_string(&self.identity).expect("identities always serialize");
        write_line(&mut write, identity).await?;
        let mut lines = BufReader::new(read).lines();
        let reply = lines
            .next_line()
            .await
            .map_err(ConnectionError::Socket)?
            .ok_or(ConnectionError::UnexpectedResponse)?;
        match serde_json::from_str(&reply).map_err(|_| ConnectionError::UnexpectedResponse)? {
            LocalHandshakeReply::Accepted => Ok((lines, write)),
            LocalHandshakeReply::Refused { reason } => Err(ConnectionError::Refused(reason)),
        }
    }

    #[cfg(not(unix))]
    pub async fn send(&self, _command: Command) -> Result<Outcome, ConnectionError> {
        Err(unsupported())
    }
}

/// Writes `json` as one line.
#[cfg(unix)]
pub async fn write_line(write: &mut OwnedWriteHalf, json: String) -> Result<(), ConnectionError> {
    write
        .write_all(as_line(json).as_bytes())
        .await
        .map_err(ConnectionError::Socket)
}

/// `json` as one newline-terminated line. Raw newlines in JSON are only ever
/// whitespace between tokens (inside strings they are escaped), so they become spaces.
#[cfg(unix)]
pub fn as_line(json: String) -> String {
    let mut line = json.replace(['\r', '\n'], " ");
    line.push('\n');
    line
}

#[cfg(not(unix))]
pub fn unsupported() -> ConnectionError {
    ConnectionError::Socket(io::Error::new(
        io::ErrorKind::Unsupported,
        "local connections need Unix sockets",
    ))
}
