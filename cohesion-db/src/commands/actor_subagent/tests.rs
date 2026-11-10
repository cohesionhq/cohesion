use cohesion_core::LocalIdentity;
use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{
    ActorAnswer, ActorDefinition, ActorQuestion, ActorQuestionOption, ActorToolKind,
    ActorUserContent, ReportedActorSessionStatus,
};
use cohesion_core::protocol::ClientCapabilities;

use super::*;
use crate::commands::actor::SearchActorSessionsCommand;
use crate::test_support::{context, recording_context};
use crate::{
    AddActorSessionMessageCommand, CommandError, CreateActorSessionCommand, DatabaseContext,
    DatabaseEventKind, DeleteActorSessionCommand, IdentifyLocalClientCommand,
    ListActorSessionsCommand, ListActorsCommand, SUBAGENT_MESSAGE_LIMIT, ShutdownCleanupCommand,
    SyncCapabilitiesCommand,
};

/// A client named `name` offering one actor, its id and the actor's.
fn actor(db: &DatabaseContext, name: &str) -> (i64, i64) {
    let client_id = db
        .send_blocking(IdentifyLocalClientCommand {
            identity: LocalIdentity {
                key: name.to_string(),
                name: name.to_string(),
            },
        })
        .unwrap()
        .id;
    let actors = vec![ActorDefinition {
        identifier: "cohesion".to_string(),
        product: "Claude Code".to_string(),
        scope: "/src/cohesion".to_string(),
        modes: Vec::new(),
        models: Vec::new(),
    }];
    db.send_blocking(SyncCapabilitiesCommand {
        client_id,
        offers: ClientCapabilities { actors },
    })
    .unwrap();
    let actor_id = db
        .send_blocking(ListActorsCommand)
        .unwrap()
        .into_iter()
        .find(|actor| actor.client_id == client_id)
        .unwrap()
        .id;
    (client_id, actor_id)
}

fn start(db: &DatabaseContext, actor_id: i64, prompt: &str) -> ActorSession {
    db.send_blocking(CreateActorSessionCommand {
        actor_id,
        prompt: prompt.to_string(),
        title: None,
        mode: None,
        model: None,
        effort: None,
        attachments: Vec::new(),
    })
    .unwrap()
    .0
}

/// A client offering one actor, and a session started on it.
fn session(db: &DatabaseContext) -> ActorSession {
    let (_, actor_id) = actor(db, "MacBook");
    start(db, actor_id, "Fix the build")
}

fn launch_as(title: &str, parent: Option<&str>, message_id: Option<i64>) -> SubagentLaunch {
    SubagentLaunch {
        title: title.to_string(),
        parent: parent.map(str::to_string),
        message_id,
        agent_type: Some("Explore".to_string()),
        model: Some("haiku".to_string()),
        background: false,
    }
}

fn launch(
    db: &DatabaseContext,
    session_id: i64,
    key: &str,
    launch: SubagentLaunch,
) -> Result<ActorSession, CommandError> {
    db.send_blocking(LaunchActorSubagentCommand {
        session_id,
        key: key.to_string(),
        launch,
    })
}

fn say(
    db: &DatabaseContext,
    session_id: i64,
    content: ActorSessionMessageContent,
) -> Result<ActorSessionMessage, CommandError> {
    db.send_blocking(AddActorSessionMessageCommand {
        session_id,
        content,
        attachments: Vec::new(),
    })
}

fn tool_use(db: &DatabaseContext, session_id: i64) -> i64 {
    let content = ActorSessionMessageContent::Actor(ActorMessageContent::ToolUse {
        tool: "Task".to_string(),
        category: ActorToolKind::Agent,
        summary: "Map the db layer".to_string(),
        ended_at: None,
    });
    say(db, session_id, content).unwrap().id
}

fn question() -> ActorSessionMessageContent {
    ActorSessionMessageContent::Actor(ActorMessageContent::Question {
        questions: vec![ActorQuestion {
            header: None,
            question: "Which?".to_string(),
            options: vec![ActorQuestionOption {
                label: "A".to_string(),
                description: None,
            }],
            multi_select: false,
            allows_other: false,
        }],
    })
}

fn answer(request_id: i64) -> ActorSessionMessageContent {
    ActorSessionMessageContent::User(ActorUserContent::Answers {
        request_id,
        answers: vec![ActorAnswer {
            selected: vec!["A".to_string()],
            other: None,
        }],
    })
}

fn get(db: &DatabaseContext, id: i64) -> ActorSession {
    db.send_blocking(GetActorSessionCommand { id }).unwrap()
}

fn set(db: &DatabaseContext, session_id: i64, status: ReportedActorSessionStatus) {
    db.send_blocking(SetActorSessionStatusCommand {
        session_id,
        moved: ActorSessionMove::reported(status),
    })
    .unwrap();
}

fn refused_on(result: Result<ActorSession, CommandError>, field: &str) {
    match result {
        Err(CommandError::Validation(error)) => {
            assert_eq!(error.field.as_deref(), Some(field), "{error:?}")
        }
        other => panic!("expected a refusal on {field}, got {other:?}"),
    }
}

#[test]
fn a_launch_adds_a_working_subagent_under_its_session_and_out_of_the_lists() {
    let db = context();
    let top = session(&db);
    let tool = tool_use(&db, top.id);
    let subagent = launch(
        &db,
        top.id,
        "a1",
        launch_as("Map the db layer", None, Some(tool)),
    )
    .unwrap();

    assert_eq!(subagent.status, ActorSessionStatus::Working);
    assert_eq!(subagent.actor_id, top.actor_id);
    assert_eq!(subagent.title, "Map the db layer");
    assert_eq!(subagent.model.as_deref(), Some("haiku"));
    assert_eq!(
        subagent.subagent,
        Some(ActorSubagent {
            parent_session_id: top.id,
            key: "a1".to_string(),
            agent_type: Some("Explore".to_string()),
            message_id: Some(tool),
            background: false,
        })
    );
    assert!(top.subagent.is_none());

    let listed = db
        .send_blocking(ListActorSubagentsCommand { session_id: top.id })
        .unwrap();
    assert_eq!(listed, std::slice::from_ref(&subagent));
    let sessions = db
        .send_blocking(ListActorSessionsCommand { actor_id: None })
        .unwrap();
    assert_eq!(sessions.iter().map(|s| s.id).collect::<Vec<_>>(), [top.id]);
    let hits = db
        .send_blocking(SearchActorSessionsCommand {
            query: "\"db layer\"".to_string(),
        })
        .unwrap();
    assert!(hits.is_empty(), "{hits:?}");
}

#[test]
fn a_launch_again_updates_the_subagent_where_it_stands() {
    let db = context();
    let top = session(&db);
    let first = launch(&db, top.id, "a1", launch_as("Look around", None, None)).unwrap();
    set(&db, first.id, ReportedActorSessionStatus::Idle);

    let mut again = launch_as("Look around the db", None, None);
    again.background = true;
    again.agent_type = None;
    let updated = launch(&db, top.id, "a1", again).unwrap();
    assert_eq!(updated.id, first.id);
    assert_eq!(updated.title, "Look around the db");
    assert_eq!(updated.status, ActorSessionStatus::Idle);
    let subagent = updated.subagent.unwrap();
    assert!(subagent.background);
    assert_eq!(subagent.agent_type, None);

    // Under another parent, it would move.
    launch(&db, top.id, "b2", launch_as("Another", None, None)).unwrap();
    refused_on(
        launch(
            &db,
            top.id,
            "a1",
            launch_as("Look around", Some("b2"), None),
        ),
        "parent",
    );
}

#[test]
fn subagents_nest_under_launched_ones_only_so_deep() {
    let db = context();
    let top = session(&db);
    refused_on(
        launch(
            &db,
            top.id,
            "child",
            launch_as("Child", Some("nobody"), None),
        ),
        "parent",
    );
    let mut parent: Option<String> = None;
    let mut last = top.id;
    for depth in 1..=SUBAGENT_DEPTH_LIMIT {
        let key = format!("level-{depth}");
        let launched = launch(&db, top.id, &key, launch_as(&key, parent.as_deref(), None)).unwrap();
        assert_eq!(launched.subagent.unwrap().parent_session_id, last);
        last = launched.id;
        parent = Some(key);
    }
    refused_on(
        launch(
            &db,
            top.id,
            "too-deep",
            launch_as("Too deep", parent.as_deref(), None),
        ),
        "parent",
    );
    // Each level lists only its own.
    let first = db
        .send_blocking(ListActorSubagentsCommand { session_id: top.id })
        .unwrap();
    assert_eq!(first.len(), 1);
}

#[test]
fn a_session_launches_only_so_many_subagents() {
    let db = context();
    let top = session(&db);
    for n in 0..SUBAGENT_LIMIT {
        launch(&db, top.id, &format!("s{n}"), launch_as("Busy", None, None)).unwrap();
    }
    refused_on(
        launch(&db, top.id, "one-more", launch_as("Busy", None, None)),
        "subagent",
    );
    // Launching one already there again still updates it.
    launch(&db, top.id, "s0", launch_as("Still busy", None, None)).unwrap();
}

#[test]
fn a_launch_is_checked() {
    let db = context();
    let top = session(&db);
    refused_on(
        launch(&db, top.id, "  ", launch_as("Blank key", None, None)),
        "subagent",
    );
    refused_on(
        launch(
            &db,
            top.id,
            &"k".repeat(257),
            launch_as("Long key", None, None),
        ),
        "subagent",
    );
    let mut long_type = launch_as("Typed", None, None);
    long_type.agent_type = Some("t".repeat(65));
    refused_on(launch(&db, top.id, "a1", long_type), "agent_type");
    assert!(matches!(
        launch(&db, top.id, "a1", launch_as("   ", None, None)),
        Err(CommandError::Validation(_))
    ));

    // The launching message must be a tool use of the session it runs under.
    let text = say(
        &db,
        top.id,
        ActorSessionMessageContent::Actor(ActorMessageContent::Text {
            text: "On it".to_string(),
        }),
    )
    .unwrap();
    refused_on(
        launch(&db, top.id, "a1", launch_as("By text", None, Some(text.id))),
        "message_id",
    );
    let other = start(&db, top.actor_id, "Something else");
    let elsewhere = tool_use(&db, other.id);
    refused_on(
        launch(
            &db,
            top.id,
            "a1",
            launch_as("Elsewhere", None, Some(elsewhere)),
        ),
        "message_id",
    );

    // A subagent is named under the session Cohesion started.
    let subagent = launch(&db, top.id, "a1", launch_as("Fine", None, None)).unwrap();
    refused_on(
        launch(
            &db,
            subagent.id,
            "a2",
            launch_as("Under a subagent", None, None),
        ),
        "session_id",
    );
    // And its key is its session's: another session can't take it.
    refused_on(
        launch(&db, other.id, "a1", launch_as("Taken", None, None)),
        "subagent",
    );
}

#[test]
fn a_subagent_is_found_by_its_key_under_its_session_only() {
    let db = context();
    let top = session(&db);
    let other = start(&db, top.actor_id, "Something else");
    let first = launch(&db, top.id, "a1", launch_as("First", None, None)).unwrap();
    let nested = launch(&db, top.id, "a2", launch_as("Nested", Some("a1"), None)).unwrap();

    let found = |session_id, key: &str| {
        db.send_blocking(GetActorSubagentCommand {
            session_id,
            key: key.to_string(),
        })
    };
    assert_eq!(found(top.id, "a1").unwrap().id, first.id);
    assert_eq!(found(top.id, "a2").unwrap().id, nested.id);
    assert!(matches!(
        found(top.id, "zz"),
        Err(CommandError::Validation(_))
    ));
    assert!(matches!(
        found(other.id, "a1"),
        Err(CommandError::Validation(_))
    ));
    assert!(matches!(
        found(first.id, "a2"),
        Err(CommandError::Validation(_))
    ));
    let top_of_nested = db
        .send_blocking(GetTopActorSessionCommand { id: nested.id })
        .unwrap();
    assert_eq!(top_of_nested.id, top.id);
}

#[test]
fn a_subagent_moves_without_an_event_and_touches_the_sessions_above_it() {
    let (db, events) = recording_context();
    let top = session(&db);
    let first = launch(&db, top.id, "a1", launch_as("First", None, None)).unwrap();
    let nested = launch(&db, top.id, "a2", launch_as("Nested", Some("a1"), None)).unwrap();
    events.take();
    let mut batches = db.subscribe();

    let moved = db
        .send_blocking(SetActorSessionStatusCommand {
            session_id: nested.id,
            moved: ActorSessionMove::Finished,
        })
        .unwrap();
    assert_eq!(moved.status, ActorSessionStatus::Idle);
    assert!(
        events
            .take()
            .iter()
            .all(|event| !matches!(event, DatabaseEventKind::ActorSessionMoved { .. }))
    );
    let mut keys = Vec::new();
    while let Ok(batch) = batches.try_recv() {
        keys.extend(batch.keys.iter().cloned());
    }
    keys.sort_by_key(|key| format!("{key:?}"));
    let mut expected = vec![
        Invalidation::ActorSession(nested.id),
        Invalidation::ActorSession(first.id),
        Invalidation::ActorSession(top.id),
    ];
    expected.sort_by_key(|key| format!("{key:?}"));
    assert_eq!(keys, expected);
}

#[test]
fn a_subagent_takes_only_responses_from_the_user_and_so_many_messages() {
    let db = context();
    let top = session(&db);
    let subagent = launch(&db, top.id, "a1", launch_as("First", None, None)).unwrap();
    let text = ActorSessionMessageContent::User(ActorUserContent::Text {
        text: "Hurry up".to_string(),
    });
    assert!(matches!(
        say(&db, subagent.id, text),
        Err(CommandError::Validation(_))
    ));
    let asked = say(&db, subagent.id, question()).unwrap();
    say(&db, subagent.id, answer(asked.id)).unwrap();

    let thought = ActorSessionMessageContent::Actor(ActorMessageContent::Text {
        text: "Hmm".to_string(),
    });
    for _ in 2..SUBAGENT_MESSAGE_LIMIT {
        say(&db, subagent.id, thought.clone()).unwrap();
    }
    assert!(matches!(
        say(&db, subagent.id, thought.clone()),
        Err(CommandError::Validation(_))
    ));
    // The session Cohesion started has no such limit.
    say(&db, top.id, thought).unwrap();
}

#[test]
fn the_requests_waiting_in_a_sessions_subagents_are_listed_until_answered() {
    let db = context();
    let top = session(&db);
    let first = launch(&db, top.id, "a1", launch_as("First", None, None)).unwrap();
    let nested = launch(&db, top.id, "a2", launch_as("Nested", Some("a1"), None)).unwrap();
    let done = launch(&db, top.id, "a3", launch_as("Done", None, None)).unwrap();
    // The session's own request isn't one of its subagents'.
    say(&db, top.id, question()).unwrap();
    let asked = say(&db, first.id, question()).unwrap();
    let deeper = say(&db, nested.id, question()).unwrap();
    say(&db, done.id, question()).unwrap();
    set(&db, done.id, ReportedActorSessionStatus::Idle);

    let waiting = |session_id| {
        db.send_blocking(ListUnansweredSubagentRequestsCommand { session_id })
            .unwrap()
            .into_iter()
            .map(|message| message.id)
            .collect::<Vec<_>>()
    };
    assert_eq!(waiting(top.id), [asked.id, deeper.id]);
    assert_eq!(waiting(first.id), [deeper.id]);

    say(&db, first.id, answer(asked.id)).unwrap();
    assert_eq!(waiting(top.id), [deeper.id]);
}

#[test]
fn subagents_still_going_settle_by_session_by_client_and_with_the_daemon() {
    let db = context();
    let (_, mac_actor) = actor(&db, "MacBook");
    let (studio, studio_actor) = actor(&db, "Studio");
    let mac_top = start(&db, mac_actor, "On the Mac");
    let studio_top = start(&db, studio_actor, "On the Studio");
    let mac_sub = launch(&db, mac_top.id, "m1", launch_as("Mac", None, None)).unwrap();
    let mac_nested = launch(&db, mac_top.id, "m2", launch_as("Deeper", Some("m1"), None)).unwrap();
    let mac_done = launch(&db, mac_top.id, "m3", launch_as("Done", None, None)).unwrap();
    set(&db, mac_done.id, ReportedActorSessionStatus::Failed);
    let studio_sub = launch(&db, studio_top.id, "s1", launch_as("Studio", None, None)).unwrap();

    db.send_blocking(SettleActorSubagentsCommand {
        of: SubagentsOf::Session(mac_top.id),
        moved: ActorSessionMove::Stopped,
    })
    .unwrap();
    assert_eq!(get(&db, mac_sub.id).status, ActorSessionStatus::Idle);
    assert_eq!(get(&db, mac_nested.id).status, ActorSessionStatus::Idle);
    assert_eq!(get(&db, mac_done.id).status, ActorSessionStatus::Failed);
    assert_eq!(get(&db, studio_sub.id).status, ActorSessionStatus::Working);
    // The session itself is the caller's to move.
    assert_eq!(get(&db, mac_top.id).status, ActorSessionStatus::Working);

    let studio_later = launch(&db, studio_top.id, "s2", launch_as("Later", None, None)).unwrap();
    set(&db, mac_sub.id, ReportedActorSessionStatus::Working);
    db.send_blocking(SettleActorSubagentsCommand {
        of: SubagentsOf::Client(studio),
        moved: ActorSessionMove::Interrupted,
    })
    .unwrap();
    assert_eq!(get(&db, studio_sub.id).status, ActorSessionStatus::Idle);
    assert_eq!(get(&db, studio_later.id).status, ActorSessionStatus::Idle);
    assert_eq!(get(&db, studio_top.id).status, ActorSessionStatus::Working);
    assert_eq!(get(&db, mac_sub.id).status, ActorSessionStatus::Working);

    db.send_blocking(ShutdownCleanupCommand).unwrap();
    assert_eq!(get(&db, mac_sub.id).status, ActorSessionStatus::Idle);
}

#[test]
fn subagents_go_with_their_session() {
    let db = context();
    let top = session(&db);
    let first = launch(&db, top.id, "a1", launch_as("First", None, None)).unwrap();
    let nested = launch(&db, top.id, "a2", launch_as("Nested", Some("a1"), None)).unwrap();
    say(&db, nested.id, question()).unwrap();

    db.send_blocking(DeleteActorSessionCommand { id: top.id })
        .unwrap();
    for id in [first.id, nested.id] {
        assert!(matches!(
            db.send_blocking(GetActorSessionCommand { id }),
            Err(CommandError::NotFound(_))
        ));
    }
    // Its key is free again.
    let again = start(&db, top.actor_id, "Again");
    launch(&db, again.id, "a1", launch_as("First", None, None)).unwrap();
}

#[test]
fn a_family_is_its_top_session_then_each_subagent_after_the_one_it_runs_under() {
    let db = context();
    let top = session(&db);
    let first = launch(&db, top.id, "a1", launch_as("First", None, None)).unwrap();
    let second = launch(&db, top.id, "a2", launch_as("Second", None, None)).unwrap();
    let nested = launch(&db, top.id, "a3", launch_as("Nested", Some("a1"), None)).unwrap();
    let deeper = launch(&db, top.id, "a4", launch_as("Deeper", Some("a3"), None)).unwrap();
    let other = start(&db, top.actor_id, "Something else");
    launch(&db, other.id, "b1", launch_as("Elsewhere", None, None)).unwrap();

    let family = |session_id| {
        db.send_blocking(ListActorSessionFamilyCommand { session_id })
            .unwrap()
            .into_iter()
            .map(|session| session.id)
            .collect::<Vec<_>>()
    };
    let expected = [top.id, first.id, nested.id, deeper.id, second.id];
    // The same from any member of it.
    assert_eq!(family(top.id), expected);
    assert_eq!(family(deeper.id), expected);
    assert_eq!(family(second.id), expected);
    // A session with no subagents is a family of one.
    let alone = start(&db, top.actor_id, "Alone");
    assert_eq!(family(alone.id), [alone.id]);
}
