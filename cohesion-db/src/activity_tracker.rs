//! The activities: long-running work the user follows from one place. Work
//! opts in by running as one through [`ActivityTracker::start`], which does
//! the paperwork; the work only reports through its [`ActivityHandle`].

use std::future::Future;

use cohesion_core::log::Module;
use cohesion_core::log_error;
use cohesion_core::model::{ActivityKind, ActivityOutcome, ActivityStanding};

use crate::commands::{
    ActivityReport, FinishActivityCommand, ReportActivityCommand, StartActivityCommand,
};
use crate::{CommandError, DatabaseContext, WorkTracker};

/// Runs work as activities: records each one, counts it as the daemon's
/// work while it runs, records how it ended and stops it when the daemon
/// shuts down. The only writer of activities. A cheap clone.
#[derive(Clone)]
pub struct ActivityTracker {
    database: DatabaseContext,
    work: WorkTracker,
}

/// How an activity's work says it ended: a `Result` succeeded or failed,
/// and an [`ActivityOutcome`] says so itself (work that can be stopped).
pub trait WorkOutcome {
    fn outcome(&self) -> ActivityOutcome;
}

impl<T, E> WorkOutcome for Result<T, E> {
    fn outcome(&self) -> ActivityOutcome {
        match self {
            Ok(_) => ActivityOutcome::Succeeded,
            Err(_) => ActivityOutcome::Failed,
        }
    }
}

impl WorkOutcome for ActivityOutcome {
    fn outcome(&self) -> ActivityOutcome {
        *self
    }
}

/// How [`ActivityTracker::start`]'s work ended.
#[derive(Debug, PartialEq)]
pub enum ActivityEnd<R> {
    /// It returned `R`.
    Finished(R),
    /// The daemon shut down first, so it was dropped where it stood.
    Cancelled,
}

/// What an activity's work holds to report on itself while it runs: the
/// one way to reach its activity. Ending it is the tracker's, so a handle
/// can't. A cheap clone, for the parts of the work that report.
#[derive(Clone)]
pub struct ActivityHandle {
    database: DatabaseContext,
    id: i64,
}

impl ActivityTracker {
    /// Activities on `database`, their work counted in `work` and stopped
    /// once it closes.
    pub fn new(database: DatabaseContext, work: WorkTracker) -> Self {
        Self { database, work }
    }

    /// Runs `work` as a new activity titled `title`, of `kind`: records it
    /// as running, hands `work` its handle and awaits it, then records it as
    /// succeeded or failed by what `work` returns. Once the daemon starts
    /// shutting down, `work` is dropped and the activity cancelled. The
    /// caller holds the future (awaiting it, or spawning it), and the
    /// activity counts as the daemon's work from this call until the future
    /// ends, so a shutdown waits for work spawned before it began. An error
    /// only if the activity could not be recorded, before `work` runs; a
    /// failure to record its end is logged, and the shutdown cleanup
    /// cancels it.
    pub fn start<R, F, Fut>(
        &self,
        title: String,
        kind: ActivityKind,
        work: F,
    ) -> impl Future<Output = Result<ActivityEnd<R>, CommandError>> + use<R, F, Fut>
    where
        R: WorkOutcome,
        F: FnOnce(ActivityHandle) -> Fut,
        Fut: Future<Output = R>,
    {
        let running = self.work.begin(format!("activity: {title}"));
        let tracker = self.clone();
        async move {
            let _running = running;
            let activity = tracker
                .database
                .send(StartActivityCommand { title, kind })
                .await?;
            let handle = ActivityHandle {
                database: tracker.database.clone(),
                id: activity.id,
            };
            let (end, outcome) = tokio::select! {
                result = work(handle) => {
                    let outcome = result.outcome();
                    (ActivityEnd::Finished(result), outcome)
                }
                () = tracker.work.closed() => (ActivityEnd::Cancelled, ActivityOutcome::Cancelled),
            };
            let finished = tracker
                .database
                .send(FinishActivityCommand {
                    activity_id: activity.id,
                    outcome,
                })
                .await;
            if let Err(e) = finished {
                log_error!(
                    Module::Database,
                    "Activity {}: could not record its end: {e:?}",
                    activity.id
                );
            }
            Ok(end)
        }
    }
}

impl ActivityHandle {
    /// The activity's id, which clients and the owner's own rows name it by.
    pub fn id(&self) -> i64 {
        self.id
    }

    /// Says what the work is doing now, such as "Read 12 of 40 sources", or
    /// clears it with `None`.
    pub async fn progress(&self, progress: Option<String>) -> Result<(), CommandError> {
        self.report(ActivityReport::Progress(progress)).await
    }

    /// Says where the work stands: working, waiting on something outside
    /// it, or paused until the user decides something.
    pub async fn standing(&self, standing: ActivityStanding) -> Result<(), CommandError> {
        self.report(ActivityReport::Standing(standing)).await
    }

    /// Calls the work `title` from now on, trimmed, which must not be
    /// empty, such as once what it is about is known better.
    pub async fn title(&self, title: String) -> Result<(), CommandError> {
        self.report(ActivityReport::Title(title)).await
    }

    async fn report(&self, report: ActivityReport) -> Result<(), CommandError> {
        self.database
            .send(ReportActivityCommand {
                activity_id: self.id,
                report,
            })
            .await
    }
}

#[cfg(test)]
mod tests;
