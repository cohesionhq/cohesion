use cohesion_macros::{Fields, Model, TextEnum, TsWire};
use serde::{Deserialize, Serialize};

use crate::model::{HookKind, IntelligenceEffort};

/// The palette an agent, a task folder or a table's choice is drawn in.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum IntelligenceAgentColor {
    Blue,
    Green,
    Orange,
    Purple,
    Red,
    Pink,
    Teal,
    Yellow,
}

impl IntelligenceAgentColor {
    /// The palette color's exact value, the light scheme's
    /// `--agent-<color>` in `web/src/lib/tokens.css`.
    pub const fn hex(self) -> &'static str {
        match self {
            Self::Blue => "#4a6fb5",
            Self::Green => "#4f8a5a",
            Self::Orange => "#c4702e",
            Self::Purple => "#7a5bb0",
            Self::Red => "#b54848",
            Self::Pink => "#b8578f",
            Self::Teal => "#3d8a8a",
            Self::Yellow => "#b8932a",
        }
    }
}

/// The manner an agent speaks in.
///
/// - `"balanced"`: plain and even, like a helpful co-worker.
/// - `"friendly"`: warm and encouraging.
/// - `"professional"`: formal and neutral.
/// - `"candid"`: blunt; says when something looks wrong and pushes back.
/// - `"playful"`: light and humorous.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum IntelligenceAgentPersonality {
    #[default]
    Balanced,
    Friendly,
    Professional,
    Candid,
    Playful,
}

/// How much an agent says, from least to most.
///
/// - `"terse"`: as few words as will do.
/// - `"brief"`: short messages, detail only when asked.
/// - `"detailed"`: explains its reasoning and what it found.
/// - `"thorough"`: full context, options and caveats.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum IntelligenceAgentVerbosity {
    Terse,
    #[default]
    Brief,
    Detailed,
    Thorough,
}

/// A persistent assistant that keeps its context over time and can wake
/// itself on a hook.
#[derive(Clone, Debug, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "intelligence_agents", backup = Included)]
pub struct IntelligenceAgent {
    pub id: i64,
    /// When the agent was created, in Unix seconds.
    pub created_at: i64,
    /// The agent's name, chosen by the user.
    #[fields(rename = "name")]
    pub agent_name: String,
    /// The color the agent is drawn in.
    #[fields(rename = "color")]
    pub agent_color: IntelligenceAgentColor,
    /// An emoji shown in place of the name's initial; none when unset.
    #[fields(rename = "emoji")]
    pub agent_emoji: Option<String>,
    /// The manner the agent speaks in.
    #[fields(rename = "personality")]
    pub agent_personality: IntelligenceAgentPersonality,
    /// How much the agent says.
    #[fields(rename = "verbosity")]
    pub agent_verbosity: IntelligenceAgentVerbosity,
    /// What the user wants from the agent, in a few words; `""` when unset.
    #[fields(rename = "role")]
    pub agent_role: String,
    /// The rules the user gave the agent to always follow; `""` when unset.
    #[fields(rename = "directive")]
    pub agent_directive: String,
    /// What the agent is doing while it works, such as `"Thinking"` or the
    /// title of the script it is running; `null` when it is not working.
    pub activity: Option<String>,
    /// When the running turn of the agent's conversation started, in Unix
    /// seconds; the user can stop it. `null` when none is running; forks
    /// don't count.
    pub foreground_started: Option<i64>,
    /// How many tokens the conversation's live model thread held when its
    /// latest request was sent, as the provider counted them (cached or not):
    /// the size of the working set the next request builds on. Updated after
    /// each request of the conversation's turns; forks, jobs and cues don't
    /// count. `null` while the agent has no live thread, and for a new thread until its first
    /// request is answered.
    #[fields(skip)]
    pub context_tokens: Option<i64>,
    /// Whether the agent has posted since the user last read its
    /// conversation; questions between agents don't count.
    #[fields(rename = "unread")]
    pub agent_unread: bool,
    /// When the latest message in the agent's conversation was sent, in Unix
    /// seconds, not counting code runs, documentation lookups, memory
    /// changes, questions between agents or the agent's bookkeeping (notes,
    /// wake lines, compressed contexts); `null` before the first.
    pub last_message_at: Option<i64>,
    /// Whether the agent is pinned, ahead of the others.
    pub pinned: bool,
    /// Where the agent sits among the pinned agents; lower comes first.
    ///
    /// Unpinned agents go by their latest message instead.
    pub sort_order: i64,
    /// The team the agent is on; `null` when it's on none. Pinning is
    /// apart from teams: a pinned agent stays on its team.
    pub team_id: Option<i64>,
    /// The workspace the user was in when they last named one in a message
    /// to the agent; `null` until one does. A hint for requests that don't
    /// say where, never a limit on where the agent works.
    pub workspace_id: Option<i64>,
    /// The endpoint model the agent's conversation runs on in place of the
    /// `agent` tier's default; `null` to use the default. Removing that
    /// model sets it back to `null`. Background runs keep their cue's tier.
    pub endpoint_model_id: Option<i64>,
    /// How many actor sessions not yet resolved the agent follows.
    #[model(expr = IntelligenceAgent::FOLLOWING_COUNT_SQL)]
    #[fields(skip)]
    pub following_count: i64,
}

impl IntelligenceAgent {
    /// The SQL of [`following_count`](Self::following_count), which
    /// [`ActorSession::counted_by`](super::ActorSession::counted_by) mirrors
    /// to tell which agents a write to a session recounts.
    pub const FOLLOWING_COUNT_SQL: &'static str = "(SELECT COUNT(*) FROM actor_sessions \
        WHERE actor_sessions.follower_agent_id = intelligence_agents.id \
        AND actor_sessions.status <> 'resolved')";
}

/// A team of agents: a named group whose members see each other in their
/// prompts and share the team's memories.
#[derive(Clone, Debug, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "intelligence_agent_teams", backup = Included)]
pub struct IntelligenceAgentTeam {
    pub id: i64,
    /// The team's name, unique ignoring case.
    pub title: String,
}

/// A hook an agent watches to wake itself in the background.
///
/// An agent can have several, each with its own name.
#[derive(Clone, Debug, Serialize, Deserialize, Model, TsWire)]
#[model(table = "intelligence_agent_cues", backup = Included)]
pub struct IntelligenceAgentCue {
    pub id: i64,
    pub agent_id: i64,
    /// What the agent calls the cue, unique among its cues; setting a cue
    /// with the same name replaces it.
    pub name: String,
    /// The hook that runs the cue.
    pub hook: HookKind,
    /// A function expression called with the hook's subject; a truthy return
    /// wakes the agent.
    pub script: String,
    /// What the agent told itself to do when it wakes.
    pub instructions: String,
    /// How capable a model each background run the cue wakes gets.
    pub effort: IntelligenceEffort,
    /// When the cue was last set, in Unix seconds.
    pub set_at: i64,
}

/// Who started a chain of an agent's work, which every turn of the chain
/// inherits and which alone decides what it may change.
///
/// - `"user"`: the user's message or pick, and every question and actor
///   follow-up that descends from it: everything the conversation may do.
/// - `"event"`: one of the agent's cues: it reads, and asks the user before
///   changing anything.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum Initiative {
    #[default]
    User,
    Event,
}
