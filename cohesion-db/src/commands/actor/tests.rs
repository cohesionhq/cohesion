use cohesion_core::LocalIdentity;
use cohesion_core::model::{
    ActorAnswer, ActorDefinition, ActorMessageContent, ActorMode, ActorModel,
    ActorPermissionDecision, ActorPermissionRequest, ActorPermissionSubject, ActorQuestion,
    ActorQuestionOption, ActorSessionMessageContent, ActorSessionMove, ActorSessionStatus,
    ActorToolKind, ActorUserContent,
};
use cohesion_core::protocol::{ClientCapabilities, ImageUpload};
use cohesion_core::value::read_column;

use super::*;
use crate::test_support::{RawSql, context, recording_context};
use crate::{
    CommandError, DatabaseContext, DeleteClientCommand, IdentifyLocalClientCommand,
    RenameClientCommand, ShutdownCleanupCommand, SyncCapabilitiesCommand,
};

fn client(db: &DatabaseContext) -> i64 {
    db.send_blocking(IdentifyLocalClientCommand {
        identity: LocalIdentity {
            key: "MacBook".to_string(),
            name: "MacBook".to_string(),
        },
    })
    .unwrap()
    .id
}

fn actor(identifier: &str, scope: &str) -> ActorDefinition {
    ActorDefinition {
        identifier: identifier.to_string(),
        product: "Claude Code".to_string(),
        scope: scope.to_string(),
        modes: vec![ActorMode {
            identifier: "auto".to_string(),
            title: "Auto".to_string(),
            description: None,
        }],
        models: vec![ActorModel {
            identifier: "opus".to_string(),
            title: "Opus".to_string(),
            efforts: vec![],
        }],
    }
}

fn offer(db: &DatabaseContext, client_id: i64, actors: Vec<ActorDefinition>) {
    let offers = ClientCapabilities { actors };
    db.send_blocking(SyncCapabilitiesCommand { client_id, offers })
        .unwrap();
}

fn actors(db: &DatabaseContext) -> Vec<Actor> {
    db.send_blocking(ListActorsCommand).unwrap()
}

/// A client offering one actor, and a session started on it.
fn session(db: &DatabaseContext) -> ActorSession {
    let mac = client(db);
    offer(db, mac, vec![actor("cohesion", "/src/cohesion")]);
    let actor_id = actors(db)[0].id;
    db.send_blocking(CreateActorSessionCommand {
        actor_id,
        prompt: "Fix the build\nIt fails on CI.".to_string(),
        title: None,
        mode: None,
        model: None,
        effort: None,
        attachments: Vec::new(),
    })
    .unwrap()
    .0
}

fn status(db: &DatabaseContext, session_id: i64) -> ActorSessionStatus {
    db.send_blocking(GetActorSessionCommand { id: session_id })
        .unwrap()
        .status
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

fn question() -> ActorSessionMessageContent {
    ActorSessionMessageContent::Actor(ActorMessageContent::Question {
        questions: vec![ActorQuestion {
            header: None,
            question: "Which?".to_string(),
            options: vec![
                ActorQuestionOption {
                    label: "A".to_string(),
                    description: None,
                },
                ActorQuestionOption {
                    label: "B".to_string(),
                    description: None,
                },
            ],
            multi_select: false,
            allows_other: false,
        }],
    })
}

fn answers(request_id: i64, selected: &[&str]) -> ActorSessionMessageContent {
    ActorSessionMessageContent::User(ActorUserContent::Answers {
        request_id,
        answers: vec![ActorAnswer {
            selected: selected.iter().map(|label| label.to_string()).collect(),
            other: None,
        }],
    })
}

#[test]
fn an_actor_is_named_after_its_client_and_outlives_its_offer_with_its_sessions() {
    let db = context();
    let session = session(&db);
    let mac = actors(&db)[0].client_id;
    assert_eq!(actors(&db)[0].title, "Claude Code on MacBook");
    db.send_blocking(RenameClientCommand {
        id: mac,
        client_name: "Studio".to_string(),
    })
    .unwrap();
    assert_eq!(actors(&db)[0].title, "Claude Code on Studio");

    // No longer offered, it is hidden, and its session stays.
    offer(&db, mac, vec![]);
    assert!(actors(&db).is_empty());
    db.send_blocking(GetActorSessionCommand { id: session.id })
        .unwrap();

    // Offered again, it is the same actor.
    offer(&db, mac, vec![actor("cohesion", "/src/cohesion")]);
    assert_eq!(actors(&db)[0].id, session.actor_id);

    // Deleting its client hides it too.
    db.send_blocking(DeleteClientCommand { id: mac }).unwrap();
    assert!(actors(&db).is_empty());
}

#[test]
fn a_session_is_titled_by_its_prompt_and_runs_only_in_what_its_actor_offers() {
    let db = context();
    let session = session(&db);
    assert_eq!(session.title, "Fix the build");
    let create = |mode: Option<&str>, model: Option<&str>| {
        db.send_blocking(CreateActorSessionCommand {
            actor_id: session.actor_id,
            prompt: "Go".to_string(),
            title: None,
            mode: mode.map(str::to_string),
            model: model.map(str::to_string),
            effort: None,
            attachments: Vec::new(),
        })
    };
    assert!(create(Some("auto"), Some("opus")).is_ok());
    assert!(create(Some("yolo"), None).is_err());
    assert!(create(None, Some("haiku")).is_err());
    // Opus takes no effort.
    let effort = db.send_blocking(CreateActorSessionCommand {
        actor_id: session.actor_id,
        prompt: "Go".to_string(),
        title: None,
        mode: None,
        model: None,
        effort: Some("high".to_string()),
        attachments: Vec::new(),
    });
    assert!(effort.is_err());
}

#[test]
fn a_session_keeps_the_title_it_is_given_and_its_client_may_retitle_it() {
    let db = context();
    let session = session(&db);
    let create = |title: &str| {
        db.send_blocking(CreateActorSessionCommand {
            actor_id: session.actor_id,
            prompt: "Fix the build\nIt fails on CI.".to_string(),
            title: Some(title.to_string()),
            mode: None,
            model: None,
            effort: None,
            attachments: Vec::new(),
        })
    };
    assert_eq!(create("  CI fix ").unwrap().0.title, "CI fix");
    assert!(create(" \n ").is_err());

    let retitle = |title: String| {
        db.send_blocking(SetActorSessionTitleCommand {
            session_id: session.id,
            title,
        })
    };
    let long = format!("{}\nThe rest.", "Repair ".repeat(20));
    let retitled = retitle(long).unwrap();
    assert!(retitled.title.ends_with('…'));
    assert!(retitled.title.chars().count() <= TITLE_LIMIT + 1);
    assert!(retitle("  ".to_string()).is_err());
    // The search index follows the new title.
    let found = db
        .send_blocking(SearchActorSessionsCommand {
            query: "repair".to_string(),
        })
        .unwrap();
    assert_eq!(
        found.iter().map(|hit| hit.id).collect::<Vec<_>>(),
        [session.id]
    );
}

#[test]
fn a_session_switches_only_to_a_mode_its_actor_offers() {
    let db = context();
    let session = session(&db);
    let switch = |mode: Option<&str>| {
        db.send_blocking(SetActorSessionModeCommand {
            session_id: session.id,
            mode: mode.map(str::to_string),
        })
    };
    assert_eq!(switch(Some("auto")).unwrap().mode.as_deref(), Some("auto"));
    assert!(switch(Some("yolo")).is_err());
    let stored = db
        .send_blocking(GetActorSessionCommand { id: session.id })
        .unwrap();
    assert_eq!(stored.mode.as_deref(), Some("auto"));
    assert_eq!(switch(None).unwrap().mode, None);
}

#[test]
fn a_message_keeps_its_images_beside_it_and_takes_them_when_it_goes() {
    let db = context();
    let session = session(&db);
    let image = |data: &str| ImageUpload {
        media_type: "image/png".to_string(),
        data: data.to_string(),
    };
    let message = db
        .send_blocking(AddActorSessionMessageCommand {
            session_id: session.id,
            content: ActorSessionMessageContent::User(ActorUserContent::Text {
                text: "Like this".to_string(),
            }),
            attachments: vec![image("AAAA"), image("BBBB")],
        })
        .unwrap();
    assert_eq!(message.attachment_count, 2);
    let list = |message_id| {
        db.send_blocking(ListActorSessionMessageAttachmentsCommand {
            message_ids: vec![message_id],
        })
        .unwrap()
    };
    let stored: Vec<String> = list(message.id)
        .into_iter()
        .map(|a| a.data.to_base64())
        .collect();
    assert_eq!(stored, ["AAAA", "BBBB"]);

    let refused = db.send_blocking(AddActorSessionMessageCommand {
        session_id: session.id,
        content: ActorSessionMessageContent::User(ActorUserContent::Text {
            text: "And this".to_string(),
        }),
        attachments: vec![image("not base64")],
    });
    assert!(refused.is_err());

    db.send_blocking(DeleteActorSessionMessageCommand { id: message.id })
        .unwrap();
    assert!(list(message.id).is_empty());
}

#[test]
fn a_session_keeps_its_own_context_and_refuses_an_empty_window() {
    let db = context();
    let first = session(&db);
    let second = db
        .send_blocking(CreateActorSessionCommand {
            actor_id: first.actor_id,
            prompt: "Bump the dependencies".to_string(),
            title: None,
            mode: None,
            model: None,
            effort: None,
            attachments: Vec::new(),
        })
        .unwrap()
        .0;
    assert_eq!(first.context, None);

    let context = |used_tokens, total_tokens| ContextUsage {
        used_tokens,
        total_tokens,
    };
    let set = |session_id, context| {
        db.send_blocking(SetActorSessionContextCommand {
            session_id,
            context,
        })
    };
    let read = |session_id| {
        db.send_blocking(GetActorSessionCommand { id: session_id })
            .unwrap()
            .context
    };
    set(first.id, context(41_800, 200_000)).unwrap();
    set(second.id, context(150_000, 200_000)).unwrap();
    set(first.id, context(52_000, 200_000)).unwrap();
    assert_eq!(read(first.id), Some(context(52_000, 200_000)));
    assert_eq!(read(second.id), Some(context(150_000, 200_000)));

    assert!(matches!(
        set(first.id, context(10, 0)),
        Err(CommandError::Validation(_))
    ));
    assert!(matches!(
        set(first.id, context(-1, 200_000)),
        Err(CommandError::Validation(_))
    ));
    assert_eq!(read(first.id), Some(context(52_000, 200_000)));
}

/// Moves session `session_id` into `status`, as its actor would, or as the user
/// resolving it does.
fn set_status(db: &DatabaseContext, session_id: i64, status: ActorSessionStatus) {
    let moved = match status {
        ActorSessionStatus::Working => ActorSessionMove::Working,
        ActorSessionStatus::NeedsAttention => ActorSessionMove::NeedsYou,
        ActorSessionStatus::Idle => ActorSessionMove::Finished,
        ActorSessionStatus::Failed => ActorSessionMove::Failed,
        ActorSessionStatus::Resolved => ActorSessionMove::Resolved,
    };
    db.send_blocking(SetActorSessionStatusCommand { session_id, moved })
        .unwrap();
}

#[test]
fn a_session_starts_working_and_a_stopped_daemon_leaves_it_idle_unless_failed() {
    let db = context();
    let working = session(&db);
    assert_eq!(status(&db, working.id), ActorSessionStatus::Working);
    let attention = db
        .send_blocking(CreateActorSessionCommand {
            actor_id: working.actor_id,
            prompt: "Ask me".to_string(),
            title: None,
            mode: None,
            model: None,
            effort: None,
            attachments: Vec::new(),
        })
        .unwrap()
        .0;
    set_status(&db, attention.id, ActorSessionStatus::NeedsAttention);
    let failed = db
        .send_blocking(CreateActorSessionCommand {
            actor_id: working.actor_id,
            prompt: "Break".to_string(),
            title: None,
            mode: None,
            model: None,
            effort: None,
            attachments: Vec::new(),
        })
        .unwrap()
        .0;
    set_status(&db, failed.id, ActorSessionStatus::Failed);

    db.send_blocking(ShutdownCleanupCommand).unwrap();
    assert_eq!(status(&db, working.id), ActorSessionStatus::Idle);
    assert_eq!(status(&db, attention.id), ActorSessionStatus::Idle);
    assert_eq!(status(&db, failed.id), ActorSessionStatus::Failed);

    // A failed session still hears more.
    let text = ActorSessionMessageContent::User(ActorUserContent::Text {
        text: "Try again".to_string(),
    });
    say(&db, failed.id, text).unwrap();
}

#[test]
fn a_session_moves_once_per_status_it_takes_and_a_stopped_daemon_raises_nothing() {
    let (db, events) = recording_context();
    let session = session(&db);
    events.take();
    set_status(&db, session.id, ActorSessionStatus::NeedsAttention);
    set_status(&db, session.id, ActorSessionStatus::NeedsAttention);
    set_status(&db, session.id, ActorSessionStatus::Working);
    db.send_blocking(ShutdownCleanupCommand).unwrap();
    let moves: Vec<(i64, ActorSessionStatus)> = events
        .take()
        .into_iter()
        .filter_map(|event| match event {
            DatabaseEventKind::ActorSessionMoved {
                actor_session: session,
                ..
            } => Some((session.id, session.status)),
            _ => None,
        })
        .collect();
    assert_eq!(
        moves,
        [
            (session.id, ActorSessionStatus::NeedsAttention),
            (session.id, ActorSessionStatus::Working),
        ]
    );
}

/// Backdates session `id`'s status change to `at`.
struct BackdateStatusChange {
    id: i64,
    at: i64,
}
impl DatabaseCommand for BackdateStatusChange {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ctx.db
            .container
            .connection()
            .execute(
                "UPDATE actor_sessions SET status_changed_at = ?1 WHERE id = ?2",
                rusqlite::params![self.at, self.id],
            )
            .unwrap();
        Ok(().invalidating_nothing())
    }
}

#[test]
fn sessions_are_ordered_by_their_last_status_change_not_by_what_is_said() {
    let db = context();
    let older = session(&db);
    let newer = db
        .send_blocking(CreateActorSessionCommand {
            actor_id: older.actor_id,
            prompt: "Bump the dependencies".to_string(),
            title: None,
            mode: None,
            model: None,
            effort: None,
            attachments: Vec::new(),
        })
        .unwrap()
        .0;
    for (id, at) in [(older.id, 100), (newer.id, 200)] {
        db.send_blocking(BackdateStatusChange { id, at }).unwrap();
    }
    let order = || -> Vec<i64> {
        db.send_blocking(ListActorSessionsCommand { actor_id: None })
            .unwrap()
            .into_iter()
            .map(|session| session.id)
            .collect()
    };

    let text = ActorSessionMessageContent::Actor(ActorMessageContent::Text {
        text: "Still at it".to_string(),
    });
    say(&db, older.id, text).unwrap();
    set_status(&db, older.id, ActorSessionStatus::Working);
    assert_eq!(order(), [newer.id, older.id]);

    set_status(&db, older.id, ActorSessionStatus::Idle);
    assert_eq!(order(), [older.id, newer.id]);
}

#[test]
fn a_request_takes_one_response_of_its_kind_and_shape() {
    let db = context();
    let session = session(&db);
    let request = say(&db, session.id, question()).unwrap();
    assert!(!request.answered);

    // Not an option, two picks of a single choice, or no answer at all.
    assert!(say(&db, session.id, answers(request.id, &["C"])).is_err());
    assert!(say(&db, session.id, answers(request.id, &["A", "B"])).is_err());
    assert!(say(&db, session.id, answers(request.id, &[])).is_err());
    // A response of another kind.
    let permission = ActorSessionMessageContent::User(ActorUserContent::Permission {
        request_id: request.id,
        decision: ActorPermissionDecision::Allow,
        message: None,
    });
    assert!(say(&db, session.id, permission).is_err());

    say(&db, session.id, answers(request.id, &["B"])).unwrap();
    let messages = db
        .send_blocking(ListActorSessionMessagesCommand {
            session_id: session.id,
            ids: None,
        })
        .unwrap();
    assert!(
        messages
            .iter()
            .any(|message| message.id == request.id && message.answered)
    );
    // Answered once only.
    assert!(say(&db, session.id, answers(request.id, &["A"])).is_err());
}

#[test]
fn what_the_user_says_after_a_request_passes_it_over() {
    let db = context();
    let session = session(&db);
    let text = |text: &str| {
        ActorSessionMessageContent::User(ActorUserContent::Text {
            text: text.to_string(),
        })
    };
    let said = ActorSessionMessageContent::Actor(ActorMessageContent::Text {
        text: "Which one?".to_string(),
    });
    let earlier = say(&db, session.id, said).unwrap();
    let request = say(&db, session.id, question()).unwrap();
    let answered = |id: i64| {
        db.send_blocking(ListActorSessionMessagesCommand {
            session_id: session.id,
            ids: None,
        })
        .unwrap()
        .into_iter()
        .any(|message| message.id == id && message.answered)
    };
    assert!(!answered(request.id));

    let reply = say(&db, session.id, text("Neither, use C")).unwrap();
    assert!(answered(request.id));
    // Only a request is answered, never what the user says.
    assert!(!answered(earlier.id));
    assert!(!answered(reply.id));
    // A request made after it is still open.
    let later = say(&db, session.id, question()).unwrap();
    assert!(!answered(later.id));
}

/// The category and end of the tool in message `message_id`, as read back.
fn stored_tool(
    db: &DatabaseContext,
    session_id: i64,
    message_id: i64,
) -> (ActorToolKind, Option<i64>) {
    let messages = db
        .send_blocking(ListActorSessionMessagesCommand {
            session_id,
            ids: None,
        })
        .unwrap();
    let message = messages
        .into_iter()
        .find(|message| message.id == message_id);
    match message.map(|message| message.content) {
        Some(ActorSessionMessageContent::Actor(ActorMessageContent::ToolUse {
            category,
            ended_at,
            ..
        })) => (category, ended_at),
        _ => panic!("not a tool"),
    }
}

/// The end of message `message_id`, a tool or thinking, as read back.
fn stored_end(db: &DatabaseContext, session_id: i64, message_id: i64) -> Option<i64> {
    let messages = db
        .send_blocking(ListActorSessionMessagesCommand {
            session_id,
            ids: None,
        })
        .unwrap();
    let message = messages
        .into_iter()
        .find(|message| message.id == message_id);
    match message.map(|message| message.content) {
        Some(ActorSessionMessageContent::Actor(mut said)) => {
            *said.ended_at_mut().expect("a message that runs")
        }
        _ => panic!("not the actor's"),
    }
}

#[test]
fn only_a_tool_of_the_session_ends_and_keeps_its_first_end() {
    let db = context();
    let session = session(&db);
    let tool = ActorSessionMessageContent::Actor(ActorMessageContent::ToolUse {
        tool: "Edit".to_string(),
        category: ActorToolKind::Edit,
        summary: "src/main.rs".to_string(),
        ended_at: None,
    });
    let tool = say(&db, session.id, tool).unwrap();
    assert_eq!(stored_end(&db, session.id, tool.id), None);
    let end = |session_id, message_id| {
        db.send_blocking(EndActorSessionMessageCommand {
            session_id,
            message_id,
        })
    };

    // Not a tool, or a tool of another session.
    let text = ActorSessionMessageContent::Actor(ActorMessageContent::Text {
        text: "Fixed.".to_string(),
    });
    let text = say(&db, session.id, text).unwrap();
    assert!(end(session.id, text.id).is_err());
    assert!(end(session.id + 1, tool.id).is_err());
    assert_eq!(stored_end(&db, session.id, tool.id), None);

    end(session.id, tool.id).unwrap();
    let ended_at = stored_end(&db, session.id, tool.id);
    assert!(ended_at.is_some());
    end(session.id, tool.id).unwrap();
    assert_eq!(stored_end(&db, session.id, tool.id), ended_at);
    // Ending it keeps what kind of tool it is.
    assert_eq!(stored_tool(&db, session.id, tool.id).0, ActorToolKind::Edit);
}

#[test]
fn thinking_ends_once_and_keeps_its_text() {
    let db = context();
    let session = session(&db);
    let thinking = ActorSessionMessageContent::Actor(ActorMessageContent::Thinking {
        text: "Maybe the linker.".to_string(),
        ended_at: None,
    });
    let thinking = say(&db, session.id, thinking).unwrap();
    assert_eq!(stored_end(&db, session.id, thinking.id), None);
    let end = || {
        db.send_blocking(EndActorSessionMessageCommand {
            session_id: session.id,
            message_id: thinking.id,
        })
        .unwrap()
    };
    end();
    let first = stored_end(&db, session.id, thinking.id);
    assert!(first.is_some());
    end();
    assert_eq!(stored_end(&db, session.id, thinking.id), first);
    let messages = db
        .send_blocking(ListActorSessionMessagesCommand {
            session_id: session.id,
            ids: None,
        })
        .unwrap();
    assert!(matches!(
        &messages.iter().find(|message| message.id == thinking.id).unwrap().content,
        ActorSessionMessageContent::Actor(ActorMessageContent::Thinking { text, .. })
            if text == "Maybe the linker."
    ));
}

#[test]
fn a_permission_takes_only_a_decision_it_offers() {
    let db = context();
    let session = session(&db);
    let request = ActorSessionMessageContent::Actor(ActorMessageContent::Permission(
        ActorPermissionRequest {
            subject: ActorPermissionSubject::Command {
                command: "rm -rf target".to_string(),
                cwd: None,
            },
            reason: None,
            decisions: vec![
                ActorPermissionDecision::Allow,
                ActorPermissionDecision::Deny,
            ],
        },
    ));
    let request = say(&db, session.id, request).unwrap();
    let decide = |decision| {
        ActorSessionMessageContent::User(ActorUserContent::Permission {
            request_id: request.id,
            decision,
            message: None,
        })
    };
    assert!(
        say(
            &db,
            session.id,
            decide(ActorPermissionDecision::AllowForSession)
        )
        .is_err()
    );
    say(&db, session.id, decide(ActorPermissionDecision::Deny)).unwrap();
}

#[test]
fn a_session_is_unread_once_its_actor_finishes_a_stretch_not_when_one_is_cut_short() {
    use crate::{FinishActivityCommand, StartActivityCommand};
    use cohesion_core::model::{ActivityKind, ActivityOutcome};

    let db = context();
    let session = session(&db);
    let unread = |db: &DatabaseContext| {
        db.send_blocking(GetActorSessionCommand { id: session.id })
            .unwrap()
            .unread
    };
    let stretch = |db: &DatabaseContext, outcome: ActivityOutcome| {
        let activity = db
            .send_blocking(StartActivityCommand {
                title: session.title.clone(),
                kind: ActivityKind::ActorSession {
                    actor_session_id: session.id,
                },
            })
            .unwrap();
        db.send_blocking(FinishActivityCommand {
            activity_id: activity.id,
            outcome,
        })
        .unwrap();
    };
    assert!(!unread(&db));

    stretch(&db, ActivityOutcome::Succeeded);
    assert!(unread(&db));
    let read = db
        .send_blocking(MarkActorSessionReadCommand {
            session_id: session.id,
        })
        .unwrap();
    assert!(read.read_at.is_some());
    assert!(!unread(&db));

    // Stopped, or cut off by its client or the daemon going away.
    stretch(&db, ActivityOutcome::Cancelled);
    db.send_blocking(StartActivityCommand {
        title: session.title.clone(),
        kind: ActivityKind::ActorSession {
            actor_session_id: session.id,
        },
    })
    .unwrap();
    db.send_blocking(ShutdownCleanupCommand).unwrap();
    assert!(!unread(&db));

    stretch(&db, ActivityOutcome::Failed);
    assert!(unread(&db));
}

#[test]
fn a_finished_turn_leaves_a_session_unread_and_resolving_it_reads_it() {
    let db = context();
    let session = session(&db);
    let get = |db: &DatabaseContext| {
        db.send_blocking(GetActorSessionCommand { id: session.id })
            .unwrap()
    };
    let moved = |db: &DatabaseContext, moved: ActorSessionMove| {
        db.send_blocking(SetActorSessionStatusCommand {
            session_id: session.id,
            moved,
        })
        .unwrap();
    };
    moved(&db, ActorSessionMove::Working);
    assert!(!get(&db).unread);
    moved(&db, ActorSessionMove::Finished);
    assert!(get(&db).unread);

    moved(&db, ActorSessionMove::Resolved);
    let resolved = get(&db);
    assert!(!resolved.unread);
    assert!(resolved.read_at.is_some());

    // Its actor ending a turn after it was resolved leaves it resolved and read.
    moved(&db, ActorSessionMove::Finished);
    assert!(!get(&db).unread);

    moved(&db, ActorSessionMove::Working);
    moved(&db, ActorSessionMove::Failed);
    assert!(get(&db).unread);
}

#[test]
fn search_finds_a_session_by_its_title_or_its_text_but_not_its_tools() {
    let db = context();
    let session = session(&db);
    let search = |query: &str| {
        db.send_blocking(SearchActorSessionsCommand {
            query: query.to_string(),
        })
        .unwrap()
        .into_iter()
        .map(|hit| (hit.id, hit.excerpt))
        .collect::<Vec<_>>()
    };
    let reply = say(
        &db,
        session.id,
        ActorSessionMessageContent::Actor(ActorMessageContent::Text {
            text: "The flaky test was a race in the scheduler.".to_string(),
        }),
    )
    .unwrap();
    say(
        &db,
        session.id,
        ActorSessionMessageContent::Actor(ActorMessageContent::ToolUse {
            tool: "Edit".to_string(),
            category: ActorToolKind::Edit,
            summary: "Cargo.toml".to_string(),
            ended_at: None,
        }),
    )
    .unwrap();

    // The title outweighs the prompt that repeats it, so no passage shows.
    assert_eq!(search("build"), [(session.id, None)]);
    let [(id, Some(excerpt))] = &search("schedulers")[..] else {
        panic!("no passage");
    };
    assert_eq!(*id, session.id);
    assert!(excerpt.contains("scheduler"));
    assert!(search("toml").is_empty());

    db.send_blocking(DeleteActorSessionMessageCommand { id: reply.id })
        .unwrap();
    assert!(search("scheduler").is_empty());
    db.send_blocking(DeleteActorSessionCommand { id: session.id })
        .unwrap();
    assert!(search("build").is_empty());
}

/// How many rows `sql`, a `SELECT COUNT(*)`, counts.
struct Count(&'static str);
impl DatabaseCommand for Count {
    type Response = i64;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<i64> {
        Ok(ctx
            .db
            .container()
            .query_row(self.0, [], |row| read_column(row, "value"))?
            .unwrap_or(0))
    }
}

#[test]
fn a_session_resolved_a_week_ago_goes_with_everything_that_pointed_at_it_and_no_other_does() {
    let db = context();
    let make = || session(&db).id;
    let (done, failed, unread, working, attention, recent, idle, broke) = (
        make(),
        make(),
        make(),
        make(),
        make(),
        make(),
        make(),
        make(),
    );
    let scout = db
        .send_blocking(crate::CreateIntelligenceAgentCommand {
            name: Some("Scout".to_string()),
            color: cohesion_core::model::IntelligenceAgentColor::Blue,
            emoji: None,
            personality: None,
            verbosity: None,
            role: None,
            directive: None,
            team_id: None,
        })
        .unwrap()
        .id;
    db.send_blocking(crate::FollowActorSessionCommand {
        session_id: done,
        agent_id: scout,
        initiative: cohesion_core::model::Initiative::User,
    })
    .unwrap();
    db.send_blocking(AddActorSessionMessageCommand {
        session_id: done,
        content: ActorSessionMessageContent::User(ActorUserContent::Text {
            text: "Like this".to_string(),
        }),
        attachments: vec![ImageUpload {
            media_type: "image/png".to_string(),
            data: "AAAA".to_string(),
        }],
    })
    .unwrap();
    for (id, status) in [
        (done, ActorSessionStatus::Resolved),
        (failed, ActorSessionStatus::Resolved),
        (unread, ActorSessionStatus::Idle),
        (attention, ActorSessionStatus::NeedsAttention),
        (idle, ActorSessionStatus::Idle),
        (broke, ActorSessionStatus::Failed),
    ] {
        set_status(&db, id, status);
    }
    db.send_blocking(RawSql(
        "UPDATE actor_sessions SET unread = (id = 3), status_changed_at = unixepoch() - 8 * 86400",
    ))
    .unwrap();
    set_status(&db, recent, ActorSessionStatus::Resolved);

    db.send_blocking(DeleteOldActorSessionsCommand { days: 7 })
        .unwrap();

    let exists = |id| db.send_blocking(GetActorSessionCommand { id }).is_ok();
    for gone in [done, failed] {
        assert!(!exists(gone), "{gone}");
    }
    // Done or failed long ago, read or not, a session stays until resolved.
    for kept in [unread, working, attention, recent, idle, broke] {
        assert!(exists(kept), "{kept}");
    }
    for sql in [
        "SELECT COUNT(*) AS value FROM actor_session_messages WHERE session_id IN (1, 2)",
        "SELECT COUNT(*) AS value FROM attachments",
        "SELECT COUNT(*) AS value FROM activities WHERE actor_session_id IN (1, 2)",
    ] {
        assert_eq!(db.send_blocking(Count(sql)).unwrap(), 0, "{sql}");
    }
}

#[test]
fn a_resolved_session_stays_resolved_until_it_works_again_and_lists_leave_it_out() {
    let db = context();
    let session = session(&db);
    let listed = |db: &DatabaseContext| -> Vec<i64> {
        db.send_blocking(ListActorSessionsCommand { actor_id: None })
            .unwrap()
            .into_iter()
            .map(|session| session.id)
            .collect()
    };
    set_status(&db, session.id, ActorSessionStatus::Resolved);
    assert_eq!(status(&db, session.id), ActorSessionStatus::Resolved);
    assert!(listed(&db).is_empty());

    // Its actor winding down, or a stop, leaves it resolved.
    for moved in [
        ActorSessionMove::Finished,
        ActorSessionMove::Failed,
        ActorSessionMove::Interrupted,
        ActorSessionMove::Stopped,
    ] {
        db.send_blocking(SetActorSessionStatusCommand {
            session_id: session.id,
            moved,
        })
        .unwrap();
        assert_eq!(
            status(&db, session.id),
            ActorSessionStatus::Resolved,
            "{moved:?}"
        );
    }

    // More to do starts it again, and lists show it.
    set_status(&db, session.id, ActorSessionStatus::Working);
    assert_eq!(listed(&db), [session.id]);
}

#[test]
fn a_message_changes_only_itself_not_the_session_or_the_lists() {
    let db = context();
    let session = session(&db);
    let mut batches = db.subscribe();
    let message = say(&db, session.id, question()).unwrap();
    let mut keys = Vec::new();
    while let Ok(batch) = batches.try_recv() {
        keys.extend(batch.keys.clone());
    }
    assert_eq!(
        keys,
        [Invalidation::ActorSessionMessage {
            session_id: session.id,
            message_id: message.id,
        }]
    );
}
