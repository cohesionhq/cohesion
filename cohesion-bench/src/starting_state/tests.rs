use super::*;

// A state survives a trip through JSON, so one could be read from a file.
#[test]
fn a_starting_state_reads_back_from_json() {
    let state = StartingState::empty();
    let json = serde_json::to_string(&state).unwrap();
    assert_eq!(serde_json::from_str::<StartingState>(&json).unwrap(), state);
    assert_eq!(state.workspace_titles(), ["Personal"]);
}

#[test]
fn a_state_in_json_leaves_out_what_it_doesnt_need() {
    let state: StartingState = serde_json::from_str(
        r#"{ "workspaces": [{ "title": "Home", "tasks": [
            { "title": "Sweep", "priority": "high", "due": -1, "status": "completed" },
            { "title": "Mop", "assignee": { "kind": "contact", "firstName": "Ada", "lastName": "L" } }
        ] }] }"#,
    )
    .unwrap();
    let tasks = &state.workspaces[0].tasks;
    assert_eq!(tasks[0].priority, Some(TaskPriority::High));
    assert_eq!(tasks[0].status, Some(TaskStatus::Completed));
    assert_eq!(tasks[0].due, Some(-1));
    assert!(matches!(
        tasks[1].assignee,
        Some(AssigneeState::Contact { .. })
    ));
    assert!(
        serde_json::from_str::<StartingState>(r#"{ "workspaces": [{ "colour": 1 }] }"#).is_err()
    );
}
