use cohesion_core::model::{ActivityKind, ActivityOutcome, ActivityStanding, ActivityStatus};

use crate::commands::{ActivityReport, ReportActivityCommand};
use crate::test_support::{actor_session, context, recording_context};
use crate::{
    DatabaseContext, DatabaseEventKind, FinishActivityCommand, ListActivitiesCommand,
    ShutdownCleanupCommand, StartActivityCommand,
};

/// Starts an activity titled `title` on a session of its own.
fn start(db: &DatabaseContext, title: &str) -> i64 {
    let actor_session_id = actor_session(db);
    db.send_blocking(StartActivityCommand {
        title: title.to_string(),
        kind: ActivityKind::ActorSession { actor_session_id },
    })
    .unwrap()
    .id
}

#[test]
fn the_shutdown_cleanup_cancels_what_has_not_ended() {
    let db = context();
    let ended = start(&db, "Done");
    db.send_blocking(FinishActivityCommand {
        activity_id: ended,
        outcome: ActivityOutcome::Succeeded,
    })
    .unwrap();
    start(&db, "Cut off");

    db.send_blocking(ShutdownCleanupCommand).unwrap();
    let statuses: Vec<_> = db
        .send_blocking(ListActivitiesCommand { limit: 10 })
        .unwrap()
        .into_iter()
        .map(|activity| (activity.title, activity.status))
        .collect();
    assert!(statuses.contains(&("Done".to_string(), ActivityStatus::Succeeded)));
    assert!(statuses.contains(&("Cut off".to_string(), ActivityStatus::Cancelled)));
}

#[test]
fn running_activities_list_first_then_the_newest_ended() {
    let db = context();
    for title in ["Old", "Newer"] {
        let id = start(&db, title);
        db.send_blocking(FinishActivityCommand {
            activity_id: id,
            outcome: ActivityOutcome::Failed,
        })
        .unwrap();
    }
    start(&db, "Running");
    let titles: Vec<_> = db
        .send_blocking(ListActivitiesCommand { limit: 1 })
        .unwrap()
        .into_iter()
        .map(|activity| activity.title)
        .collect();
    assert_eq!(titles, ["Running", "Newer"]);
}

#[test]
fn an_empty_title_is_refused() {
    let db = context();
    assert!(
        db.send_blocking(StartActivityCommand {
            title: "  ".to_string(),
            kind: ActivityKind::ActorSession {
                actor_session_id: actor_session(&db),
            },
        })
        .is_err()
    );
}

fn report(db: &DatabaseContext, activity_id: i64, report: ActivityReport) {
    db.send_blocking(ReportActivityCommand {
        activity_id,
        report,
    })
    .unwrap();
}

/// What each recorded activity event says: its name, status and progress.
fn activity_events(
    events: Vec<DatabaseEventKind>,
) -> Vec<(&'static str, ActivityStatus, Option<String>)> {
    events
        .into_iter()
        .filter_map(|event| match event {
            DatabaseEventKind::ActivityStarted { activity } => {
                Some(("started", activity.status, activity.progress))
            }
            DatabaseEventKind::ActivityUpdated { activity } => {
                Some(("updated", activity.status, activity.progress))
            }
            DatabaseEventKind::ActivityEnded { activity } => {
                Some(("ended", activity.status, activity.progress))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn activity_events_carry_each_change_and_skip_what_changes_nothing() {
    let (db, events) = recording_context();
    let id = start(&db, "Research");
    let progress = || ActivityReport::Progress(Some("Read 3 sources".to_string()));
    report(&db, id, progress());
    report(&db, id, progress());
    report(&db, id, ActivityReport::Standing(ActivityStanding::Running));
    report(&db, id, ActivityReport::Standing(ActivityStanding::Paused));
    db.send_blocking(FinishActivityCommand {
        activity_id: id,
        outcome: ActivityOutcome::Succeeded,
    })
    .unwrap();
    report(&db, id, ActivityReport::Progress(None));
    db.send_blocking(FinishActivityCommand {
        activity_id: id,
        outcome: ActivityOutcome::Failed,
    })
    .unwrap();
    let progressed = Some("Read 3 sources".to_string());
    assert_eq!(
        activity_events(events.take()),
        [
            ("started", ActivityStatus::Running, None),
            ("updated", ActivityStatus::Running, progressed.clone()),
            ("updated", ActivityStatus::Paused, progressed),
            ("ended", ActivityStatus::Succeeded, None),
        ]
    );
}

#[test]
fn the_shutdown_cleanup_raises_no_activity_event() {
    let (db, events) = recording_context();
    start(&db, "Cut off");
    events.take();
    db.send_blocking(ShutdownCleanupCommand).unwrap();
    assert!(activity_events(events.take()).is_empty());
}

#[test]
fn a_session_keeps_one_activity_reopened_each_time_it_works() {
    let (db, events) = recording_context();
    let actor_session_id = actor_session(&db);
    let start = |title: &str| {
        db.send_blocking(StartActivityCommand {
            title: title.to_string(),
            kind: ActivityKind::ActorSession { actor_session_id },
        })
    };
    let first = start("Fix the build").unwrap();
    report(
        &db,
        first.id,
        ActivityReport::Progress(Some("Building".to_string())),
    );
    // Still running: there is nothing to reopen.
    assert!(start("Again").is_err());
    db.send_blocking(FinishActivityCommand {
        activity_id: first.id,
        outcome: ActivityOutcome::Failed,
    })
    .unwrap();
    events.take();

    let again = start("Fix the tests").unwrap();
    assert_eq!(again.id, first.id);
    assert_eq!(again.title, "Fix the tests");
    assert_eq!(again.status, ActivityStatus::Running);
    assert_eq!(again.progress, None);
    assert_eq!(again.started_at, first.started_at);
    assert_eq!(again.ended_at, None);
    assert_eq!(
        activity_events(events.take()),
        [("started", ActivityStatus::Running, None)]
    );
    let listed = db
        .send_blocking(ListActivitiesCommand { limit: 10 })
        .unwrap();
    assert_eq!(listed, [again]);
}
