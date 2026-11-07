use cohesion_core::model::HookKind;
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
