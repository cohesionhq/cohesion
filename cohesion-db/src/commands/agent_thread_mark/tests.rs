use cohesion_core::model::{
    AgentThreadMark, Dialect, IntelligenceAgentColor, IntelligenceEndpointPreset, ThreadItem,
};

use crate::test_support::context;
use crate::{
    AddEndpointModelCommand, CreateIntelligenceAgentCommand, CreateIntelligenceEndpointCommand,
    CreateIntelligenceMessageCommand, CreateIntelligenceThreadCommand, DatabaseContext,
    GetIntelligenceAgentCommand, LookupAgentThreadMarkByAgentCommand,
    RefreshAgentThreadMarkCommand, SetAgentContextTokensCommand, SetAgentThreadMarkCommand,
    ShutdownCleanupCommand,
};

/// An agent, a model it runs on and a thread holding `items` messages, with
/// the thread context and context tokens of a live conversation.
fn setup(db: &DatabaseContext, items: usize) -> (i64, i64, i64) {
    let agent = db
        .send_blocking(CreateIntelligenceAgentCommand {
            name: Some("Scout".to_string()),
            color: IntelligenceAgentColor::Teal,
            emoji: None,
            personality: None,
            verbosity: None,
            role: None,
            directive: None,
            team_id: None,
        })
        .unwrap();
    let endpoint = db
        .send_blocking(CreateIntelligenceEndpointCommand {
            name: Some("Local".to_string()),
            api_key: None,
            usage_url: None,
            cache_minutes: None,
            long_cache: None,
            preset: IntelligenceEndpointPreset::Custom {
                provider_identifier: "local".to_string(),
                dialect: Dialect::Messages,
                base_url: "http://localhost:8080/v1".to_string(),
            },
        })
        .unwrap();
    let model = db
        .send_blocking(AddEndpointModelCommand {
            endpoint_id: endpoint.id,
            model_identifier: "model".to_string(),
            model_name: None,
            context_size: None,
            prices: None,
        })
        .unwrap();
    let thread = db
        .send_blocking(CreateIntelligenceThreadCommand {
            system_prompt: "You are Scout.".to_string(),
        })
        .unwrap();
    for _ in 0..items {
        add_item(db, thread.id);
    }
    db.send_blocking(SetAgentContextTokensCommand {
        agent_id: agent.id,
        tokens: Some(1200),
    })
    .unwrap();
    (agent.id, model.id, thread.id)
}

fn add_item(db: &DatabaseContext, thread_id: i64) {
    db.send_blocking(CreateIntelligenceMessageCommand {
        thread_id,
        item: ThreadItem::UserMessage {
            text: "item".to_string(),
        },
        images: Vec::new(),
    })
    .unwrap();
}

fn mark(
    agent_id: i64,
    model_id: i64,
    thread_id: i64,
    items: i64,
    at: i64,
) -> SetAgentThreadMarkCommand {
    SetAgentThreadMarkCommand {
        agent_id,
        thread_id,
        items,
        endpoint_model_id: model_id,
        extended_from: 1,
        heard_through: 9,
        memory_limit: 20,
        memories: Vec::new(),
        last_call_at: at,
        prompt_tokens: 1200,
        cache_seconds: 3600,
    }
}

fn lookup(db: &DatabaseContext, agent_id: i64) -> Option<AgentThreadMark> {
    db.send_blocking(LookupAgentThreadMarkByAgentCommand { agent_id })
        .unwrap()
}

#[test]
fn a_mark_replaces_the_last_and_a_read_only_moves_it_forward() {
    let db = context();
    let (agent, model, thread) = setup(&db, 4);
    db.send_blocking(mark(agent, model, thread, 2, 1_000))
        .unwrap();
    db.send_blocking(mark(agent, model, thread, 4, 2_000))
        .unwrap();
    let refresh = |items, read_at| {
        db.send_blocking(RefreshAgentThreadMarkCommand {
            agent_id: agent,
            thread_id: thread,
            items,
            read_at,
        })
        .unwrap()
    };
    let at = || lookup(&db, agent).unwrap().last_call_at;
    assert_eq!(lookup(&db, agent).unwrap().items, 4);

    // A fork that read an older mark kept only that older prefix alive.
    refresh(2, 5_000);
    assert_eq!(at(), 2_000);
    refresh(4, 5_000);
    assert_eq!(at(), 5_000);
    refresh(4, 3_000);
    assert_eq!(at(), 5_000);
}

#[test]
fn a_restart_keeps_a_whole_thread_and_forgets_one_cut_short() {
    let db = context();
    let (agent, model, thread) = setup(&db, 4);
    db.send_blocking(mark(agent, model, thread, 4, 1_000))
        .unwrap();
    let context_of = || {
        let agent = db
            .send_blocking(GetIntelligenceAgentCommand { agent_id: agent })
            .unwrap();
        agent.context_tokens
    };

    db.send_blocking(ShutdownCleanupCommand).unwrap();
    assert!(lookup(&db, agent).is_some());
    assert_eq!(context_of(), Some(1200));

    // A turn began after the mark and the daemon stopped during it.
    add_item(&db, thread);
    db.send_blocking(ShutdownCleanupCommand).unwrap();
    assert!(lookup(&db, agent).is_none());
    assert_eq!(context_of(), None);
}
