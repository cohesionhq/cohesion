use cohesion_macros::{Model, TextEnum, TsWire};
use serde::{Deserialize, Serialize};

/// The most every model call together may cost over one period before
/// background work stops making model calls. A period without a row has
/// no limit.
#[derive(Clone, Debug, Serialize, Deserialize, Model, TsWire)]
#[model(table = "intelligence_spend_limits", backup = Included)]
pub struct IntelligenceSpendLimit {
    pub id: i64,
    /// Each period has one limit at most.
    pub period: SpendPeriod,
    /// In US dollars; more than zero.
    pub amount: f64,
}

/// How far back a spend limit counts model calls. Each is rolling: it
/// counts what was spent in the length of time just past, so spending
/// falls back under a limit as old calls age out, not at a set hour.
///
/// - `"hour"`: the last 60 minutes.
/// - `"day"`: the last 24 hours.
/// - `"month"`: the last 30 days.
#[derive(Serialize, Deserialize, TextEnum, Debug, Clone, Copy, PartialEq, Eq, Hash, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum SpendPeriod {
    Hour,
    Day,
    Month,
}

impl SpendPeriod {
    /// How many seconds back the period counts.
    pub fn seconds(self) -> i64 {
        const HOUR: i64 = 60 * 60;
        match self {
            Self::Hour => HOUR,
            Self::Day => 24 * HOUR,
            Self::Month => 30 * 24 * HOUR,
        }
    }
}

/// What every model call cost over one period, against that period's
/// limit, if it has one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TsWire)]
pub struct IntelligenceSpend {
    pub period: SpendPeriod,
    /// In US dollars; `None` when the period has no limit.
    pub limit: Option<f64>,
    /// What the calls in the period cost, in US dollars.
    pub spent: f64,
    /// Whether `spent` has reached `limit`, which stops background work
    /// making model calls.
    pub reached: bool,
}
