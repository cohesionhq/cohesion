use cohesion_macros::{ColumnGroup, Model, TsWire};
use serde::{Deserialize, Serialize};

/// A fact an agent keeps across conversations, in its own words.
#[derive(Clone, Debug, Serialize, Deserialize, Model, TsWire)]
#[model(table = "memories", backup = Included)]
pub struct Memory {
    pub id: i64,
    /// Who sees it: one agent, one team's agents, or every agent.
    #[model(group)]
    pub scope: MemoryScope,
    pub created_at: i64,
    pub memory: String,
}

/// Who a memory is for. The table's check gives every row at most one of
/// an agent and a team.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ColumnGroup, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MemoryScope {
    /// Every agent's.
    Global,
    /// The agents' on one team; it goes with the team.
    Team {
        /// The team it belongs to.
        team_id: i64,
    },
    /// One agent's own; it goes with the agent.
    Agent {
        /// The agent it belongs to.
        agent_id: i64,
    },
}

/// Which memories a listing returns.
///
/// - `{"seenBy": id}`: what agent `id` sees: its own, then its team's, then
///   the global ones.
/// - `"global"`: the global ones alone.
/// - `"every"`: every memory, of every agent and team and the global ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TsWire)]
#[serde(rename_all = "camelCase")]
pub enum MemoryFilter {
    SeenBy(i64),
    Global,
    Every,
}
