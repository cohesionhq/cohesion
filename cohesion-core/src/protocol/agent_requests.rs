//! Requests for agents: creating and configuring them, their conversation,
//! cues, skills and background runs.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

use crate::model::{
    AgentSkillScope, IntelligenceAgentColor, IntelligenceAgentPersonality,
    IntelligenceAgentVerbosity,
};

/// Creates an agent; it runs on the `agent` tier's default model.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct CreateIntelligenceAgentRequest {
    /// A blank or absent name picks an unused first name, like `Ada`.
    pub name: Option<String>,
    pub color: IntelligenceAgentColor,
    /// Shown in place of the name's initial; none when absent.
    pub emoji: Option<String>,
    /// `balanced` when absent.
    pub personality: Option<IntelligenceAgentPersonality>,
    /// `brief` when absent.
    pub verbosity: Option<IntelligenceAgentVerbosity>,
    /// What the user wants from the agent; none when absent.
    pub role: Option<String>,
    /// The rules the agent always follows; none when absent.
    pub directive: Option<String>,
    /// The team it starts on; none when absent.
    pub team_id: Option<i64>,
}

/// Lists every agent.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListIntelligenceAgentsRequest;

/// Puts the pinned agents in the order of `agent_ids`, first to last.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ReorderIntelligenceAgentsRequest {
    /// Every pinned agent, in its new order.
    pub agent_ids: Vec<i64>,
}

/// Pins or unpins agent `agent_id`; a newly pinned agent goes after the
/// other pinned agents.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetAgentPinnedRequest {
    pub agent_id: i64,
    pub pinned: bool,
}

/// Puts agent `agent_id` on team `team_id`, or on none when absent. Its pin
/// is left as it was.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct SetAgentTeamRequest {
    pub agent_id: i64,
    pub team_id: Option<i64>,
}

/// Lists every team of agents, by title.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListAgentTeamsRequest;

/// Creates a team of agents, with none on it yet.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct CreateAgentTeamRequest {
    /// Unique among teams, ignoring case.
    pub title: String,
}

/// Renames team `team_id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RenameAgentTeamRequest {
    pub team_id: i64,
    /// Unique among teams, ignoring case.
    pub title: String,
}

/// Deletes team `team_id`: its agents stay, on no team, its memories are
/// forgotten and its skills deleted.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DeleteAgentTeamRequest {
    pub team_id: i64,
}

/// Fetches agent `agent_id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct GetIntelligenceAgentRequest {
    pub agent_id: i64,
}

/// Changes agent `agent_id`; an absent field is left alone.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct UpdateIntelligenceAgentRequest {
    pub agent_id: i64,
    pub name: Option<String>,
    pub color: Option<IntelligenceAgentColor>,
    /// The new emoji; `""` clears it.
    pub emoji: Option<String>,
    pub personality: Option<IntelligenceAgentPersonality>,
    pub verbosity: Option<IntelligenceAgentVerbosity>,
    /// The new role; `""` clears it.
    pub role: Option<String>,
    /// The new directive; `""` clears it.
    pub directive: Option<String>,
}

/// Deletes agent `agent_id` with its cues, runs and messages.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DeleteIntelligenceAgentRequest {
    pub agent_id: i64,
}

/// Marks agent `agent_id`'s conversation read.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct MarkAgentReadRequest {
    pub agent_id: i64,
}

/// Pages through an agent's messages: the `limit` newest with an id below
/// `before` (the newest overall when absent), oldest first.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct ListAgentMessagesRequest {
    pub agent_id: i64,
    pub before: Option<i64>,
    pub limit: i64,
}

/// Fetches agent message `message_id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct GetAgentMessageRequest {
    pub message_id: i64,
}

/// Lists agent `agent_id`'s cues, oldest first.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListAgentCuesRequest {
    pub agent_id: i64,
}

/// Lists agent `agent_id`'s agenda, in order, finished items included.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListAgentAgendaRequest {
    pub agent_id: i64,
}

/// Removes cue `cue_id`, so its hook no longer wakes the agent; an agent
/// left with no cues goes idle.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RemoveAgentCueRequest {
    pub cue_id: i64,
}

/// Lists the memories agent `agent_id` sees: its own and the global ones;
/// only the global ones when absent. Oldest first.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct ListMemoriesRequest {
    pub agent_id: Option<i64>,
}

/// Forgets memory `memory_id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ForgetMemoryRequest {
    pub memory_id: i64,
}

/// Lists every skill, each agent's and each team's, by name.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListAgentSkillsRequest;

/// Saves a skill for one agent or for every agent on a team.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct CreateAgentSkillRequest {
    pub scope: AgentSkillScope,
    /// Unique in its scope, ignoring case.
    pub name: String,
    /// One line saying when to use it; may be empty.
    pub description: String,
    pub prompt: String,
    /// Whether the agent sees it listed and may run it itself.
    pub agent_can_use: bool,
}

/// Changes skill `skill_id`; its scope stays.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct UpdateAgentSkillRequest {
    pub skill_id: i64,
    /// Unique in its scope, ignoring case.
    pub name: String,
    pub description: String,
    pub prompt: String,
    pub agent_can_use: bool,
}

/// Deletes skill `skill_id`. Messages that ran it keep its prompt.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DeleteAgentSkillRequest {
    pub skill_id: i64,
}

/// Lists agent `agent_id`'s `limit` most recent background runs, newest
/// first.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListAgentRunsRequest {
    pub agent_id: i64,
    pub limit: i64,
}

/// Sends `message` to agent `agent_id` in its foreground chat, with any
/// images in `attachments` and the item `forward` names, which the message
/// then forwards. `message` may be empty when there are images or an item.
/// The message is saved before the reply, which comes at once. A turn the
/// agent is running takes it in before its next model call; otherwise the
/// agent's turn on it is queued behind any it is answering. What the turn posts
/// arrives with the agent's invalidations, and the agent has an `activity`
/// until it ends.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct SendAgentMessageRequest {
    pub agent_id: i64,
    pub message: String,
    pub attachments: Option<Vec<ImageUpload>>,
    pub forward: Option<AgentForwardTarget>,
    /// A skill the agent sees (its own or its team's) to run, with
    /// `message` as the note; a message runs a skill or forwards an item,
    /// not both.
    pub skill: Option<i64>,
    /// The workspace the user is in as they send it. When it isn't the one
    /// the agent was last told, the agent reads `User switched to the "…"
    /// workspace` before the message. A hint for what the user means, never
    /// a limit on where the agent works; `null` says nothing.
    pub workspace_id: Option<i64>,
}

/// The item a message to an agent forwards, by the kind its `cohesion://`
/// link names it by: a journal entry is a document.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AgentForwardTarget {
    Task { workspace_id: i64, id: i64 },
    Document { workspace_id: i64, id: i64 },
}

/// Stops the running turn of agent `agent_id`'s conversation: a script it
/// runs is interrupted and shows as cancelled, and the turn ends without
/// another model call. Nothing happens when no turn is running.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct StopAgentTurnRequest {
    pub agent_id: i64,
}

/// An image to attach to a message to an agent or in an actor session: PNG,
/// JPEG, GIF or WebP, at most 5 MB once base64-encoded.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ImageUpload {
    /// `image/png`, `image/jpeg`, `image/gif` or `image/webp`.
    pub media_type: String,
    /// The image, base64-encoded.
    pub data: String,
}

/// Lists the images attached to message `message_id`, in the order sent.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListAgentMessageAttachmentsRequest {
    pub message_id: i64,
}

/// Reacts to message `message_id`, one the agent sent, with a single emoji
/// (clients offer those in `AGENT_REACTIONS`); a blank or absent `reaction`
/// clears it. A reaction queues the agent's turn on it, which may post
/// nothing; like a message's, the reply doesn't wait for it.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct SetAgentMessageReactionRequest {
    pub message_id: i64,
    pub reaction: Option<String>,
}

/// Picks option `option` (an index into its `options`) of the choice in
/// message `message_id`, which the reply follows, and queues the agent's
/// turn on the pick; like a message's, the reply doesn't wait for it. The
/// pick posts no message; the choice records it.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RespondToAgentChoiceRequest {
    pub message_id: i64,
    pub option: usize,
}
