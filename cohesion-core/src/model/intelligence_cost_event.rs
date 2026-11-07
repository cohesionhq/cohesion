use cohesion_macros::{Model, TsWire};
use serde::{Deserialize, Serialize};

/// A record of what one model invocation cost.
#[derive(Debug, Clone, Serialize, Deserialize, Model, TsWire)]
#[model(table = "intelligence_cost_events", backup = Included)]
pub struct IntelligenceCostEvent {
    pub id: i64,
    pub created_at: i64,
    /// The thread the invocation answered; `None` for events recorded before
    /// threads were linked.
    pub thread_id: Option<i64>,
    pub provider_identifier: String,
    pub model_identifier: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    /// What the invocation cost in all, in USD.
    pub total_cost: f64,
    /// What each kind of token cost of the total; `None` when the provider
    /// reported only the total.
    pub input_cost: Option<f64>,
    pub output_cost: Option<f64>,
    pub cache_read_cost: Option<f64>,
    pub cache_write_cost: Option<f64>,
}
