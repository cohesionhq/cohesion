//! The protocol as a client reads and writes it: the frames around commands,
//! with only the messages and replies a client acts on decoded. Anything
//! else stays raw JSON, so a client keeps working when the daemon adds a
//! message, a reply or an error code, and none of the daemon's own types
//! need to be here. The daemon's full protocol is `cohesion_core::protocol`,
//! and its tests check that its frames decode into these.

use base64::{DecodeError, Engine, engine::general_purpose::STANDARD};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// Sent in `Hello`. Stays 1 during active development: do not bump it (`docs/design-principles.md`).
pub const PROTOCOL_VERSION: u32 = 1;

// Client -> server

/// Asks the server to trust the caller's certificate (sent to `/register`).
#[derive(Serialize, Deserialize)]
pub struct RegisterRequest {
    pub setup_key: String,
    pub client_name: String,
}

/// One command from a client; `id` correlates the reply.
#[derive(Serialize)]
pub struct Request {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(flatten)]
    pub command: Command,
}

/// The commands a client sends through this crate.
#[derive(Serialize)]
#[serde(tag = "type", content = "args", rename_all = "camelCase")]
pub enum Command {
    /// Asks for the web UI the daemon was built with; answered with
    /// [`ReplyData::WebBundle`].
    WebBundle,
}

/// Asks for the web UI the daemon was built with.
pub struct WebBundleRequest;

impl From<WebBundleRequest> for Command {
    fn from(_: WebBundleRequest) -> Self {
        Self::WebBundle
    }
}

// Server -> client

/// Every server message, numbered in send order.
pub struct Frame {
    pub seq: u64,
    pub message: ServerMessage,
}

/// What a frame carries.
pub enum ServerMessage {
    Hello(Hello),
    Reply(Reply),
    /// Any other message (an invalidation, a call, an eviction): its JSON,
    /// `type` included.
    Other(Value),
}

/// The first frame of a session, identifying the server, and the client to
/// itself.
#[derive(Deserialize)]
pub struct Hello {
    pub protocol: u32,
    pub server: String,
    /// The name the client is listed under, as its transport identified it.
    pub client_name: String,
    /// The [`WebBundle`]'s hash, for a client to check its cached copy
    /// against; `None` when the daemon was built without the UI.
    pub web_bundle_hash: Option<String>,
}

/// The answer to one [`Request`], carrying its `id`.
#[derive(Deserialize)]
pub struct Reply {
    pub id: Option<u64>,
    #[serde(flatten)]
    pub outcome: Outcome,
}

/// Whether a command succeeded, with its data or error.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    Ok(ReplyData),
    Error(ServerError),
}

/// The payload of a successful reply.
pub enum ReplyData {
    WebBundle(WebBundle),
    /// Any other reply: its `type`, and its `data` (`null` when it has none).
    Other {
        kind: String,
        data: Value,
    },
}

/// Why a command failed; `field` names the offending input when there is one.
#[derive(Deserialize)]
pub struct ServerError {
    /// The kind of failure, such as `badRequest` or `notFound`, for clients
    /// that branch on it.
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub field: Option<String>,
}

/// The web UI's static build, for a client to serve.
#[derive(Deserialize)]
pub struct WebBundle {
    /// Identifies this build: any change to the files changes it, so a client
    /// may keep what it unpacked for as long as the hash is unchanged.
    pub hash: String,
    pub files: Vec<WebFile>,
}

/// One file of the web UI's build.
#[derive(Deserialize)]
pub struct WebFile {
    /// Relative to the build's root, with `/` separators, e.g. `_app/version.json`.
    pub path: String,
    /// The file's bytes, compressed as raw deflate (RFC 1951, no zlib or gzip
    /// wrapper), base64-encoded.
    pub data: String,
}

impl WebFile {
    /// The file's raw-deflated bytes.
    pub fn deflated(&self) -> Result<Vec<u8>, DecodeError> {
        STANDARD.decode(&self.data)
    }
}

/// By hand, so a frame whose message this crate doesn't know still decodes.
impl<'de> Deserialize<'de> for Frame {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut value = Value::deserialize(deserializer)?;
        let seq = value
            .as_object_mut()
            .and_then(|object| object.remove("seq"))
            .ok_or_else(|| D::Error::missing_field("seq"))?;
        let seq = u64::deserialize(seq).map_err(D::Error::custom)?;
        let message = match value.get("type").and_then(Value::as_str) {
            Some("hello") => {
                ServerMessage::Hello(Hello::deserialize(value).map_err(D::Error::custom)?)
            }
            Some("reply") => {
                ServerMessage::Reply(Reply::deserialize(value).map_err(D::Error::custom)?)
            }
            Some(_) => ServerMessage::Other(value),
            None => return Err(D::Error::missing_field("type")),
        };
        Ok(Self { seq, message })
    }
}

/// By hand, so a reply this crate doesn't know still decodes.
impl<'de> Deserialize<'de> for ReplyData {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut value = Value::deserialize(deserializer)?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| D::Error::custom("a reply's data is an object"))?;
        let kind = match object.remove("type") {
            Some(Value::String(kind)) => kind,
            _ => return Err(D::Error::missing_field("type")),
        };
        let data = object.remove("data").unwrap_or(Value::Null);
        Ok(match kind.as_str() {
            "webBundle" => Self::WebBundle(WebBundle::deserialize(data).map_err(D::Error::custom)?),
            _ => Self::Other { kind, data },
        })
    }
}

#[cfg(test)]
mod tests;
