use cohesion_macros::{Model, TsWire};
use serde::{Deserialize, Serialize};

/// One item of an agent's agenda: a step it means to take, kept in order
/// with the others and shared by every thread of its conversation. The
/// agent adds, finishes and clears them with its agenda tools; a finished
/// item stays until the agenda is cleared.
#[derive(Clone, Debug, Serialize, Deserialize, Model, TsWire)]
#[model(table = "agent_agenda_items", backup = Included)]
pub struct AgentAgendaItem {
    pub id: i64,
    pub agent_id: i64,
    /// Where it stands in the agenda, from 1.
    pub position: i64,
    pub text: String,
    /// When it was added, in Unix seconds.
    pub added_at: i64,
    /// When it was finished, in Unix seconds; `null` while it is open.
    pub finished_at: Option<i64>,
}

/// How many items an agent's agenda holds at most, finished ones included.
pub const AGENDA_LIMIT: usize = 20;

/// How many characters one agenda item holds at most.
pub const AGENDA_ITEM_CHARS: usize = 200;
