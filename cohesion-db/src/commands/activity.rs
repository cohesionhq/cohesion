use cohesion_core::{
    FromRequest,
    invalidation::Invalidation,
    model::{Activity, ActivityKind, ActivityOutcome, ActivityStanding, ActivityStatus},
    protocol::ListActivitiesRequest,
};

use super::title::ensure_title;
use crate::changed::{Changed, Invalidates};
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, DatabaseEventKind, Filtered, Found,
    Read, ReadableDatabase, Write,
};

/// Starts an activity, `running`, under `title` trimmed, which must not be
/// empty, raising `ActivityStarted`. What it shows has one activity for its
/// whole life: once that one has ended, it is reopened, keeping its id and
/// `started_at`, its progress and `ended_at` cleared; only the first start
/// inserts it. Starting what is still running is refused. Only
/// `ActivityTracker` writes activities, so the commands that do are its
/// alone.
pub(crate) struct StartActivityCommand {
    pub title: String,
    pub kind: ActivityKind,
}
impl DatabaseCommand for StartActivityCommand {
    type Response = Activity;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let title = self.title.trim();
        ensure_title(title)?;
        let ActivityKind::ActorSession { actor_session_id } = self.kind;
        let reopened = ctx
            .db
            .update::<Activity>()
            .set(Activity::TITLE, title.to_string())
            .set(Activity::STATUS, ActivityStatus::Running)
            .set_null(Activity::PROGRESS)
            .set_null(Activity::ENDED_AT)
            .filter("actor_session_id = ?")
            .bind(actor_session_id)
            .filter("ended_at IS NOT NULL")
            .returning_one()?;
        let activity = match reopened {
            Some(activity) => activity,
            None => ctx
                .db
                .insert()
                .set(Activity::TITLE, title.to_string())
                .set_group(self.kind)
                .returning_one()
                .found()?,
        };
        let event = DatabaseEventKind::ActivityStarted {
            activity: activity.clone(),
        };
        ctx.events.send(ctx.trace.event(event));
        Ok(activity.invalidating([Invalidation::Activities]))
    }
}

/// Sets what activity `activity_id` says it is doing, or clears it, where
/// it stands, or its title, trimmed and not empty, while it has not ended, raising `ActivityUpdated`; an
/// ended one, or a report of what it already says, is left as it is.
pub(crate) struct ReportActivityCommand {
    pub activity_id: i64,
    pub report: ActivityReport,
}

/// What an activity's work says about itself.
pub(crate) enum ActivityReport {
    Progress(Option<String>),
    Standing(ActivityStanding),
    Title(String),
}

impl DatabaseCommand for ReportActivityCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let update = ctx.db.update::<Activity>();
        let update = match self.report {
            ActivityReport::Progress(progress) => update
                .set(Activity::PROGRESS, progress.clone())
                .filter("progress IS NOT ?")
                .bind(progress),
            ActivityReport::Standing(standing) => {
                let status = ActivityStatus::from(standing);
                update
                    .set(Activity::STATUS, status)
                    .filter("status IS NOT ?")
                    .bind(status)
            }
            ActivityReport::Title(title) => {
                let title = title.trim().to_string();
                ensure_title(&title)?;
                update
                    .set(Activity::TITLE, title.clone())
                    .filter("title IS NOT ?")
                    .bind(title)
            }
        };
        let Some(activity) = update
            .filter_eq(Activity::ID, self.activity_id)
            .filter_eq(Activity::ENDED_AT, None)
            .returning_one()?
        else {
            return Ok(().invalidating_nothing());
        };
        let event = DatabaseEventKind::ActivityUpdated { activity };
        ctx.events.send(ctx.trace.event(event));
        Ok(().invalidating([Invalidation::Activities]))
    }
}

/// Ends activity `activity_id` with `outcome`, clearing its progress and
/// raising `ActivityEnded`, unless it has already ended: an outcome is
/// final.
pub(crate) struct FinishActivityCommand {
    pub activity_id: i64,
    pub outcome: ActivityOutcome,
}
impl DatabaseCommand for FinishActivityCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let Some(activity) = ctx
            .db
            .update::<Activity>()
            .set(Activity::STATUS, ActivityStatus::from(self.outcome))
            .set_null(Activity::PROGRESS)
            .set_now(Activity::ENDED_AT)
            .filter_eq(Activity::ID, self.activity_id)
            .filter_eq(Activity::ENDED_AT, None)
            .returning_one()?
        else {
            return Ok(().invalidating_nothing());
        };
        // An actor session's stretch of work ending can leave the session unread.
        let ActivityKind::ActorSession { actor_session_id } = activity.kind;
        let keys = [
            Invalidation::Activities,
            Invalidation::ActorSessions,
            Invalidation::ActorSession(actor_session_id),
        ];
        let event = DatabaseEventKind::ActivityEnded { activity };
        ctx.events.send(ctx.trace.event(event));
        Ok(().invalidating(keys))
    }
}

/// Cancels every activity that has not ended, since a stopped daemon runs
/// none, raising nothing: it runs as the daemon stops, or as it starts
/// before hooks do. Part of [`ShutdownCleanupCommand`](crate::ShutdownCleanupCommand).
pub struct SettleActivitiesCommand;
impl DatabaseCommand for SettleActivitiesCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let settled = ctx
            .db
            .update::<Activity>()
            .set(Activity::STATUS, ActivityStatus::Cancelled)
            .set_null(Activity::PROGRESS)
            .set_now(Activity::ENDED_AT)
            .filter_eq(Activity::ENDED_AT, None)
            .execute()?;
        Ok(().invalidating(if settled > 0 {
            vec![Invalidation::Activities]
        } else {
            Vec::new()
        }))
    }
}

/// The activities still running or waiting, newest first, then the `limit`
/// that ended most recently.
#[derive(FromRequest)]
#[request(ListActivitiesRequest)]
pub struct ListActivitiesCommand {
    pub limit: i64,
}
impl DatabaseCommand for ListActivitiesCommand {
    type Response = Vec<Activity>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        let mut activities: Vec<Activity> = ctx
            .db
            .select()
            .filter_eq(Activity::ENDED_AT, None)
            .order("started_at DESC, id DESC")
            .all()?;
        let ended: Vec<Activity> = ctx
            .db
            .select()
            .filter("ended_at IS NOT NULL")
            .order("ended_at DESC, id DESC")
            .limit(self.limit as u64)
            .all()?;
        activities.extend(ended);
        Ok(activities)
    }
}

#[cfg(test)]
mod tests;
