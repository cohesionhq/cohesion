//! What a client offers and what the daemon asks of it: the payloads
//! of `Hello`, `Call`, `CallResult`, `ActorSessionUpdate` and `ActorUsage`
//! frames.

use std::time::Duration;

use cohesion_macros::{TsInterface, TsWire};
use serde::{Deserialize, Serialize};

use crate::model::{
    ActorDefinition, ActorMessageContent, ActorUserContent, ContextUsage,
    ReportedActorSessionStatus,
};

/// Everything a client offers, as its `Hello` frame lists it: its actors,
/// each identifier unique among them.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, TsWire)]
pub struct ClientCapabilities {
    /// Agents on the client's device it hands sessions to, each answering
    /// `StartActorSession`, `SendActorSessionMessage`, `StopActorSession` and
    /// `SetActorSessionMode` calls and reporting each session's progress in
    /// `ActorSessionUpdate` frames.
    /// Left out, none.
    #[serde(default)]
    pub actors: Vec<ActorDefinition>,
}

/// What the daemon asks a client to do.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ClientCall {
    /// Start session `session_id` on actor `actor` with the user's `prompt` and
    /// the `images` sent with it, in the mode, on the model and at the
    /// effort given (the actor's defaults when `null`), answered with
    /// [`ClientResult::Accepted`] once it has started; the actor then
    /// reports on it in `ActorSessionUpdate` frames. `title` is the one it
    /// was started with, or `null` when Cohesion titled it by the prompt's
    /// first line, which the client may replace with a `title` update.
    StartActorSession {
        actor: String,
        session_id: i64,
        prompt: String,
        title: Option<String>,
        images: Vec<ClientImage>,
        mode: Option<String>,
        model: Option<String>,
        effort: Option<String>,
    },
    /// Hand actor `actor`'s session `session_id` what the user said, message
    /// `message_id`, and the `images` sent with it: more to do, or a
    /// response to one of its requests, answered with
    /// [`ClientResult::Accepted`].
    ///
    /// `subagent`, when set, is the client's key for the subagent of the
    /// session whose request `content` responds to, as its `launch` named
    /// it; left out for the session itself. A subagent is told only
    /// responses.
    SendActorSessionMessage {
        actor: String,
        session_id: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subagent: Option<String>,
        message_id: i64,
        content: ActorUserContent,
        images: Vec<ClientImage>,
    },
    /// Stop what actor `actor`'s session `session_id` is doing, answered with
    /// [`ClientResult::Accepted`]; the session is idle until the user says
    /// more in a `SendActorSessionMessage`.
    StopActorSession { actor: String, session_id: i64 },
    /// Run actor `actor`'s session `session_id` in mode `mode` from its next step
    /// on, whether it is working, waiting on the user or done, answered
    /// with [`ClientResult::Accepted`] once the actor has taken it.
    SetActorSessionMode {
        actor: String,
        session_id: i64,
        mode: String,
    },
}

/// What a client answered a call with, tagged by `kind`; every call is
/// one of its actors', so the one answer is `accepted`.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ClientResult {
    /// The client did what an actor call asked.
    Accepted,
}

impl ClientCapabilities {
    /// Whether it holds actor `actor`.
    pub fn hosts(&self, actor: &str) -> bool {
        self.actors
            .iter()
            .any(|offered| offered.identifier == actor)
    }
}

/// A call to an actor, answered with [`ClientResult::Accepted`].
pub struct ActorCall {
    /// The actor called, by its client's identifier for it.
    pub actor: String,
    pub call: ClientCall,
}
impl ActorCall {
    /// How long the client may take before the call fails as timed out.
    pub const DEADLINE: Duration = Duration::from_secs(60);
}

/// What a client says about one of its actors' sessions, unasked: something
/// the actor said, or where the session stands. Updates are taken in the order
/// sent, each answered with a `Reply` carrying its `id`: the message stored,
/// for a `message` (whose `id` a response to a request, or an `end`,
/// names), the subagent, for a `launch`, `accepted` otherwise, or why it was
/// refused.
///
/// `session_id` is always a session Cohesion started. `subagent`, when set,
/// makes the report about one of that session's subagents, at any depth,
/// by the client's own key for it, which a `launch` names first; left out,
/// the report is about the session itself.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ActorSessionUpdate {
    pub id: u64,
    pub session_id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subagent: Option<String>,
    #[serde(flatten)]
    pub report: ActorSessionReport,
}

/// One thing an actor reports about a session, tagged by `kind`.
///
/// - `"message"`: something it said, added to the session's transcript, with
///   any `images` it shows with it (a screenshot it took, say), which pass
///   the check the user's do. Left out, none. A `toolUse`'s `category` is
///   the client's to pick from the tool's name
///   ([`ActorToolKind`](crate::model::ActorToolKind)); left out, it is
///   `"other"`.
/// - `"status"`: where the session stands now: `"working"`,
///   `"needsAttention"`, `"idle"` or `"failed"`. A resolved session that goes
///   idle or fails stays resolved.
/// - `"progress"`: what it is doing now, in a line, or `null` to clear it.
///   A session that is idle or failed is working from then on.
/// - `"end"`: the `toolUse` or `thinking` message `message_id` has ended,
///   which stamps its `ended_at`. It adds nothing to the transcript; one
///   that has ended stays as it was.
/// - `"context"`: how full the session's context window is now, replacing
///   what it said before.
/// - `"title"`: what to call the session from now on, such as one the actor
///   generated from its first turn; its first line, cut short past 80
///   characters, which must not be blank.
/// - `"launch"`: the actor launched the update's `subagent`, or says more
///   of one it launched: what it is doing, as its `title`; under which of
///   the session's subagents, by key, as `parent` (left out, the session
///   itself; a launch again can't move it); the `toolUse` message one level
///   up that launched it, as `message_id`; what kind of subagent it is, as
///   `agent_type`; the model it runs on, as `model`; and whether it runs in
///   the background. The first launch adds it, working; a launch again
///   updates it as it stands, as a client catching up does. Only a
///   subagent is launched.
///
/// A subagent takes every kind; `progress` only marks it working, since a
/// subagent shows no activity.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ActorSessionReport {
    Launch(SubagentLaunch),
    Message {
        content: ActorMessageContent,
        #[serde(default)]
        images: Vec<ClientImage>,
    },
    Status {
        status: ReportedActorSessionStatus,
    },
    Progress {
        progress: Option<String>,
    },
    End {
        message_id: i64,
    },
    Context {
        context: ContextUsage,
    },
    Title {
        title: String,
    },
}

/// What an actor says of a subagent it launched, in a `launch` report.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SubagentLaunch {
    /// What the subagent is doing, as the session's title is held: its first
    /// line, cut short past 80 characters, which must not be blank.
    pub title: String,
    /// The subagent it runs under, by the client's key; left out, the
    /// session itself.
    #[serde(default)]
    pub parent: Option<String>,
    /// The `toolUse` message one level up that launched it. Left out, none.
    #[serde(default)]
    pub message_id: Option<i64>,
    /// What kind of subagent it is, as the actor names them, such as
    /// "Explore". Left out, none.
    #[serde(default)]
    pub agent_type: Option<String>,
    /// The model it runs on, as the actor names it. Left out, the session's.
    #[serde(default)]
    pub model: Option<String>,
    /// Whether it goes on after the tool use that launched it has ended.
    /// Left out, it doesn't.
    #[serde(default)]
    pub background: bool,
}

/// How much of its limits one of the client's actors has used, unasked:
/// sent whenever that changes, each replacing the last for the actor. The
/// daemon keeps it only while the client is connected and answers with a
/// `Reply` carrying `id`: `accepted`, or why it was refused. An actor the
/// client doesn't offer is refused.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ActorUsageUpdate {
    pub id: u64,
    /// The actor, by the identifier its `Hello` gave it.
    pub actor: String,
    #[serde(flatten)]
    pub usage: ActorUsage,
}

/// How much of its plan's limits an actor has used, as its client last
/// reported. A session's context window is its own, which it reports as the
/// session's `context`.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, TsWire)]
pub struct ActorUsage {
    /// Its plan's limits, such as a five-hour and a weekly one, in the
    /// order to show them. Left out, none.
    #[serde(default)]
    pub plan: Vec<PlanUsage>,
    /// The account whose limits these are, when other actors draw on it
    /// too. Left out, the actor's limits are its own.
    #[serde(default)]
    pub pool: Option<UsagePool>,
}

/// An account several actors draw on, which one set of plan limits fuels:
/// the actors reporting the same `id`, from any client, share them, and
/// the latest report among them stands for all.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TsWire)]
pub struct UsagePool {
    /// Names the account for good, the same from every actor drawing on
    /// it, prefixed with the product to keep apart other products' ids:
    /// `claude:jackson@example.com`.
    pub id: String,
    /// The account, as people read it: `jackson@example.com`.
    pub name: String,
}

/// One limit of the plan an actor runs on, such as its five-hour limit.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TsWire, TsInterface)]
pub struct PlanUsage {
    /// What the limit is, in a few words: "5-hour limit".
    pub label: String,
    /// How much of it is used, from 0 to 1.
    pub used: f64,
    /// When it resets, in Unix seconds. Left out, unknown.
    #[serde(default)]
    pub resets_at: Option<i64>,
}

/// An image a user sent, or an actor showed.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ClientImage {
    /// `image/png`, `image/jpeg`, `image/gif` or `image/webp`.
    pub media_type: String,
    /// The image, base64-encoded.
    pub data: String,
}

/// Whether a call succeeded, with its result or why it failed.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ClientOutcome {
    Ok(ClientResult),
    /// Why the client could not answer, as people read it.
    Error(String),
}

/// Why the daemon ended a client's session. A client must not reconnect
/// on its own after either: replaced, another session serves it now, and
/// reconnecting would only take its place back; revoked, it is no longer
/// trusted and has to be paired again.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EvictionReason {
    /// Another session sent a `Hello` for the same client.
    Replaced,
    /// The client was deleted.
    Revoked,
}
