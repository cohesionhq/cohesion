use std::sync::Arc;

use cohesion_core::model::IntelligenceAgentColor;
use cohesion_db::{
    Container, CreateIntelligenceAgentCommand, DatabaseContext, GetIntelligenceAgentCommand,
    Secrets, WorkTracker,
};
use tokio::sync::{mpsc, oneshot};

use super::*;
use crate::activity::{Activity, ShowActivity};

#[test]
fn activities_hand_back_to_the_work_before_then_clear() {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    let db = DatabaseContext::new(container, Secrets::new(), ());
    let agent_id = db
        .send_blocking(CreateIntelligenceAgentCommand {
            name: Some("Scout".to_string()),
            color: IntelligenceAgentColor::Blue,
            emoji: None,
            personality: None,
            verbosity: None,
            role: None,
            directive: None,
            team_id: None,
        })
        .unwrap()
        .id;
    let shown = async || {
        db.send(GetIntelligenceAgentCommand { agent_id })
            .await
            .unwrap()
            .activity
    };
    let activity = Arc::new(AgentActivity::new(db.clone()));
    let tracker = WorkTracker::default();
    // Work that runs each script it is told to, and ends when told no more.
    let start = |activity: &Arc<AgentActivity>| {
        let (titles, mut told) = mpsc::unbounded_channel::<String>();
        let (done_tx, done) = mpsc::unbounded_channel::<()>();
        let (begun_tx, begun) = oneshot::channel();
        let activity = activity.clone();
        let counted = tracker.begin("test");
        let scope = tokio::spawn(async move {
            activity
                .scope(agent_id, counted, Some(begun_tx), move |work| async move {
                    while let Some(title) = told.recv().await {
                        work.show(Activity::Running(title)).await;
                        done_tx.send(()).unwrap();
                    }
                    Ok(())
                })
                .await
        });
        (titles, done, begun, scope)
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    runtime.block_on(async {
        let (first, mut first_done, begun, first_scope) = start(&activity);
        begun.await.unwrap();
        assert!(shown().await.is_some());
        first.send("Checking the task".to_string()).unwrap();
        first_done.recv().await.unwrap();
        let (second, mut second_done, begun, second_scope) = start(&activity);
        begun.await.unwrap();
        second.send("Drafting a proposal".to_string()).unwrap();
        second_done.recv().await.unwrap();
        assert_eq!(shown().await.as_deref(), Some("Drafting a proposal"));

        // The older work's change stays behind the newer work's.
        first.send("Checking again".to_string()).unwrap();
        first_done.recv().await.unwrap();
        assert_eq!(shown().await.as_deref(), Some("Drafting a proposal"));

        drop(second);
        second_scope.await.unwrap().unwrap();
        assert_eq!(shown().await.as_deref(), Some("Checking again"));
        drop(first);
        first_scope.await.unwrap().unwrap();
        assert_eq!(shown().await, None);
    });
}

// A job that panics still ends its work, so the agent does not stay working.
#[test]
fn a_panicking_job_still_ends_its_work() {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    let db = DatabaseContext::new(container, Secrets::new(), ());
    let agent_id = db
        .send_blocking(CreateIntelligenceAgentCommand {
            name: Some("Scout".to_string()),
            color: IntelligenceAgentColor::Blue,
            emoji: None,
            personality: None,
            verbosity: None,
            role: None,
            directive: None,
            team_id: None,
        })
        .unwrap()
        .id;
    let activity = Arc::new(AgentActivity::new(db.clone()));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    runtime.block_on(async {
        let result = activity
            .scope(
                agent_id,
                WorkTracker::default().begin("test"),
                None,
                |_| async { panic!("the job fails") as Result<(), AgentError> },
            )
            .await;
        assert!(matches!(result, Err(AgentError::Panicked)));
        let agent = db
            .send(GetIntelligenceAgentCommand { agent_id })
            .await
            .unwrap();
        assert_eq!(agent.activity, None);
    });
}
