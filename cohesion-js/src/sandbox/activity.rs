//! `Activity`, the plain object `onActivityStarted`, `onActivityUpdated`
//! and `onActivityEnded` receive.

use cohesion_core::model::{self, ActivityStatus};
use cohesion_core::ts_documentation::TsDocumentation;
use serde::Serialize;

/// An activity, serialized to the object a script receives.
#[derive(Serialize)]
pub struct Activity {
    id: i64,
    title: String,
    kind: ActivityKind,
    status: ActivityStatus,
    progress: Option<String>,
    #[serde(rename = "startedAt")]
    started_at: i64,
    #[serde(rename = "endedAt")]
    ended_at: Option<i64>,
}

/// What an activity is, with camelCase fields as scripts read them.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum ActivityKind {
    ActorSession {
        #[serde(rename = "actorSessionId")]
        actor_session_id: i64,
    },
}

impl From<model::Activity> for Activity {
    fn from(activity: model::Activity) -> Self {
        let model::ActivityKind::ActorSession { actor_session_id } = activity.kind;
        let kind = ActivityKind::ActorSession { actor_session_id };
        Self {
            id: activity.id,
            title: activity.title,
            kind,
            status: activity.status,
            progress: activity.progress,
            started_at: activity.started_at,
            ended_at: activity.ended_at,
        }
    }
}

impl TsDocumentation for Activity {
    const TS: &'static str = r#"/**
 * Long-running work the user follows, as `onActivityStarted`,
 * `onActivityUpdated` and `onActivityEnded` receive it: plain data, as it
 * stood when the hook fired. `startedAt` and `endedAt` are in Unix seconds.
 */
interface Activity {
  id: number;
  title: string;
  /**
   * What the activity is: `"actorSession"`, a stretch of an actor's session,
   * from when it starts until it is idle again.
   */
  kind: { kind: "actorSession"; actorSessionId: number };
  /**
   * `"running"`, `"waiting"` (on something outside it) or `"paused"`
   * (until the user decides something) while it goes on; `"succeeded"`,
   * `"failed"` or `"cancelled"` once it ends.
   */
  status: "running" | "waiting" | "paused" | "succeeded" | "failed" | "cancelled";
  /**
   * What the work last said it is doing, such as `"Read 12 of 40
   * sources"`; `null` when it said nothing, and once it ends.
   */
  progress: string | null;
  startedAt: number;
  /** When it ended; `null` while it goes on. */
  endedAt: number | null;
}"#;
    const TS_BRIEF: &'static str = Self::TS;
}
