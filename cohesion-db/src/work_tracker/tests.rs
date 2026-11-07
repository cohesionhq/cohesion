use cohesion_core::protocol::ShuttingDown;

use crate::WorkTracker;

#[test]
fn closing_refuses_new_work_but_admits_follow_up_work() {
    let work = WorkTracker::default();
    let request = work.begin_new("request").unwrap();
    work.close();
    assert_eq!(work.begin_new("another request").err(), Some(ShuttingDown));
    let hook = work.begin("hook");
    drop(request);
    let labels: Vec<_> = work.pending().into_iter().map(|w| w.label).collect();
    assert_eq!(labels, ["hook"]);
    drop(hook);
    assert!(work.pending().is_empty());
}

#[test]
fn idle_waits_for_work_started_by_running_work() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    runtime.block_on(async {
        let work = WorkTracker::default();
        let request = work.begin("request");
        let tracker = work.clone();
        tokio::spawn(async move {
            // The request hands off to a hook before it ends.
            let hook = tracker.begin("hook");
            drop(request);
            tokio::task::yield_now().await;
            drop(hook);
        });
        work.idle().await;
        assert!(work.pending().is_empty());
    });
}

#[test]
fn run_new_holds_work_across_the_future_and_refuses_once_closed() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    runtime.block_on(async {
        let work = WorkTracker::default();
        let during = work
            .run_new("request", async { work.pending().len() })
            .await;
        assert_eq!(during, Ok(1));
        assert!(work.pending().is_empty());
        work.close();
        let refused = work
            .run_new("another request", async {
                unreachable!("refused work never runs")
            })
            .await;
        assert_eq!(refused, Err(ShuttingDown));
        assert!(work.pending().is_empty());
    });
}
