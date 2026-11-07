use cohesion_core::TaskStatus;
use cohesion_core::model::HookKind;
use cohesion_db::{
    ChangeTaskStatusCommand, Container, CreateTaskCommand, CreateWorkspaceCommand, DatabaseContext,
    Secrets,
};
use cohesion_js::ScriptArgument;
use jiff::Zoned;

use super::{dated, passed, until_next_minute};

fn at(datetime: &str) -> Zoned {
    format!("{datetime}[America/Denver]").parse().unwrap()
}

fn kinds(since: &str, now: &str) -> Vec<HookKind> {
    passed(&at(since), &at(now))
        .iter()
        .map(|hook| hook.kind())
        .collect()
}

/// What `onTimer` is called with, if `timer` fires.
fn timer_subject(since: &str, now: &str) -> Option<serde_json::Value> {
    passed(&at(since), &at(now))
        .iter()
        .find(|hook| hook.kind() == HookKind::Timer)
        .map(|hook| match hook.arguments().as_slice() {
            [ScriptArgument::Json(subject)] => subject.clone(),
            _ => panic!("onTimer takes one JSON argument"),
        })
}

fn millis(datetime: &str) -> i64 {
    at(datetime).timestamp().as_millisecond()
}

#[test]
fn fires_each_hook_as_its_time_passes() {
    assert_eq!(
        kinds("2026-10-17T05:59", "2026-10-17T06:00"),
        [HookKind::DayStart, HookKind::Timer]
    );
    assert_eq!(
        kinds("2026-10-17T22:59", "2026-10-17T23:00"),
        [HookKind::DayEnd, HookKind::Timer]
    );
    assert_eq!(
        kinds("2026-10-17T14:59:30", "2026-10-17T15:00:30"),
        [HookKind::Timer]
    );
    assert_eq!(
        kinds("2026-10-17T06:00", "2026-10-17T06:01"),
        [HookKind::Timer]
    );
    assert!(kinds("2026-10-17T06:01:01", "2026-10-17T06:01:59").is_empty());
}

#[test]
fn a_long_sleep_fires_each_hook_once_in_order() {
    assert_eq!(
        kinds("2026-10-17T22:00", "2026-10-18T07:00"),
        [HookKind::DayEnd, HookKind::DayStart, HookKind::Timer]
    );
    assert_eq!(
        kinds("2026-10-14T12:00", "2026-10-17T12:00"),
        [HookKind::DayEnd, HookKind::DayStart, HookKind::Timer]
    );
}

#[test]
fn the_timer_names_the_minutes_it_covers() {
    let subject = timer_subject("2026-10-17T14:59:30", "2026-10-17T15:00:30").unwrap();
    assert_eq!(subject["time"], millis("2026-10-17T15:00"));
    assert_eq!(subject["hour"], 15);
    assert_eq!(subject["minute"], 0);
    assert_eq!(subject["since"], millis("2026-10-17T14:59"));

    let after_sleep = timer_subject("2026-10-17T10:30:10", "2026-10-17T17:10:05").unwrap();
    assert_eq!(after_sleep["time"], millis("2026-10-17T17:10"));
    assert_eq!(after_sleep["minute"], 10);
    assert_eq!(after_sleep["since"], millis("2026-10-17T10:30"));
}

#[test]
fn each_reading_lands_in_the_next_minute() {
    let now = at("2026-10-17T14:59:42.5");
    let next = now.checked_add(until_next_minute(&now)).unwrap();
    assert_eq!(next, at("2026-10-17T15:00"));
}

/// `task:start` and `task:due` fire for the incomplete tasks starting or due
/// on the day, starts first, and a task both starting and due fires both.
#[tokio::test]
async fn a_day_fires_its_tasks_starts_then_dues() {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    let database = DatabaseContext::new(container, Secrets::new(), ());
    let workspace = database
        .send(CreateWorkspaceCommand {
            title: "Home".to_string(),
            emoji: None,
        })
        .await
        .unwrap();
    let mut ids = Vec::new();
    for (title, start_date, due_date) in [
        ("starts", Some("2026-11-02"), None),
        ("due", None, Some("2026-11-02")),
        ("both", Some("2026-11-02"), Some("2026-11-02")),
        ("tomorrow", Some("2026-11-03"), Some("2026-11-03")),
        ("done", Some("2026-11-02"), Some("2026-11-02")),
    ] {
        let task = database
            .send(CreateTaskCommand {
                workspace_id: workspace.id,
                title: title.to_string(),
                folder_id: None,
                priority: None,
                size: None,
                assignee: None,
                description: None,
                start_date: start_date.map(str::to_string),
                due_date: due_date.map(str::to_string),
                tag_ids: Vec::new(),
            })
            .await
            .unwrap();
        ids.push(task.id);
    }
    database
        .send(ChangeTaskStatusCommand {
            workspace_id: workspace.id,
            task_id: ids[4],
            status: TaskStatus::Completed,
        })
        .await
        .unwrap();

    let fired: Vec<(HookKind, i64)> = dated(&database, "2026-11-02".parse().unwrap())
        .await
        .iter()
        .map(|hook| match hook.arguments().as_slice() {
            [ScriptArgument::Task { task_id, .. }] => (hook.kind(), *task_id),
            _ => panic!("a task hook takes its task"),
        })
        .collect();
    assert_eq!(
        fired,
        [
            (HookKind::TaskStart, ids[0]),
            (HookKind::TaskStart, ids[2]),
            (HookKind::TaskDue, ids[1]),
            (HookKind::TaskDue, ids[2]),
        ]
    );
}
