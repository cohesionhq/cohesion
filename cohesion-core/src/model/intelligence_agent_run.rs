use cohesion_macros::{Model, TextEnum, TsWire};
use serde::{Deserialize, Serialize};

use crate::model::HookKind;
use crate::stored_model::ColumnGroup;
use crate::value::{IntoValue, Row, RowError, Value, read_column};

/// Which state a run's `outcome` column reads.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum IntelligenceAgentRunOutcome {
    Running,
    Completed,
    Failed,
}

/// Where a fork of an agent's conversation stands, tagged by `status`.
///
/// - `"running"`: still going.
/// - `"completed"`: finished normally, at `ended_at`, in Unix seconds.
/// - `"failed"`: stopped on `error`, at `ended_at`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, TsWire)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum IntelligenceAgentRunState {
    Running,
    Completed { ended_at: i64 },
    Failed { ended_at: i64, error: String },
}

/// A run's state as `outcome` and the columns it sets, which the table's checks
/// hold to.
impl ColumnGroup for IntelligenceAgentRunState {
    const COLUMNS: &'static [&'static str] = &["outcome", "ended_at", "error"];

    fn from_row(row: &dyn Row) -> Result<Self, RowError> {
        Ok(
            match read_column::<IntelligenceAgentRunOutcome>(row, "outcome")? {
                IntelligenceAgentRunOutcome::Running => Self::Running,
                IntelligenceAgentRunOutcome::Completed => Self::Completed {
                    ended_at: read_column(row, "ended_at")?,
                },
                IntelligenceAgentRunOutcome::Failed => Self::Failed {
                    ended_at: read_column(row, "ended_at")?,
                    error: read_column(row, "error")?,
                },
            },
        )
    }

    fn into_values(self) -> Vec<Value> {
        match self {
            Self::Running => vec![
                IntelligenceAgentRunOutcome::Running.into_value(),
                Value::Null,
                Value::Null,
            ],
            Self::Completed { ended_at } => vec![
                IntelligenceAgentRunOutcome::Completed.into_value(),
                ended_at.into_value(),
                Value::Null,
            ],
            Self::Failed { ended_at, error } => vec![
                IntelligenceAgentRunOutcome::Failed.into_value(),
                ended_at.into_value(),
                error.into_value(),
            ],
        }
    }
}

/// One fork of an agent's conversation: one of its cues, another agent's
/// question or an actor session it follows woke it.
#[derive(Clone, Debug, Serialize, Deserialize, Model, TsWire)]
#[model(table = "intelligence_agent_runs", backup = Included)]
pub struct IntelligenceAgentRun {
    pub id: i64,
    pub agent_id: i64,
    /// The intelligence thread of the session, once it has one.
    pub thread_id: Option<i64>,
    /// The hook that fired; `agent:message` for a fork another agent's
    /// question woke, `actor:moved` for one an actor session it follows woke,
    /// `task:assigned` for one a task assigned to it woke.
    pub hook: HookKind,
    /// The name of the cue that started the run; `null` for a fork, and for
    /// runs from before agents had several cues.
    pub cue_name: Option<String>,
    /// The agent whose question woke the run, a fork of the conversation;
    /// `null` for a cue's run, or once that agent is deleted.
    pub sender_agent_id: Option<i64>,
    pub started_at: i64,
    /// Where it stands, with when it ended and why it failed.
    #[model(group)]
    pub state: IntelligenceAgentRunState,
    /// Prompt tokens across the run that were neither read from nor written
    /// to the cache.
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    /// What the run cost; `null` when nothing was priced.
    pub cost: Option<f64>,
}
