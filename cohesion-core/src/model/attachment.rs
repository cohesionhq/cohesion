//! An image that came with a message, in one table whichever conversation the
//! message is in.

use cohesion_macros::{ColumnGroup, Model, TsWire};
use serde::{Deserialize, Serialize};

use super::ImageBytes;

/// An image with a message: one the user attached, or one an agent or actor
/// showed.
#[derive(Clone, Debug, Serialize, Deserialize, Model, TsWire)]
#[model(table = "attachments", backup = LeftOut)]
pub struct Attachment {
    pub id: i64,
    /// The message it came with.
    #[model(group)]
    pub message: AttachmentMessage,
    /// `image/png`, `image/jpeg`, `image/gif` or `image/webp`.
    pub media_type: String,
    /// The image, base64-encoded.
    pub data: ImageBytes,
}

/// The message an attachment came with, by which conversation it is in. Real
/// foreign keys, so a message's attachments cascade with it.
///
/// - `"actorSession"`: a message in an actor session.
/// - `"agent"`: a message in an agent's conversation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ColumnGroup, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AttachmentMessage {
    ActorSession { actor_session_message_id: i64 },
    Agent { agent_message_id: i64 },
}
