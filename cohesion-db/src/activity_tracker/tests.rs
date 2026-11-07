use cohesion_core::model::{Activity, ActivityKind, ActivityStanding, ActivityStatus};

use crate::test_support::{actor_session, context};
use crate::{
    ActivityEnd, ActivityHandle, ActivityTracker, DatabaseContext, ListActivitiesCommand,
    WorkTracker,
};

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
}

/// A tracker over a fresh database, its work counted in the tracker
/// returned, and the kind of an activity of a new actor session's.
fn tracker() -> (DatabaseContext, WorkTracker, ActivityTracker, ActivityKind) {
    let db = context();
    let work = WorkTracker::default();
    let activities = ActivityTracker::new(db.clone(), work.clone());
    let kind = ActivityKind::ActorSession {
        actor_session_id: actor_session(&db),
    };
    (db, work, activities, kind)
}

fn only_activity(db: &DatabaseContext) -> Activity {
    let mut activities = db
        .send_blocking(ListActivitiesCommand { limit: 10 })
        .unwrap();
    assert_eq!(activities.len(), 1);
    activities.remove(0)
}

/// [`only_activity`] from inside the activity's work, which runs on the runtime.
async fn only_activity_now(db: &DatabaseContext) -> Activity {
    let mut activities = db.send(ListActivitiesCommand { limit: 10 }).await.unwrap();
    assert_eq!(activities.len(), 1);
    activities.remove(0)
}

#[test]
fn work_that_returns_ok_succeeds_and_err_fails() {
    let (db, _work, activities, kind) = tracker();
    let runtime = runtime();
    let end = runtime.block_on(activities.start(
        "Research".to_string(),
        kind.clone(),
        |activity: ActivityHandle| async move {
            activity.progress(Some("Read 1 of 2".to_string())).await?;
            activity.title(" Research the launch ".to_string()).await?;
            assert!(activity.title(" ".to_string()).await.is_err());
            Ok::<_, crate::CommandError>(7)
        },
    ));
    assert!(matches!(end, Ok(ActivityEnd::Finished(Ok(7)))));
    let activity = only_activity(&db);
    assert_eq!(activity.status, ActivityStatus::Succeeded);
    assert_eq!(activity.title, "Research the launch");
    assert!(activity.ended_at.is_some());
    // An ended activity says nothing more about its progress.
    assert_eq!(activity.progress, None);

    let end = runtime
        .block_on(activities.start("Poll".to_string(), kind, |_| async { Err::<(), _>("gone") }));
    assert!(matches!(end, Ok(ActivityEnd::Finished(Err("gone")))));
    let failed = db
        .send_blocking(ListActivitiesCommand { limit: 10 })
        .unwrap();
    assert_eq!(failed[0].title, "Poll");
    assert_eq!(failed[0].status, ActivityStatus::Failed);
}

#[test]
fn reports_reach_the_activity_while_it_runs() {
    let (db, _work, activities, kind) = tracker();
    let reader = db.clone();
    let end = runtime().block_on(activities.start(
        "Research".to_string(),
        kind,
        |activity: ActivityHandle| async move {
            activity.progress(Some("Read 12 of 40".to_string())).await?;
            activity.standing(ActivityStanding::Waiting).await?;
            let seen = only_activity_now(&reader).await;
            assert_eq!(seen.progress.as_deref(), Some("Read 12 of 40"));
            assert_eq!(seen.status, ActivityStatus::Waiting);
            activity.standing(ActivityStanding::Paused).await?;
            assert_eq!(
                only_activity_now(&reader).await.status,
                ActivityStatus::Paused
            );
            activity.standing(ActivityStanding::Running).await?;
            assert_eq!(
                only_activity_now(&reader).await.status,
                ActivityStatus::Running
            );
            Ok::<_, crate::CommandError>(())
        },
    ));
    assert!(matches!(end, Ok(ActivityEnd::Finished(Ok(())))));
}

#[test]
fn closing_the_work_cancels_the_activity_and_drops_its_work() {
    let (db, work, activities, kind) = tracker();
    let closer = work.clone();
    let end = runtime().block_on(activities.start(
        "Forever".to_string(),
        kind,
        |activity: ActivityHandle| async move {
            closer.close();
            std::future::pending::<()>().await;
            // Never reached: the handle would outlive the activity.
            activity.progress(None).await
        },
    ));
    assert!(matches!(end, Ok(ActivityEnd::Cancelled)));
    assert_eq!(only_activity(&db).status, ActivityStatus::Cancelled);
    // The activity counted as work only while it ran.
    assert!(work.pending().is_empty());
}

#[test]
fn a_report_after_the_end_leaves_the_outcome() {
    let (db, _work, activities, kind) = tracker();
    let mut kept = None;
    let end = runtime().block_on(activities.start(
        "Research".to_string(),
        kind,
        |activity: ActivityHandle| {
            kept = Some(activity);
            async { Ok::<_, ()>(()) }
        },
    ));
    assert!(matches!(end, Ok(ActivityEnd::Finished(Ok(())))));
    let activity = kept.unwrap();
    runtime()
        .block_on(async {
            activity.standing(ActivityStanding::Paused).await?;
            activity.progress(Some("late".to_string())).await
        })
        .unwrap();
    let ended = only_activity(&db);
    assert_eq!(ended.status, ActivityStatus::Succeeded);
    assert_eq!(ended.progress, None);
}
