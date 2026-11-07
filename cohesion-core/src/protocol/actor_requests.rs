//! Requests about actors, the agents on clients' devices, and the sessions
//! Cohesion hands them.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

use super::{ActorUsage, ImageUpload};
use crate::model::ActorUserContent;

/// Lists the actors the clients offer, by client then product.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListActorsRequest;

/// Lists how much of its limits each usage pool has used, as the clients
/// connected last reported: one entry an account the actors share, and one
/// for each actor whose limits are its own. An actor whose client never
/// said is left out.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListActorUsageRequest;

/// How much of its limits one pool has used: the latest report among
/// actors `actor_ids`, which draw on it, least id first. `pool` is left out
/// for a single actor whose limits are its own.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TsWire)]
pub struct ListedActorUsage {
    pub actor_ids: Vec<i64>,
    #[serde(flatten)]
    pub usage: ActorUsage,
}

/// Lists actor sessions, latest status change first: actor `actor_id`'s, or
/// every actor's.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct ListActorSessionsRequest {
    pub actor_id: Option<i64>,
}

/// Searches every actor session's title and what the user and the actor said
/// in it, best match first. `query` is FTS5 syntax.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SearchActorSessionsRequest {
    pub query: String,
}

/// Fetches actor session `id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct GetActorSessionRequest {
    pub id: i64,
}

/// Marks actor session `session_id` read, now; the next stretch of work its actor
/// finishes marks it unread again.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct MarkActorSessionReadRequest {
    pub session_id: i64,
}

/// Lists what was said in actor session `session_id`, oldest first.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListActorSessionMessagesRequest {
    pub session_id: i64,
}

/// Hands actor `actor_id` a new session, `prompt`, with any images in
/// `attachments`, in the mode, on the model and at the effort given (the
/// actor's defaults when left out), titled `title`, or by the prompt's
/// first line when left out. Its client must be online.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct StartActorSessionRequest {
    pub actor_id: i64,
    pub prompt: String,
    pub title: Option<String>,
    pub mode: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub attachments: Option<Vec<ImageUpload>>,
}

/// Says `content` in actor session `session_id`, with any images in
/// `attachments`: more to do, which starts a session that is done or failed
/// again, or a response to one of its requests. Its client must be online.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct SendActorSessionMessageRequest {
    pub session_id: i64,
    pub content: ActorUserContent,
    pub attachments: Option<Vec<ImageUpload>>,
}

/// Lists the images attached to actor session message `message_id`, in the
/// order sent.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListActorSessionMessageAttachmentsRequest {
    pub message_id: i64,
}

/// Stops what actor session `session_id` is doing, telling its actor when its
/// client is online; the session is idle, until the user says more.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct StopActorSessionRequest {
    pub session_id: i64,
}

/// Resolves actor session `session_id`: the user is done with it. One going on is
/// stopped first, telling its actor when its client is online. Lists leave
/// it out from then on, until the user says more, which starts it again.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ResolveActorSessionRequest {
    pub session_id: i64,
}

/// Switches actor session `session_id` to mode `mode`, one its actor offers, from
/// its next step on, even while it works. Its client must be online.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetActorSessionModeRequest {
    pub session_id: i64,
    pub mode: String,
}
