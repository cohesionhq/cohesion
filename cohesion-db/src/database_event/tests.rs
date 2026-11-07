use super::*;

fn hops(n: usize) -> DatabaseEventTrace {
    (0..n).fold(DatabaseEventTrace::default(), |trace, _| trace.hop())
}

fn deep(n: usize) -> DatabaseEventTrace {
    (0..n).fold(DatabaseEventTrace::default(), |trace, _| trace.deeper())
}

#[test]
fn a_hop_counts_one_and_leaves_the_event_depth_alone() {
    let trace = DatabaseEventTrace::default().deeper().hop();
    assert_eq!((trace.depth(), trace.hops()), (1, 1));
    let deeper = trace.deeper();
    assert_eq!((deeper.depth(), deeper.hops()), (2, 1));
}

#[test]
fn a_follow_up_is_one_hop_from_the_event_that_moved_the_task() {
    // A client moves the task: the event is depth 1, and the fork it wakes
    // is one hop, not another level of depth on top of the event's.
    let moved = DatabaseEventTrace::default()
        .event(DatabaseEventKind::WorkspaceCreated { workspace_id: 1 })
        .trace;
    assert_eq!(moved.depth(), 1);
    let woken = moved.hop();
    assert_eq!((woken.depth(), woken.hops()), (1, 1));
}

#[test]
fn the_hook_limit_is_three_and_hops_never_use_it_up() {
    assert_eq!(DatabaseEventTrace::MAX_DEPTH, 3);
    assert!(!deep(3).is_too_deep());
    assert!(deep(4).is_too_deep());
    assert!(!hops(50).deeper().is_too_deep());
}
