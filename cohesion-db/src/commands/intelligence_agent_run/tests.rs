use cohesion_core::model::{HookKind, IntelligenceAgentColor, IntelligenceAgentRunState};

use crate::test_support::context;
use crate::{
    CreateAgentRunCommand, CreateIntelligenceAgentCommand, CreateIntelligenceCostEventCommand,
    CreateIntelligenceThreadCommand, DatabaseContext, FinishAgentRunCommand,
};

/// Starts a run of a new agent on a new thread, returning the run's and the
/// thread's ids.
fn start_run(db: &DatabaseContext) -> (i64, i64) {
    let agent = db
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
        .unwrap();
    let thread = db
        .send_blocking(CreateIntelligenceThreadCommand {
            system_prompt: "".to_string(),
        })
        .unwrap();
    let run = db
        .send_blocking(CreateAgentRunCommand {
            agent_id: agent.id,
            hook: HookKind::TaskCreate,
            cue_name: "Triage".to_string(),
            thread_id: Some(thread.id),
        })
        .unwrap();
    (run.id, thread.id)
}

fn finish(db: &DatabaseContext, run_id: i64) -> cohesion_core::model::IntelligenceAgentRun {
    db.send_blocking(FinishAgentRunCommand {
        run_id,
        error: None,
    })
    .unwrap()
}

#[test]
fn finishing_a_run_sums_its_threads_cost_events() {
    let db = context();
    let (run_id, thread_id) = start_run(&db);
    // The second event's provider reported only its total.
    for (input_tokens, output_tokens, cache_read_tokens, parts) in [
        (10, 1, 100, Some([0.5, 0.25, 0.125, 0.0625])),
        (20, 2, 200, None),
    ] {
        db.send_blocking(CreateIntelligenceCostEventCommand {
            thread_id,
            provider_identifier: "anthropic".to_string(),
            model_identifier: "haiku".to_string(),
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_write_tokens: 5,
            total_cost: 0.9375,
            input_cost: parts.map(|parts| parts[0]),
            output_cost: parts.map(|parts| parts[1]),
            cache_read_cost: parts.map(|parts| parts[2]),
            cache_write_cost: parts.map(|parts| parts[3]),
        })
        .unwrap();
    }

    let run = finish(&db, run_id);
    assert_eq!(
        (
            run.input_tokens,
            run.output_tokens,
            run.cache_read_tokens,
            run.cache_write_tokens
        ),
        (30, 3, 300, 10)
    );
    assert_eq!(run.cost, Some(1.875));
    assert!(matches!(
        run.state,
        IntelligenceAgentRunState::Completed { .. }
    ));
}

#[test]
fn a_run_without_cost_events_has_no_cost() {
    let db = context();
    let (run_id, _) = start_run(&db);

    let run = finish(&db, run_id);
    assert_eq!((run.input_tokens, run.output_tokens), (0, 0));
    assert_eq!(run.cost, None);
}
