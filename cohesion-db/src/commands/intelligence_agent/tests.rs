use cohesion_core::model::{
    AgentMessageContent, HookKind, IntelligenceAgent, IntelligenceAgentColor,
    IntelligenceAgentMessageSender, IntelligenceAgentPersonality, IntelligenceAgentRunState,
    IntelligenceAgentVerbosity, IntelligenceEffort,
};

use crate::test_support::context;
use crate::{Changed, Filtered, Invalidates};
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, Found, SetAgentPinnedCommand, Write,
};
use crate::{
    CreateAgentMessageCommand, CreateAgentRunCommand, CreateIntelligenceAgentCommand,
    DeleteIntelligenceAgentCommand, FinishAgentRunCommand, GetIntelligenceAgentCommand,
    ListAgentCuesCommand, ListAgentMessagesCommand, ListAgentRunsCommand,
    ListIntelligenceAgentsCommand, ReorderIntelligenceAgentsCommand, SetAgentActivityCommand,
    SetAgentContextTokensCommand, SetAgentCueCommand, SettleAgentActivityCommand,
    UpdateIntelligenceAgentCommand,
};

fn create_agent() -> CreateIntelligenceAgentCommand {
    CreateIntelligenceAgentCommand {
        name: Some("Scout".to_string()),
        color: IntelligenceAgentColor::Teal,
        emoji: None,
        personality: None,
        verbosity: None,
        role: None,
        directive: None,
        team_id: None,
    }
}

#[test]
fn updating_an_agent_keeps_its_emoji_unless_given_one() {
    let db = context();
    let agent = db
        .send_blocking(CreateIntelligenceAgentCommand {
            emoji: Some(" 🔭 ".to_string()),
            ..create_agent()
        })
        .unwrap();
    assert_eq!(agent.agent_emoji.as_deref(), Some("🔭"));
    let update = |emoji: Option<&str>| {
        db.send_blocking(UpdateIntelligenceAgentCommand {
            agent_id: agent.id,
            name: Some("Lookout".to_string()),
            color: None,
            emoji: emoji.map(str::to_string),
            personality: None,
            verbosity: None,
            role: None,
            directive: None,
        })
        .unwrap()
        .agent_emoji
    };
    assert_eq!(update(None).as_deref(), Some("🔭"));
    assert_eq!(update(Some("🌱")).as_deref(), Some("🌱"));
    assert_eq!(update(Some("")), None);
}

#[test]
fn an_agents_role_and_directive_are_trimmed_and_kept_until_changed() {
    let db = context();
    let agent = db
        .send_blocking(CreateIntelligenceAgentCommand {
            role: Some(" Triages my inbox ".to_string()),
            ..create_agent()
        })
        .unwrap();
    assert_eq!(
        (agent.agent_role.as_str(), agent.agent_directive.as_str()),
        ("Triages my inbox", "")
    );
    let update = |role: Option<&str>, directive: Option<&str>| {
        let agent = db
            .send_blocking(UpdateIntelligenceAgentCommand {
                agent_id: agent.id,
                name: None,
                color: None,
                emoji: None,
                personality: None,
                verbosity: None,
                role: role.map(str::to_string),
                directive: directive.map(str::to_string),
            })
            .unwrap();
        (agent.agent_role, agent.agent_directive)
    };
    assert_eq!(
        update(None, Some("Ask before archiving.\n")),
        (
            "Triages my inbox".to_string(),
            "Ask before archiving.".to_string()
        )
    );
    assert_eq!(
        update(Some(""), None),
        (String::new(), "Ask before archiving.".to_string())
    );
}

#[test]
fn an_agent_speaks_balanced_and_brief_until_its_voice_is_changed() {
    let db = context();
    let agent = db.send_blocking(create_agent()).unwrap();
    assert_eq!(
        (agent.agent_personality, agent.agent_verbosity),
        (
            IntelligenceAgentPersonality::Balanced,
            IntelligenceAgentVerbosity::Brief
        )
    );
    let update = |personality, verbosity| {
        let agent = db
            .send_blocking(UpdateIntelligenceAgentCommand {
                agent_id: agent.id,
                name: None,
                color: None,
                emoji: None,
                personality,
                verbosity,
                role: None,
                directive: None,
            })
            .unwrap();
        (agent.agent_personality, agent.agent_verbosity)
    };
    assert_eq!(
        update(Some(IntelligenceAgentPersonality::Candid), None),
        (
            IntelligenceAgentPersonality::Candid,
            IntelligenceAgentVerbosity::Brief
        )
    );
    assert_eq!(
        update(None, Some(IntelligenceAgentVerbosity::Thorough)),
        (
            IntelligenceAgentPersonality::Candid,
            IntelligenceAgentVerbosity::Thorough
        )
    );
}

#[test]
fn deleting_an_agent_removes_its_cues_runs_and_messages() {
    let db = context();
    let agent = db.send_blocking(create_agent()).unwrap();
    db.send_blocking(SetAgentCueCommand {
        agent_id: agent.id,
        name: "Triage".to_string(),
        hook: HookKind::TaskCreate,
        script: "() => true".to_string(),
        instructions: "".to_string(),
        effort: IntelligenceEffort::Medium,
    })
    .unwrap();
    let run = db
        .send_blocking(CreateAgentRunCommand {
            agent_id: agent.id,
            hook: HookKind::TaskCreate,
            cue_name: "Triage".to_string(),
            thread_id: None,
        })
        .unwrap();
    db.send_blocking(CreateAgentMessageCommand {
        agent_id: agent.id,
        run_id: Some(run.id),
        run_description: Some("\"Test task\" was created".to_string()),
        sender: IntelligenceAgentMessageSender::BackgroundAgent,
        content: AgentMessageContent::Text {
            text: "done".to_string(),
        },
        reply_to: None,
    })
    .unwrap();

    db.send_blocking(DeleteIntelligenceAgentCommand { agent_id: agent.id })
        .unwrap();

    assert!(
        db.send_blocking(ListAgentCuesCommand { agent_id: agent.id })
            .unwrap()
            .is_empty()
    );
    assert!(
        db.send_blocking(ListAgentRunsCommand {
            agent_id: agent.id,
            limit: 10,
        })
        .unwrap()
        .is_empty()
    );
    assert!(
        db.send_blocking(ListAgentMessagesCommand {
            agent_id: agent.id,
            before: None,
            limit: 10,
        })
        .unwrap()
        .is_empty()
    );
}

#[test]
fn an_agent_created_without_a_name_gets_an_unused_one() {
    let db = context();
    let unnamed = |name: Option<&str>| {
        db.send_blocking(CreateIntelligenceAgentCommand {
            name: name.map(str::to_string),
            ..create_agent()
        })
        .unwrap()
        .agent_name
    };
    let first = unnamed(None);
    let second = unnamed(Some("  "));
    assert!(!first.is_empty() && !second.is_empty());
    assert!(!first.eq_ignore_ascii_case(&second));
}

#[test]
fn settling_clears_the_activity_a_stopped_daemon_left() {
    let db = context();
    let agent = db.send_blocking(create_agent()).unwrap();
    db.send_blocking(SetAgentActivityCommand {
        agent_id: agent.id,
        activity: Some("Checking the task".to_string()),
    })
    .unwrap();

    db.send_blocking(SettleAgentActivityCommand).unwrap();
    let agent = db
        .send_blocking(GetIntelligenceAgentCommand { agent_id: agent.id })
        .unwrap();
    assert_eq!(agent.activity, None);
}

#[test]
fn an_agents_context_tokens_are_the_live_threads_and_a_settle_clears_an_unmarked_one() {
    let db = context();
    let agent = db.send_blocking(create_agent()).unwrap();
    let tokens = || {
        db.send_blocking(GetIntelligenceAgentCommand { agent_id: agent.id })
            .unwrap()
            .context_tokens
    };
    let record = |value| {
        db.send_blocking(SetAgentContextTokensCommand {
            agent_id: agent.id,
            tokens: value,
        })
        .unwrap()
    };

    record(Some(1200));
    record(Some(1500));
    assert_eq!(tokens(), Some(1500));

    // A thread begun or dropped starts over until its first request is
    // answered.
    record(None);
    assert_eq!(tokens(), None);

    // With no conversation mark, a stopped daemon leaves no count.
    record(Some(400));
    db.send_blocking(SettleAgentActivityCommand).unwrap();
    assert_eq!(tokens(), None);
}

#[test]
fn settling_fails_runs_a_stopped_daemon_left_running() {
    let db = context();
    let agent = db.send_blocking(create_agent()).unwrap();
    let run = |cue_name: &str| {
        db.send_blocking(CreateAgentRunCommand {
            agent_id: agent.id,
            hook: HookKind::TaskCreate,
            cue_name: cue_name.to_string(),
            thread_id: None,
        })
        .unwrap()
    };
    let finished = run("Finished");
    db.send_blocking(FinishAgentRunCommand {
        run_id: finished.id,
        error: None,
    })
    .unwrap();
    let interrupted = run("Interrupted");

    db.send_blocking(SettleAgentActivityCommand).unwrap();
    let runs = db
        .send_blocking(ListAgentRunsCommand {
            agent_id: agent.id,
            limit: 10,
        })
        .unwrap();
    let state = |run_id| {
        runs.iter()
            .find(|run| run.id == run_id)
            .unwrap()
            .state
            .clone()
    };
    assert!(matches!(
        state(finished.id),
        IntelligenceAgentRunState::Completed { .. }
    ));
    assert!(matches!(
        state(interrupted.id),
        IntelligenceAgentRunState::Failed { .. }
    ));
}

#[test]
fn pinned_agents_list_first_in_their_order_then_the_rest_by_latest_message() {
    let db = context();
    let [a, b, c] = ["a", "b", "c"].map(|name| {
        db.send_blocking(CreateIntelligenceAgentCommand {
            name: Some(name.to_string()),
            ..create_agent()
        })
        .unwrap()
    });
    for (agent, ago) in [(&a, 300), (&b, 200), (&c, 100)] {
        db.send_blocking(Backdate {
            agent_id: agent.id,
            ago,
        })
        .unwrap();
    }
    let names = || {
        db.send_blocking(ListIntelligenceAgentsCommand {})
            .unwrap()
            .into_iter()
            .map(|a| a.agent_name)
            .collect::<Vec<_>>()
    };
    let pin = |agent_id, pinned| {
        db.send_blocking(SetAgentPinnedCommand { agent_id, pinned })
            .unwrap()
    };
    let send = |agent_id, content| {
        db.send_blocking(CreateAgentMessageCommand {
            agent_id,
            run_id: None,
            run_description: None,
            sender: IntelligenceAgentMessageSender::User,
            content,
            reply_to: None,
        })
        .unwrap()
    };
    assert_eq!(names(), ["c", "b", "a"]);

    send(
        a.id,
        AgentMessageContent::Text {
            text: "hi".to_string(),
        },
    );
    assert_eq!(names(), ["a", "c", "b"]);
    // A documentation lookup isn't conversation, so it doesn't bring its
    // agent up.
    send(
        b.id,
        AgentMessageContent::Documentation {
            module: "network".to_string(),
        },
    );
    assert_eq!(names(), ["a", "c", "b"]);

    assert!(pin(b.id, true).pinned);
    pin(c.id, true);
    assert_eq!(names(), ["b", "c", "a"]);

    let reordered = db
        .send_blocking(ReorderIntelligenceAgentsCommand {
            agent_ids: vec![c.id, b.id],
        })
        .unwrap();
    assert_eq!(
        reordered
            .into_iter()
            .map(|a| a.agent_name)
            .collect::<Vec<_>>(),
        ["c", "b", "a"]
    );

    assert!(!pin(c.id, false).pinned);
    assert_eq!(names(), ["b", "a", "c"]);
    pin(c.id, true);
    assert_eq!(names(), ["b", "c", "a"]);
}

/// Moves agent `agent_id`'s creation `ago` seconds into the past.
struct Backdate {
    agent_id: i64,
    ago: i64,
}
impl DatabaseCommand for Backdate {
    type Response = IntelligenceAgent;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let agent: IntelligenceAgent = ctx
            .db
            .update()
            .set(IntelligenceAgent::CREATED_AT, ctx.now - self.ago)
            .filter_eq(IntelligenceAgent::ID, self.agent_id)
            .returning_one()
            .found()?;
        Ok(agent.invalidating_nothing())
    }
}
