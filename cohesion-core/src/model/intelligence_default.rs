use cohesion_macros::{Model, TextEnum, TsWire};
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize, Model, TsWire)]
#[model(table = "intelligence_defaults", backup = Included)]
pub struct IntelligenceDefault {
    pub id: i64,
    /// The tier of the intelligence.
    pub tier: IntelligenceTier,
    /// The endpoint model the tier uses. Removing that model unsets the tier.
    pub endpoint_model_id: i64,
}

/// Which default model a session uses.
///
/// - `"agent"`: every agent conversation, and scripts that name no tier.
/// - `"low"`, `"medium"`, `"high"`: background runs, by the effort their
///   cue asked for.
#[derive(Serialize, Deserialize, TextEnum, Debug, Clone, Copy, PartialEq, Eq, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum IntelligenceTier {
    Agent,
    Low,
    Medium,
    High,
}

/// How much model a background run gets; each picks the matching tier's
/// default model.
///
/// - `"low"`: quick checks and short replies.
/// - `"medium"`: ordinary work.
/// - `"high"`: hard, multi-step reasoning.
#[derive(Serialize, Deserialize, TextEnum, Debug, Clone, Copy, PartialEq, Eq, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum IntelligenceEffort {
    Low,
    Medium,
    High,
}

impl From<IntelligenceEffort> for IntelligenceTier {
    fn from(value: IntelligenceEffort) -> Self {
        match value {
            IntelligenceEffort::Low => Self::Low,
            IntelligenceEffort::Medium => Self::Medium,
            IntelligenceEffort::High => Self::High,
        }
    }
}
