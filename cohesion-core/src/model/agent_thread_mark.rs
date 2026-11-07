use cohesion_macros::Model;
use serde::{Deserialize, Serialize};

use super::Memory;

/// Where an agent's main thread stood when its last turn ended,
/// kept so a restarted daemon carries the thread on while its provider
/// still holds its prompt cached, rather than paying to write it again.
#[derive(Clone, Debug, Serialize, Deserialize, Model)]
#[model(table = "agent_thread_marks", backup = Included)]
pub struct AgentThreadMark {
    pub id: i64,
    pub agent_id: i64,
    pub thread_id: i64,
    /// How many items the thread held then, ending with a whole exchange.
    pub items: i64,
    /// The endpoint model the thread runs on.
    pub endpoint_model_id: i64,
    /// The oldest message of the agent's conversation the thread holds:
    /// everything from here through `heard_through` is in the thread, as a
    /// line of its opening summary or as a turn of its own; earlier
    /// messages are out of it.
    pub extended_from: i64,
    /// The newest message of the agent's conversation the thread had been
    /// told of.
    pub heard_through: i64,
    /// How many memories each scope keeps, as the thread's system prompt
    /// says.
    pub memory_limit: i64,
    /// Every memory the agent sees, as the thread last heard of them.
    #[model(json)]
    pub memories: Vec<Memory>,
    /// When the latest request that read the thread's cached prompt was
    /// sent, in Unix milliseconds: the thread's own, or a later fork's that
    /// copied it as it stands.
    pub last_call_at: i64,
    /// How many tokens the provider counted in the thread's latest prompt.
    pub prompt_tokens: i64,
    /// How long the provider keeps the thread's prompt cached past
    /// `last_call_at`, in seconds.
    pub cache_seconds: i64,
}
