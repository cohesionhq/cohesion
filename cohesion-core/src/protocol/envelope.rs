//! The frames around commands: what a client sends, and every message the
//! server sends back. On a session, a client may also subscribe to
//! invalidations, and offer actors and answer the daemon's calls; a client
//! that does neither receives only replies.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

use super::capabilities::{
    ActorSessionUpdate, ActorUsageUpdate, ClientCall, ClientCapabilities, ClientOutcome,
    EvictionReason,
};
use super::command::Command;
use super::reply_data::ReplyData;
use super::server_error::ServerError;
use super::vocabulary::Vocabulary;
use crate::invalidation::InvalidationBatch;

/// Sent in `Hello`. Stays 1 during active development: do not bump it (`docs/design-principles.md`).
pub const PROTOCOL_VERSION: u32 = 1;

// Client -> Server

/// One command from a client; `id` correlates the reply on streaming transports.
#[derive(Serialize)]
pub struct Request {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(flatten)]
    pub command: Command,
}
/// By hand, so a command without arguments reads the same whether `args`
/// is absent, `null` or `{}`: every such command is a unit variant, and
/// serde reads a unit variant only from an absent `args`. An empty `args`
/// is still tried as sent when that fails, since `{}` is a whole request
/// for a command whose arguments are all optional (`ListCatalogModels`),
/// and that attempt's error is the one reported.
impl<'de> Deserialize<'de> for Request {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Wire {
            #[serde(default)]
            id: Option<u64>,
            #[serde(flatten)]
            command: Command,
        }
        let value = serde_json::Value::deserialize(deserializer)?;
        let command_name = value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let empty_args = value.get("args").is_some_and(|args| {
            args.is_null() || args.as_object().is_some_and(serde_json::Map::is_empty)
        });
        let wire = if empty_args {
            let mut without_args = value.clone();
            if let Some(object) = without_args.as_object_mut() {
                object.remove("args");
            }
            serde_json::from_value(without_args).or_else(|_| serde_json::from_value(value))
        } else {
            serde_json::from_value(value)
        };
        let Wire { id, command } = wire.map_err(|error| {
            // Serde answers an unknown command with every valid name; say
            // only which one it was. A nested unknown value in `args` names
            // some other variant and keeps serde's wording.
            match command_name {
                Some(name)
                    if error
                        .to_string()
                        .starts_with(&format!("unknown variant `{name}`")) =>
                {
                    D::Error::custom(format!("unknown command {name}"))
                }
                _ => D::Error::custom(error),
            }
        })?;
        Ok(Self { id, command })
    }
}

/// Every frame a client sends on a session. A `Request` is told apart by its
/// command's `type`; `subscribe` and a client's `hello`, `callResult`,
/// `actorSessionUpdate` and `actorUsage` are the session's own.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ClientFrame {
    Subscribe(Subscribe),
    Hello(ClientHello),
    CallResult(CallResult),
    ActorSessionUpdate(ActorSessionUpdate),
    ActorUsage(ActorUsageUpdate),
    #[serde(untagged)]
    Request(Box<Request>),
}
/// By hand, reading `type` first, so a malformed request reports what is
/// wrong with it rather than that no frame matched.
impl<'de> Deserialize<'de> for ClientFrame {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        let frame = match value.get("type").and_then(serde_json::Value::as_str) {
            Some("subscribe") => serde_json::from_value(value).map(Self::Subscribe),
            Some("hello") => serde_json::from_value(value).map(Self::Hello),
            Some("callResult") => serde_json::from_value(value).map(Self::CallResult),
            Some("actorSessionUpdate") => {
                serde_json::from_value(value).map(Self::ActorSessionUpdate)
            }
            Some("actorUsage") => serde_json::from_value(value).map(Self::ActorUsage),
            _ => serde_json::from_value(value).map(|request| Self::Request(Box::new(request))),
        };
        frame.map_err(D::Error::custom)
    }
}

/// Asks for the session's `invalidate` and `reset` frames, from now until it
/// ends; none are sent before. Subscribing again changes nothing.
#[derive(Serialize, Deserialize)]
pub struct Subscribe {
    /// Correlates the reply, `accepted`.
    pub id: u64,
}

/// Makes the session the one serving its client: what it offers, replacing
/// whatever it offered before. A client that serves actors sends it first,
/// and again whenever what it offers changes.
#[derive(Serialize, Deserialize)]
pub struct ClientHello {
    /// Correlates the reply: `accepted`, or why the offers were refused.
    pub id: u64,
    #[serde(flatten)]
    pub offers: ClientCapabilities,
}

/// A client's answer to the daemon's `Call` with `call_id`.
#[derive(Serialize, Deserialize)]
pub struct CallResult {
    pub call_id: u64,
    #[serde(flatten)]
    pub outcome: ClientOutcome,
}

/// Asks the server to trust the caller's certificate (sent to `/register`).
#[derive(Serialize, Deserialize)]
pub struct RegisterRequest {
    pub setup_key: String,
    pub client_name: String,
}

// Server -> client

/// Every server message, numbered in send order.
#[derive(Serialize, Deserialize)]
pub struct Frame {
    pub seq: u64,
    #[serde(flatten)]
    pub message: ServerMessage,
}

/// What a frame carries.
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ServerMessage {
    Hello(Hello),
    Reply(Reply),
    /// Data changed. Sent only to a session that subscribed.
    Invalidate(InvalidationBatch),
    /// Invalidations were dropped: refetch everything. Sent only to a
    /// session that subscribed.
    Reset,
    /// Asks a client to do something it offered; it answers with a
    /// `CallResult` carrying `call_id`. Sent only to a session that
    /// sent a `Hello`, so no other client ever sees one.
    Call {
        call_id: u64,
        call: ClientCall,
    },
    /// The last frame of a client's session the daemon ended, saying
    /// why: to any session once its client is revoked, and to one that
    /// sent a `Hello` once another replaces it.
    Evicted {
        reason: EvictionReason,
    },
}

/// The first frame of a session, identifying the server, and the client to
/// itself.
#[derive(Serialize, Deserialize)]
pub struct Hello {
    pub protocol: u32,
    pub server: String,
    /// The name the client is listed under, as its transport identified it.
    pub client_name: String,
    /// The [`WebBundle`](super::WebBundle)'s hash, for a client to check its
    /// cached copy against; `None` when the daemon was built without the UI.
    pub web_bundle_hash: Option<String>,
    /// How to word and order the daemon's kinds; boxed, as it is most of
    /// the frame's size.
    pub vocabulary: Box<Vocabulary>,
}

/// The answer to one [`Request`], carrying its `id`.
#[derive(Serialize, Deserialize)]
pub struct Reply {
    pub id: Option<u64>,
    #[serde(flatten)]
    pub outcome: Outcome,
}

/// Whether a command succeeded, with its data or error.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    Ok(ReplyData),
    Error(ServerError),
}

#[cfg(test)]
mod tests;
