use std::sync::Arc;
use std::time::Duration;

use cohesion_core::model::{IntelligenceAgentColor, IntelligenceTier};
use cohesion_db::{
    Container, CreateIntelligenceAgentCommand, DatabaseContext, DatabaseEventTrace,
    GetIntelligenceDefaultCommand, Secrets, SetIntelligenceDefaultCommand, WorkTracker,
};
use cohesion_js::ScriptEngine;

use super::*;
use crate::agents::agent_messenger::Ask;
use crate::test_model::{TestModel, texts};

/// Scout and Lead on a coordinator whose sessions run on a model that takes
/// every call and answers none, so whatever starts stays running.
struct Crew {
    model: Arc<TestModel>,
    coordinator: AgentCoordinator,
    scout: i64,
    lead: i64,
}

fn crew() -> Crew {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    let db = DatabaseContext::new(container, Secrets::new(), ());
    let model = TestModel::serve(Vec::new());
    model.store(&db, IntelligenceTier::Agent);
    let endpoint_model_id = db
        .send_blocking(GetIntelligenceDefaultCommand {
            tier: IntelligenceTier::Agent,
        })
        .unwrap()
        .model
        .id;
    db.send_blocking(SetIntelligenceDefaultCommand {
        tier: IntelligenceTier::Low,
        endpoint_model_id,
    })
    .unwrap();
    let agent = |name: &str| {
        db.send_blocking(CreateIntelligenceAgentCommand {
            name: Some(name.to_string()),
            color: IntelligenceAgentColor::Blue,
            emoji: None,
            personality: None,
            verbosity: None,
            role: None,
            directive: None,
            team_id: None,
        })
        .unwrap()
        .id
    };
    let (scout, lead) = (agent("Scout"), agent("Lead"));
    let engine = ScriptEngine::new(db.read_write());
    let coordinator =
        AgentCoordinator::new(db.clone(), engine, model.clone(), WorkTracker::default());
    Crew {
        model,
        coordinator,
        scout,
        lead,
    }
}

impl Crew {
    /// Wakes a fork of `recipient`'s conversation, handing it `job` one way
    /// from the other agent.
    async fn wake(&self, recipient: i64, job: &str) {
        let asker_id = if recipient == self.scout {
            self.lead
        } else {
            self.scout
        };
        self.coordinator
            .inner
            .messenger
            .hand_off(Ask {
                asker_id,
                asker_run_id: None,
                recipient_id: recipient,
                question: job.to_string(),
                trace: DatabaseEventTrace::default(),
            })
            .await
            .unwrap();
    }

    /// Sends Scout's conversation a message from the user.
    async fn say(&self, words: &str) {
        self.coordinator
            .send_message(self.scout, words.to_string(), Vec::new(), None)
            .await
            .unwrap();
    }

    /// Waits for the model to be called `calls` times while every earlier
    /// call is still unanswered, which only work running alongside can do.
    async fn running_together(&self, calls: usize) {
        let mut sent = self.model.calls();
        tokio::time::timeout(
            Duration::from_secs(10),
            sent.wait_for(|sent| *sent >= calls),
        )
        .await
        .expect("each runs while the ones before it still do")
        .unwrap();
    }

    /// The prompt of request `index`: the newest user message in it.
    fn prompt(&self, index: usize) -> String {
        texts(&self.model.sent()[index], "user").pop().unwrap()
    }
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

#[test]
fn two_forks_of_one_agent_run_together() {
    let crew = crew();
    runtime().block_on(async {
        crew.wake(crew.scout, "Job one").await;
        crew.running_together(1).await;

        crew.wake(crew.scout, "Job two").await;
        crew.running_together(2).await;
    });
    assert!(crew.prompt(1).contains("Job two"));
}

#[test]
fn a_running_fork_does_not_hold_up_another_agent() {
    let crew = crew();
    runtime().block_on(async {
        crew.wake(crew.scout, "Job one").await;
        crew.running_together(1).await;

        crew.wake(crew.lead, "Job for the lead").await;
        crew.running_together(2).await;
    });
    assert!(crew.prompt(1).contains("Job for the lead"));
}

#[test]
fn a_running_fork_does_not_hold_up_the_users_turn() {
    let crew = crew();
    runtime().block_on(async {
        crew.wake(crew.scout, "Job one").await;
        crew.running_together(1).await;

        crew.say("Revert it.").await;
        crew.running_together(2).await;
    });
    assert!(crew.prompt(1).contains("Revert it."));
}

#[test]
fn the_users_turn_does_not_hold_up_a_fork() {
    let crew = crew();
    runtime().block_on(async {
        crew.say("Revert it.").await;
        crew.running_together(1).await;

        crew.wake(crew.scout, "Job one").await;
        crew.running_together(2).await;
    });
    assert!(crew.prompt(1).contains("Job one"));
}
