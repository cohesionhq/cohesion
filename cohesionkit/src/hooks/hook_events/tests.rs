use cohesion_core::model::{ActorSession, ActorSessionMove, ActorSessionStatus, HookKind};
use cohesion_db::DatabaseEventKind;
use cohesion_js::ScriptArgument;

use super::event_hook;

#[test]
fn a_changed_assignee_raises_task_assigned_with_the_task() {
    let hook = event_hook(DatabaseEventKind::TaskAssigneeChanged {
        task_id: 7,
        workspace_id: 3,
    });
    assert_eq!(hook.kind(), HookKind::TaskAssigned);
    assert!(matches!(
        hook.arguments().as_slice(),
        [ScriptArgument::Task {
            task_id: 7,
            workspace_id: 3
        }]
    ));
}

fn session() -> ActorSession {
    ActorSession {
        id: 4,
        actor_id: 1,
        title: "Fix the build".to_string(),
        mode: None,
        model: None,
        effort: None,
        started_at: 10,
        status_changed_at: 10,
        status: ActorSessionStatus::Working,
        progress: Some("Building".to_string()),
        unread: false,
        read_at: None,
        context: None,
        follower: None,
        subagent: None,
    }
}

#[test]
fn a_session_started_or_progressed_calls_its_hook_with_the_session_moved_nowhere() {
    let started = event_hook(DatabaseEventKind::ActorSessionStarted {
        actor_session: session(),
    });
    let progressed = event_hook(DatabaseEventKind::ActorSessionProgressed {
        actor_session: session(),
    });
    for (hook, kind, function) in [
        (started, HookKind::ActorStarted, "onActorStarted"),
        (progressed, HookKind::ActorProgress, "onActorProgress"),
    ] {
        assert_eq!(hook.kind(), kind);
        assert_eq!(hook.function_name(), function);
        assert!(matches!(
            hook.arguments().as_slice(),
            [ScriptArgument::ActorSession { actor_session, moved: None }]
                if *actor_session == session()
        ));
    }
}

#[test]
fn a_session_moving_calls_on_actor_moved_with_why() {
    let hook = event_hook(DatabaseEventKind::ActorSessionMoved {
        actor_session: session(),
        moved: ActorSessionMove::Working,
    });
    assert_eq!(hook.function_name(), "onActorMoved");
    assert!(matches!(
        hook.arguments().as_slice(),
        [ScriptArgument::ActorSession {
            moved: Some(ActorSessionMove::Working),
            ..
        }]
    ));
}
