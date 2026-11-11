use std::sync::Arc;

use cohesion_core::model::{
    AGENDA_ITEM_CHARS, AgentAgendaChange, AgentMessageContent, Initiative, IntelligenceAgentColor,
};
use cohesion_db::{
    Container, CreateIntelligenceAgentCommand, DatabaseContext, DatabaseEventTrace,
    ListAgentMessagesCommand, Secrets,
};

use super::super::agent_tool::{AgentTool, Seat};
use super::super::conversation_session::Branch;
use super::super::foreground_turn::ForegroundTurn;
use super::{AgendaTool, AgendaToolKind, Agendas};
use crate::tool_definition::FieldKind;
use crate::transcript_item::ToolCallStatus;
use crate::turn_input::TurnInput;

fn database() -> (DatabaseContext, i64) {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    let db = DatabaseContext::new(container, Secrets::new(), ());
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
    (db, agent.id)
}

fn seat(turn: Arc<ForegroundTurn>) -> Seat {
    Seat {
        turn,
        branch: Branch::Conversation {
            sent: Arc::default(),
            reacted: Arc::default(),
        },
    }
}

fn tool(db: &DatabaseContext, agent_id: i64, kind: AgendaToolKind) -> AgendaTool {
    AgendaTool {
        database: db.clone(),
        agent_id,
        kind,
    }
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

/// The text of the prompts `turn` takes in before its next model call.
fn taken(turn: &ForegroundTurn) -> Vec<String> {
    turn.take().into_iter().map(|prompt| prompt.text).collect()
}

// One thread's change reaches the agent's other running thread once, before
// its next model call; the thread that made it already has it.
#[test]
fn a_change_reaches_the_other_running_threads_once() {
    let (db, agent_id) = database();
    let agendas = Agendas::default();
    let conversation = Arc::new(ForegroundTurn::new(agendas.of(agent_id)));
    let fork = Arc::new(ForegroundTurn::new(agendas.of(agent_id)));
    let elsewhere = Arc::new(ForegroundTurn::new(agendas.of(agent_id + 1)));
    for turn in [&conversation, &fork, &elsewhere] {
        turn.begin(DatabaseEventTrace::default());
        // Each opens with the agenda in its status.
        assert_eq!(turn.agenda().hear(&[]), "Your agenda is empty.");
    }
    let append = tool(&db, agent_id, AgendaToolKind::Append);
    let finish = tool(&db, agent_id, AgendaToolKind::Finish);
    runtime().block_on(async {
        let call = serde_json::json!({ "items": ["Draft the outline", "Send it"] }).to_string();
        let ToolCallStatus::Success(result) = append.call(&seat(conversation.clone()), &call).await
        else {
            panic!("agenda_append failed");
        };
        assert_eq!(
            result,
            "Your agenda:\n1. [ ] Draft the outline\n2. [ ] Send it"
        );

        assert_eq!(taken(&conversation), Vec::<String>::new());
        assert_eq!(taken(&elsewhere), Vec::<String>::new());
        assert_eq!(
            taken(&fork),
            [
                "# Agenda changed\nAnother thread of yours changed your agenda. Your agenda:\n\
              1. [ ] Draft the outline\n2. [ ] Send it"
            ]
        );
        assert_eq!(taken(&fork), Vec::<String>::new());

        let ToolCallStatus::Success(result) = finish.call(&seat(fork.clone()), "{}").await else {
            panic!("agenda_finish failed");
        };
        assert_eq!(
            result,
            "Your agenda:\n1. [x] Draft the outline\n2. [ ] Send it"
        );
        assert_eq!(taken(&conversation).len(), 1);
        assert_eq!(taken(&fork), Vec::<String>::new());
    });
}

// A turn an event started reads but changes none of the user's data, yet
// keeps its agenda like a note.
#[test]
fn a_turn_an_event_started_keeps_the_agenda() {
    let (db, agent_id) = database();
    let turn = Arc::new(ForegroundTurn::default());
    turn.begin(DatabaseEventTrace::default().started_by(Initiative::Event));
    runtime().block_on(async {
        let seat = seat(turn);
        let call = serde_json::json!({ "items": ["Check the build"] }).to_string();
        let appended = tool(&db, agent_id, AgendaToolKind::Append)
            .call(&seat, &call)
            .await;
        assert!(
            matches!(appended, ToolCallStatus::Success(_)),
            "{appended:?}"
        );
        let reset = tool(&db, agent_id, AgendaToolKind::Reset)
            .call(&seat, "{}")
            .await;
        assert!(
            matches!(&reset, ToolCallStatus::Success(text) if text == "Your agenda is empty."),
            "{reset:?}"
        );

        let feed = db
            .send(ListAgentMessagesCommand {
                agent_id,
                before: None,
                limit: 10,
            })
            .await
            .unwrap();
        let changes: Vec<_> = feed
            .iter()
            .map(|message| match &message.message_content {
                AgentMessageContent::Agenda(update) => update.change,
                other => panic!("an agenda line: {other:?}"),
            })
            .collect();
        assert_eq!(
            changes,
            [AgentAgendaChange::Appended, AgentAgendaChange::Reset]
        );
    });
}

// What the model can't do is refused with a reason it can act on.
#[test]
fn a_bad_call_is_refused_with_why() {
    let (db, agent_id) = database();
    runtime().block_on(async {
        let seat = seat(Arc::default());
        let finished = tool(&db, agent_id, AgendaToolKind::Finish)
            .call(&seat, r#"{"pos": 1}"#)
            .await;
        assert!(
            matches!(&finished, ToolCallStatus::Failure(why)
                if why == "The agenda has no item 1; it has 0."),
            "{finished:?}"
        );
        let past = tool(&db, agent_id, AgendaToolKind::Append)
            .call(&seat, r#"{"items": ["Draft"], "pos": 3}"#)
            .await;
        assert!(
            matches!(&past, ToolCallStatus::Failure(why)
                if why == "Position 3 is outside the agenda, which has 0 items; use 1 to 1."),
            "{past:?}"
        );
    });
}

#[test]
fn the_item_field_states_the_item_limit() {
    let (db, agent_id) = database();
    let definition = tool(&db, agent_id, AgendaToolKind::Append).definition();
    let items = &definition.fields[0];
    assert!(matches!(items.kind, FieldKind::StringList));
    assert!(
        items
            .description
            .contains(&format!("at most {AGENDA_ITEM_CHARS} characters")),
        "{}",
        items.description
    );
}

// A thread that never heard the agenda has nothing to correct while it is
// empty, and nothing at all before anyone read or changed it.
#[test]
fn a_thread_that_never_heard_hears_only_items() {
    let agendas = Agendas::default();
    let turn = ForegroundTurn::new(agendas.of(1));
    assert_eq!(turn.agenda().news(), None);
    ForegroundTurn::new(agendas.of(1)).agenda().hear(&[]);
    assert_eq!(turn.agenda().news(), None);
}
