use cohesion_core::model::{
    AgentAskReply, AgentMessageContent, HookKind, IntelligenceAgentColor, IntelligenceAgentMessage,
    IntelligenceAgentMessageSender,
};

use crate::test_support::context;
use crate::{
    AnswerAgentAskCommand, AskAgentCommand, CommandError, CreateIntelligenceAgentCommand,
    DatabaseContext, DeleteIntelligenceAgentCommand, GetAgentMessageCommand,
    GetIntelligenceAgentCommand, ListAgentRunsCommand, MarkAgentReadCommand,
    ShutdownCleanupCommand,
};

fn agent(db: &DatabaseContext, name: &str) -> i64 {
    let agent = db
        .send_blocking(CreateIntelligenceAgentCommand {
            name: Some(name.to_string()),
            color: IntelligenceAgentColor::Blue,
            emoji: None,
            personality: None,
            verbosity: None,
            role: None,
            directive: None,
            team_id: None,
        })
        .unwrap();
    db.send_blocking(MarkAgentReadCommand { agent_id: agent.id })
        .unwrap();
    agent.id
}

fn ask(db: &DatabaseContext, asker_id: i64, recipient_id: i64) -> Result<(i64, i64), CommandError> {
    db.send_blocking(AskAgentCommand {
        asker_id,
        asker_run_id: None,
        recipient_id,
        question: "Is the build green?".to_string(),
    })
    .map(|record| (record.asked.id, record.asked_by.id))
}

fn reply(db: &DatabaseContext, message_id: i64) -> AgentAskReply {
    match db
        .send_blocking(GetAgentMessageCommand { message_id })
        .unwrap()
        .message_content
    {
        AgentMessageContent::AskedAgent(ask) | AgentMessageContent::AskedByAgent(ask) => ask.reply,
        other => panic!("not a question: {other:?}"),
    }
}

#[test]
fn a_question_stands_in_both_conversations_and_starts_a_run() {
    let db = context();
    let lead = agent(&db, "Lead");
    let app = agent(&db, "App");

    let record = db
        .send_blocking(AskAgentCommand {
            asker_id: lead,
            asker_run_id: None,
            recipient_id: app,
            question: "Is the build green?".to_string(),
        })
        .unwrap();

    let IntelligenceAgentMessage {
        agent_id,
        sender,
        run_id,
        message_content: AgentMessageContent::AskedAgent(asked),
        ..
    } = record.asked
    else {
        panic!("the asker's message is `askedAgent`");
    };
    assert_eq!(
        (agent_id, sender, run_id),
        (lead, IntelligenceAgentMessageSender::Agent, None)
    );
    assert_eq!((asked.agent.id, asked.agent.name.as_str()), (app, "App"));
    assert_eq!(asked.reply, AgentAskReply::Waiting);

    let IntelligenceAgentMessage {
        agent_id,
        sender,
        run_id,
        message_content: AgentMessageContent::AskedByAgent(asked_by),
        ..
    } = record.asked_by
    else {
        panic!("the answerer's message is `askedByAgent`");
    };
    assert_eq!(
        (agent_id, sender, run_id),
        (
            app,
            IntelligenceAgentMessageSender::BackgroundAgent,
            Some(record.run.id)
        )
    );
    assert_eq!(
        (asked_by.agent.id, asked_by.agent.name.as_str()),
        (lead, "Lead")
    );
    assert_eq!(asked_by.question, "Is the build green?");

    let runs = db
        .send_blocking(ListAgentRunsCommand {
            agent_id: app,
            limit: 10,
        })
        .unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].hook, HookKind::AgentMessage);
    assert_eq!(runs[0].cue_name, None);
    assert_eq!(runs[0].sender_agent_id, Some(lead));

    // Neither agent said anything to the user.
    for agent_id in [lead, app] {
        let agent = db
            .send_blocking(GetIntelligenceAgentCommand { agent_id })
            .unwrap();
        assert!(!agent.agent_unread);
    }
}

#[test]
fn an_agent_asks_only_another_agent_that_exists() {
    let db = context();
    let lead = agent(&db, "Lead");

    assert!(matches!(
        ask(&db, lead, lead),
        Err(CommandError::Validation(_))
    ));
    assert!(matches!(
        ask(&db, lead, lead + 1),
        Err(CommandError::NotFound(_))
    ));
    let runs = db
        .send_blocking(ListAgentRunsCommand {
            agent_id: lead,
            limit: 10,
        })
        .unwrap();
    assert!(runs.is_empty());
}

#[test]
fn an_answer_settles_both_messages_once() {
    let db = context();
    let lead = agent(&db, "Lead");
    let app = agent(&db, "App");
    let (asked, asked_by) = ask(&db, lead, app).unwrap();

    let answered = AgentAskReply::Answered {
        text: "Green.".to_string(),
    };
    db.send_blocking(AnswerAgentAskCommand {
        asked,
        asked_by,
        reply: answered.clone(),
    })
    .unwrap();
    // A late reply, such as a timeout racing the answer, changes nothing.
    db.send_blocking(AnswerAgentAskCommand {
        asked,
        asked_by,
        reply: AgentAskReply::Unanswered {
            reason: "Too slow.".to_string(),
        },
    })
    .unwrap();

    assert_eq!(reply(&db, asked), answered);
    assert_eq!(reply(&db, asked_by), answered);
}

#[test]
fn a_stopped_daemon_leaves_no_question_waiting() {
    let db = context();
    let lead = agent(&db, "Lead");
    let app = agent(&db, "App");
    let (asked, asked_by) = ask(&db, lead, app).unwrap();

    db.send_blocking(ShutdownCleanupCommand).unwrap();

    for message_id in [asked, asked_by] {
        assert!(matches!(
            reply(&db, message_id),
            AgentAskReply::Unanswered { .. }
        ));
    }
}

#[test]
fn a_run_outlives_the_agent_that_asked_for_it() {
    let db = context();
    let lead = agent(&db, "Lead");
    let app = agent(&db, "App");
    ask(&db, lead, app).unwrap();

    db.send_blocking(DeleteIntelligenceAgentCommand { agent_id: lead })
        .unwrap();

    let runs = db
        .send_blocking(ListAgentRunsCommand {
            agent_id: app,
            limit: 10,
        })
        .unwrap();
    assert_eq!(runs[0].sender_agent_id, None);
}
