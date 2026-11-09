use cohesion_macros::{ColumnGroup, Model, TextEnum, TsWire};
use serde::{Deserialize, Serialize};

/// A unit of long-running work Cohesion shows the user while it goes on and
/// keeps once it ends: its title, what it is, where it stands and, while it
/// runs, a short line on its progress. The work itself is owned elsewhere
/// (an actor's session); only `ActivityTracker` writes these rows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Model, TsWire)]
#[model(table = "activities", backup = Included)]
pub struct Activity {
    pub id: i64,
    pub title: String,
    /// What the activity is, and so where a client jumps to show it.
    #[model(group)]
    pub kind: ActivityKind,
    pub status: ActivityStatus,
    /// What the work last said it is doing, such as "Read 12 of 40
    /// sources"; `null` when it said nothing.
    pub progress: Option<String>,
    pub started_at: i64,
    /// When the activity ended; `null` while it runs or waits.
    pub ended_at: Option<i64>,
}

/// What an activity is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ColumnGroup, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ActivityKind {
    /// An actor session's work: one activity for the session's whole
    /// life, open while it works or needs attention and reopened each time
    /// it works again.
    ActorSession {
        /// The session it shows.
        actor_session_id: i64,
    },
}

/// Where an activity stands.
///
/// - `"running"`, `"waiting"`, `"paused"`: going on, as an
///   [`ActivityStanding`] says.
/// - `"succeeded"`, `"failed"`, `"cancelled"`: ended, as an
///   [`ActivityOutcome`] says.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum ActivityStatus {
    Running,
    Waiting,
    Paused,
    Succeeded,
    Failed,
    Cancelled,
}

/// Where an activity that is still going stands: the statuses its work
/// reports itself.
///
/// - `"running"`: working.
/// - `"waiting"`: waiting on something outside it, such as a page that
///   isn't ready.
/// - `"paused"`: stopped until the user decides something, such as a
///   question it asked them.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum ActivityStanding {
    Running,
    Waiting,
    Paused,
}

impl From<ActivityStanding> for ActivityStatus {
    fn from(standing: ActivityStanding) -> Self {
        match standing {
            ActivityStanding::Running => Self::Running,
            ActivityStanding::Waiting => Self::Waiting,
            ActivityStanding::Paused => Self::Paused,
        }
    }
}

/// How an activity ended: the statuses only finishing it can set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityOutcome {
    Succeeded,
    Failed,
    /// Stopped before it ended, as the daemon stopping does.
    Cancelled,
}

impl From<ActivityOutcome> for ActivityStatus {
    fn from(outcome: ActivityOutcome) -> Self {
        match outcome {
            ActivityOutcome::Succeeded => Self::Succeeded,
            ActivityOutcome::Failed => Self::Failed,
            ActivityOutcome::Cancelled => Self::Cancelled,
        }
    }
}
