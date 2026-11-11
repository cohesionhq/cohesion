use cohesion_core::model::{
    AgentAgendaChange, AgentMessageContent, IntelligenceAgent, IntelligenceAgentColor,
};

use crate::DatabaseContext as TestContext;
use crate::test_support::context;
use crate::{
    AppendAgentAgendaCommand, CommandError, CreateIntelligenceAgentCommand,
    FinishAgentAgendaItemCommand, GetIntelligenceAgentCommand, ListAgentAgendaCommand,
    ListAgentMessagesCommand, ResetAgentAgendaCommand,
};

fn agent(db: &TestContext) -> IntelligenceAgent {
    db.send_blocking(CreateIntelligenceAgentCommand {
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
}

fn append(db: &TestContext, agent_id: i64, items: &[&str], position: Option<i64>) {
    db.send_blocking(AppendAgentAgendaCommand {
        agent_id,
        run_id: None,
        items: items.iter().map(|item| item.to_string()).collect(),
        position,
    })
    .unwrap();
}

fn texts(db: &TestContext, agent_id: i64) -> Vec<String> {
    db.send_blocking(ListAgentAgendaCommand { agent_id })
        .unwrap()
        .into_iter()
        .map(|item| item.text)
        .collect()
}

#[test]
fn items_append_at_the_end_or_at_a_position() {
    let db = context();
    let scout = agent(&db);
    append(&db, scout.id, &["Draft", "Send"], None);
    append(&db, scout.id, &["Review  the\n draft"], Some(2));
    assert_eq!(texts(&db, scout.id), ["Draft", "Review the draft", "Send"]);
    let positions: Vec<i64> = db
        .send_blocking(ListAgentAgendaCommand { agent_id: scout.id })
        .unwrap()
        .iter()
        .map(|item| item.position)
        .collect();
    assert_eq!(positions, [1, 2, 3]);
}

#[test]
fn appending_refuses_a_position_past_the_end_and_too_many_items() {
    let db = context();
    let scout = agent(&db);
    append(&db, scout.id, &["Draft"], None);
    let past = db.send_blocking(AppendAgentAgendaCommand {
        agent_id: scout.id,
        run_id: None,
        items: vec!["Send".to_string()],
        position: Some(3),
    });
    assert!(matches!(past, Err(CommandError::Validation(_))), "{past:?}");
    let many = db.send_blocking(AppendAgentAgendaCommand {
        agent_id: scout.id,
        run_id: None,
        items: (0..20).map(|n| format!("Step {n}")).collect(),
        position: None,
    });
    assert!(matches!(many, Err(CommandError::Validation(_))), "{many:?}");
    let blank = db.send_blocking(AppendAgentAgendaCommand {
        agent_id: scout.id,
        run_id: None,
        items: vec!["  ".to_string()],
        position: None,
    });
    assert!(
        matches!(blank, Err(CommandError::Validation(_))),
        "{blank:?}"
    );
    assert_eq!(texts(&db, scout.id), ["Draft"]);
}

#[test]
fn finishing_takes_the_first_open_item_or_the_one_named() {
    let db = context();
    let scout = agent(&db);
    append(&db, scout.id, &["Draft", "Review", "Send"], None);
    let finish = |position| {
        db.send_blocking(FinishAgentAgendaItemCommand {
            agent_id: scout.id,
            run_id: None,
            position,
        })
    };
    let edit = finish(Some(2)).unwrap();
    assert_eq!(edit.update.changed_text(), "Review");
    let edit = finish(None).unwrap();
    assert_eq!(edit.update.changed_text(), "Draft");
    let done: Vec<bool> = edit
        .agenda
        .iter()
        .map(|item| item.finished_at.is_some())
        .collect();
    assert_eq!(done, [true, true, false]);
    assert!(matches!(finish(Some(1)), Err(CommandError::Validation(_))));
    assert!(matches!(finish(Some(4)), Err(CommandError::Validation(_))));
    finish(None).unwrap();
    assert!(matches!(finish(None), Err(CommandError::Validation(_))));
}

#[test]
fn each_change_posts_a_quiet_agenda_line() {
    let db = context();
    let scout = agent(&db);
    append(&db, scout.id, &["Draft", "Send"], None);
    db.send_blocking(FinishAgentAgendaItemCommand {
        agent_id: scout.id,
        run_id: None,
        position: None,
    })
    .unwrap();
    db.send_blocking(ResetAgentAgendaCommand {
        agent_id: scout.id,
        run_id: None,
    })
    .unwrap();
    assert!(texts(&db, scout.id).is_empty());
    // An empty agenda has nothing to clear, and says nothing.
    db.send_blocking(ResetAgentAgendaCommand {
        agent_id: scout.id,
        run_id: None,
    })
    .unwrap();

    let messages = db
        .send_blocking(ListAgentMessagesCommand {
            agent_id: scout.id,
            before: None,
            limit: 10,
        })
        .unwrap();
    let lines: Vec<_> = messages
        .iter()
        .map(|message| {
            let AgentMessageContent::Agenda(update) = &message.message_content else {
                panic!("an agenda line: {:?}", message.message_content);
            };
            (
                update.change,
                message.excerpt_verb.clone().unwrap(),
                message.excerpt.clone(),
            )
        })
        .collect();
    assert_eq!(
        lines,
        [
            (
                AgentAgendaChange::Appended,
                "Added to the agenda".to_string(),
                "Draft; Send".to_string()
            ),
            (
                AgentAgendaChange::Finished,
                "Finished".to_string(),
                "Draft".to_string()
            ),
            (
                AgentAgendaChange::Reset,
                "Cleared the agenda".to_string(),
                "Draft; Send".to_string()
            ),
        ]
    );
    // The agent's bookkeeping leaves it read.
    let scout = db
        .send_blocking(GetIntelligenceAgentCommand { agent_id: scout.id })
        .unwrap();
    assert!(!scout.agent_unread);
}
