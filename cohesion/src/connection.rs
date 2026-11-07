use cohesion_core::{
    invalidation::InvalidationBatch,
    protocol::{ClientFrame, Command, Frame, Outcome, ReplyData, ServerMessage, Subscribe},
};
use futures_util::{SinkExt, StreamExt};
#[cfg(unix)]
use tokio::{
    io::{BufReader, Lines},
    net::unix::{OwnedReadHalf, OwnedWriteHalf},
};
use tokio_tungstenite::tungstenite::Message;

use crate::{
    connection_error::ConnectionError,
    local_connection::LocalConnection,
    remote_connection::{RemoteConnection, RemoteSocket},
};

#[derive(Clone)]
pub enum Connection {
    Remote(RemoteConnection),
    Local(LocalConnection),
}

impl Connection {
    /// Where cohesiond is expected, for error messages.
    pub fn target(&self) -> String {
        match self {
            Self::Remote(remote) => remote.base_url.clone(),
            Self::Local(local) => local.socket.display().to_string(),
        }
    }

    pub async fn send(&self, command: Command) -> Result<Outcome, ConnectionError> {
        match self {
            Self::Remote(remote) => remote.send(command).await,
            Self::Local(local) => local.send(command).await,
        }
    }

    /// Opens a session and subscribes it to invalidations, answering once the daemon has
    /// accepted, so nothing published afterwards is missed. It is the only way for the CLI
    /// to wait on a change: a session pushes nothing to a client that hasn't subscribed.
    pub async fn subscribe(&self) -> Result<Invalidations, ConnectionError> {
        // The only request on this session, so any id correlates its reply.
        const SUBSCRIBE_ID: u64 = 1;
        let mut upstream = self.connect_upstream().await?;
        let frame = serde_json::to_string(&ClientFrame::Subscribe(Subscribe { id: SUBSCRIBE_ID }))
            .expect("frames always serialize");
        upstream.send(frame).await?;
        while let Some(text) = upstream.recv().await {
            // The hello comes before the reply.
            let Ok(Frame {
                message: ServerMessage::Reply(reply),
                ..
            }) = serde_json::from_str(&text)
            else {
                continue;
            };
            return match reply.outcome {
                Outcome::Ok(ReplyData::Accepted(_)) if reply.id == Some(SUBSCRIBE_ID) => {
                    Ok(Invalidations { upstream })
                }
                _ => Err(ConnectionError::UnexpectedResponse),
            };
        }
        Err(ConnectionError::UnexpectedResponse)
    }

    /// Opens a long-lived session that streams frames as text, for relaying them
    /// untouched. Waiting on invalidations goes through [`Self::subscribe`].
    pub(crate) async fn connect_upstream(&self) -> Result<Upstream, ConnectionError> {
        match self {
            Self::Remote(remote) => remote
                .connect_ws()
                .await
                .map(|ws| Upstream::Ws(Box::new(ws))),
            #[cfg(unix)]
            Self::Local(local) => {
                let (lines, write) = local.connect_session().await?;
                Ok(Upstream::Socket { lines, write })
            }
            #[cfg(not(unix))]
            Self::Local(_) => Err(crate::local_connection::unsupported()),
        }
    }
}

/// A session with cohesiond: requests go out and frames come back as JSON text, one
/// message each, whichever transport carries them.
pub(crate) enum Upstream {
    Ws(Box<RemoteSocket>),
    #[cfg(unix)]
    Socket {
        lines: Lines<BufReader<OwnedReadHalf>>,
        write: OwnedWriteHalf,
    },
}

impl Upstream {
    pub(crate) async fn send(&mut self, text: String) -> Result<(), ConnectionError> {
        match self {
            Self::Ws(socket) => socket
                .send(Message::text(text))
                .await
                .map_err(ConnectionError::WebSocket),
            #[cfg(unix)]
            Self::Socket { write, .. } => crate::local_connection::write_line(write, text).await,
        }
    }

    /// The next frame, or `None` once the session has ended.
    pub(crate) async fn recv(&mut self) -> Option<String> {
        match self {
            Self::Ws(socket) => loop {
                match socket.next().await? {
                    Ok(Message::Text(text)) => return Some(text.to_string()),
                    Ok(Message::Close(_)) | Err(_) => return None,
                    Ok(_) => continue,
                }
            },
            #[cfg(unix)]
            Self::Socket { lines, .. } => lines.next_line().await.ok().flatten(),
        }
    }
}

/// What a subscribed session pushes.
pub enum Received {
    /// Data changed.
    Batch(InvalidationBatch),
    /// The daemon dropped invalidations this session was too slow to take: refetch
    /// whatever is being followed.
    Reset,
}

/// A session subscribed to invalidations, from [`Connection::subscribe`].
pub struct Invalidations {
    upstream: Upstream,
}

impl Invalidations {
    /// The next batch of changes, or a `Reset`; `None` once the session has ended. Other
    /// frames, and any this client doesn't know, are skipped.
    pub async fn next(&mut self) -> Option<Received> {
        loop {
            let text = self.upstream.recv().await?;
            match serde_json::from_str::<Frame>(&text).map(|frame| frame.message) {
                Ok(ServerMessage::Invalidate(batch)) => return Some(Received::Batch(batch)),
                Ok(ServerMessage::Reset) => return Some(Received::Reset),
                _ => {}
            }
        }
    }
}

/// Whether `text` is a reply frame, read by its `type` alone, so a frame this
/// client doesn't know (an invalidation kind newer than it) is skipped rather
/// than failing the request.
pub fn is_reply(text: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(text).is_ok_and(|frame| frame["type"] == "reply")
}
