use cohesion_core::model::{HookKind, ImageBytes, IntelligenceAgentColor, ThreadItem};

use cohesion_core::value::read_column;

use crate::test_support::{RawSql, context};
use crate::{
    BlankOldThreadMessagesCommand, CommandContext, CreateAgentRunCommand,
    CreateIntelligenceAgentCommand, CreateIntelligenceMessageCommand,
    CreateIntelligenceThreadCommand, DatabaseCommand, DatabaseCommandResult, DatabaseContext,
    GetIntelligenceThreadCommand, GetIntelligenceThreadMessagesCommand, GetStorageSettingsCommand,
    ListAgentRunsCommand, PeriodicCleanupCommand, Read, ReadableDatabase,
    UpdateStorageSettingsCommand,
};

fn agent(db: &DatabaseContext) -> i64 {
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
    .id
}

#[test]
fn a_thread_is_deleted_two_weeks_after_its_last_message() {
    let db = context();
    let agent = agent(&db);
    let thread = || {
        db.send_blocking(CreateIntelligenceThreadCommand {
            system_prompt: "".to_string(),
        })
        .unwrap()
        .id
    };
    let say = |thread_id| {
        db.send_blocking(CreateIntelligenceMessageCommand {
            thread_id,
            item: ThreadItem::UserMessage {
                text: "Hello".to_string(),
            },
            images: Vec::new(),
        })
        .unwrap();
    };
    let (idle, quiet, busy) = (thread(), thread(), thread());
    say(quiet);
    say(busy);
    db.send_blocking(RawSql(
        "UPDATE intelligence_threads SET created_at = unixepoch() - 14 * 86400",
    ))
    .unwrap();
    db.send_blocking(RawSql(
        "UPDATE intelligence_thread_messages SET created_at = unixepoch() - 14 * 86400",
    ))
    .unwrap();
    say(busy);
    let fresh = thread();
    db.send_blocking(CreateAgentRunCommand {
        agent_id: agent,
        hook: HookKind::Timer,
        cue_name: "Check in".to_string(),
        thread_id: Some(quiet),
    })
    .unwrap();

    db.send_blocking(PeriodicCleanupCommand).unwrap();

    let exists = |thread_id| {
        db.send_blocking(GetIntelligenceThreadCommand { thread_id })
            .is_ok()
    };
    assert!(!exists(idle));
    assert!(!exists(quiet));
    assert!(exists(busy));
    assert!(exists(fresh));
    let runs = db
        .send_blocking(ListAgentRunsCommand {
            agent_id: agent,
            limit: 1,
        })
        .unwrap();
    assert_eq!(runs[0].thread_id, None);
}

fn say_item(db: &DatabaseContext, thread_id: i64, item: ThreadItem) {
    db.send_blocking(CreateIntelligenceMessageCommand {
        thread_id,
        item,
        images: Vec::new(),
    })
    .unwrap();
}

#[test]
fn an_old_message_loses_its_contents_once_and_keeps_its_row_and_pairing() {
    use ThreadItem::*;
    let db = context();
    let thread = db
        .send_blocking(CreateIntelligenceThreadCommand {
            system_prompt: "".to_string(),
        })
        .unwrap()
        .id;
    db.send_blocking(CreateIntelligenceMessageCommand {
        thread_id: thread,
        item: UserMessage {
            text: "Hi".to_string(),
        },
        images: vec![(
            "image/png".to_string(),
            ImageBytes::from_base64("AAAA").unwrap(),
        )],
    })
    .unwrap();
    for item in [
        AssistantMessage {
            text: "Hello".to_string(),
        },
        Thought {
            text: "hm".to_string(),
            signature: Some("sig".to_string()),
            in_content: false,
        },
        RedactedThought {
            trace: "opaque".to_string(),
            summary: Some("gist".to_string()),
        },
        ToolCallRequest {
            call_identifier: "c1".to_string(),
            tool_name: "code".to_string(),
            arguments: "{\"a\":1}".to_string(),
        },
        ToolCallResponse {
            call_identifier: "c1".to_string(),
            succeeded: false,
            text: "boom".to_string(),
        },
        ToolCallResponse {
            call_identifier: "c2".to_string(),
            succeeded: true,
            text: "fine".to_string(),
        },
    ] {
        say_item(&db, thread, item);
    }
    db.send_blocking(RawSql(
        "UPDATE intelligence_thread_messages SET created_at = unixepoch() - 15 * 86400",
    ))
    .unwrap();
    say_item(
        &db,
        thread,
        UserMessage {
            text: "Recent".to_string(),
        },
    );

    let blanked = db
        .send_blocking(BlankOldThreadMessagesCommand { days: 14 })
        .unwrap();
    assert_eq!(blanked, 7);
    assert_eq!(
        db.send_blocking(BlankOldThreadMessagesCommand { days: 14 })
            .unwrap(),
        0
    );

    let entries = db
        .send_blocking(GetIntelligenceThreadMessagesCommand { thread_id: thread })
        .unwrap();
    assert!(entries.iter().all(|entry| entry.images.is_empty()));
    let items: Vec<ThreadItem> = entries.into_iter().map(|e| e.message.item).collect();
    let blank_thought = Thought {
        text: String::new(),
        signature: None,
        in_content: false,
    };
    assert_eq!(
        items,
        [
            UserMessage {
                text: String::new()
            },
            AssistantMessage {
                text: String::new()
            },
            blank_thought.clone(),
            blank_thought,
            ToolCallRequest {
                call_identifier: "c1".to_string(),
                tool_name: "code".to_string(),
                arguments: "{}".to_string(),
            },
            ToolCallResponse {
                call_identifier: "c1".to_string(),
                succeeded: false,
                text: String::new(),
            },
            ToolCallResponse {
                call_identifier: "c2".to_string(),
                succeeded: true,
                text: String::new(),
            },
            UserMessage {
                text: "Recent".to_string()
            },
        ]
    );
}

#[test]
fn the_database_is_vacuumed_only_once_enough_space_is_free() {
    let db = context();
    db.send_blocking(CreateIntelligenceThreadCommand {
        system_prompt: "".to_string(),
    })
    .unwrap();
    let fill = |sql: &'static str| db.send_blocking(RawSql(sql)).unwrap();
    fill(
        "INSERT INTO intelligence_thread_messages (thread_id, kind, text) VALUES (1, 0, zeroblob(1000000))",
    );
    fill("DELETE FROM intelligence_thread_messages");
    assert_eq!(block_on(db.vacuum_if_reclaimable()).unwrap(), None);

    fill(
        "INSERT INTO intelligence_thread_messages (thread_id, kind, text) VALUES (1, 0, zeroblob(9000000))",
    );
    fill("DELETE FROM intelligence_thread_messages");
    assert!(block_on(db.vacuum_if_reclaimable()).unwrap().is_some());
    assert_eq!(block_on(db.vacuum_if_reclaimable()).unwrap(), None);

    // Free pages that are a small share of a large file wait to be reused.
    fill(
        "INSERT INTO intelligence_thread_messages (thread_id, kind, text) VALUES (1, 0, zeroblob(60000000))",
    );
    fill(
        "INSERT INTO intelligence_thread_messages (thread_id, kind, text) VALUES (1, 1, zeroblob(9000000))",
    );
    fill("DELETE FROM intelligence_thread_messages WHERE kind = 1");
    assert_eq!(block_on(db.vacuum_if_reclaimable()).unwrap(), None);
}

fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(future)
}

fn count(db: &DatabaseContext, sql: &'static str) -> i64 {
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
    db.send_blocking(Count(sql)).unwrap()
}

/// Sets the storage settings' model thread retention, keeping the rest.
fn keep_threads_for(db: &DatabaseContext, days: i64) {
    let settings = db.send_blocking(GetStorageSettingsCommand {}).unwrap();
    db.send_blocking(UpdateStorageSettingsCommand {
        thread_retention_days: days,
        actor_session_retention_days: settings.actor_session_retention_days,
        rewind_retention_days: settings.rewind_retention_days,
        deleted_item_retention_days: settings.deleted_item_retention_days,
        memory_limit: settings.memory_limit,
    })
    .unwrap();
}

#[test]
fn what_a_fork_added_empties_when_its_conversation_does() {
    let db = context();
    let agent = agent(&db);
    let thread = || {
        db.send_blocking(CreateIntelligenceThreadCommand {
            system_prompt: "".to_string(),
        })
        .unwrap()
        .id
    };
    let (trunk, fork) = (thread(), thread());
    for thread_id in [trunk, fork] {
        say_item(
            &db,
            thread_id,
            ThreadItem::UserMessage {
                text: "words".to_string(),
            },
        );
    }
    db.send_blocking(RawSql(Box::leak(
        format!(
            "INSERT INTO intelligence_agent_runs (agent_id, thread_id, hook)
             VALUES ({agent}, {fork}, 'timer')"
        )
        .into_boxed_str(),
    )))
    .unwrap();
    db.send_blocking(RawSql(
        "UPDATE intelligence_thread_messages SET created_at = unixepoch() - 4 * 86400",
    ))
    .unwrap();
    let text = |thread_id: i64| match db
        .send_blocking(GetIntelligenceThreadMessagesCommand { thread_id })
        .unwrap()
        .remove(0)
        .message
        .item
    {
        ThreadItem::UserMessage { text } => text,
        other => panic!("{other:?}"),
    };

    db.send_blocking(PeriodicCleanupCommand).unwrap();
    assert_eq!((text(trunk), text(fork)), ("words".into(), "words".into()));

    keep_threads_for(&db, 3);
    db.send_blocking(PeriodicCleanupCommand).unwrap();
    assert_eq!((text(trunk), text(fork)), (String::new(), String::new()));
}

#[test]
fn actor_images_go_with_their_task_and_script_results_with_the_thread_retention() {
    let db = context();
    let agent = agent(&db);
    for sql in [
        "INSERT INTO clients (id, key, client_name) VALUES (1, 'k', 'c')",
        "INSERT INTO actors (id, client_id, identifier, product, scope, modes, models) VALUES (1, 1, 'a', 'p', 's', '[]', '[]')",
        "INSERT INTO actor_sessions (id, actor_id, title) VALUES (1, 1, 'open')",
        "INSERT INTO actor_sessions (id, actor_id, title, status, status_changed_at) VALUES (2, 1, 'resolved', 'resolved', unixepoch() - 8 * 86400)",
        "INSERT INTO actor_session_messages (id, session_id, sender, kind, text, sent_at) VALUES (1, 1, 'user', 'text', 'old', unixepoch() - 400 * 86400), (2, 2, 'user', 'text', 'gone', unixepoch())",
        "INSERT INTO attachments (actor_session_message_id, media_type, data) VALUES (1, 'image/png', x'00'), (2, 'image/png', x'00')",
    ] {
        db.send_blocking(RawSql(sql)).unwrap();
    }
    db.send_blocking(RawSql(Box::leak(
        format!(
            "INSERT INTO intelligence_agent_messages (id, agent_id, sender, kind, text, sent_at)
             VALUES (1, {agent}, 'user', 'text', 'hi', unixepoch() - 400 * 86400)"
        )
        .into_boxed_str(),
    )))
    .unwrap();
    db.send_blocking(RawSql(
        "INSERT INTO attachments (agent_message_id, media_type, data) VALUES (1, 'image/png', x'00')",
    ))
    .unwrap();
    db.send_blocking(RawSql(Box::leak(
        format!(
            "INSERT INTO intelligence_agent_messages (id, agent_id, sender, kind, title, code,
                outcome, outcome_text, sent_at) VALUES
             (2, {agent}, 'agent', 'codeExecution', 'T', 'C', 'succeeded', 'long ago', unixepoch() - 15 * 86400),
             (3, {agent}, 'agent', 'codeExecution', 'T', 'C', 'succeeded', 'lately', unixepoch() - 3 * 86400)"
        )
        .into_boxed_str(),
    )))
    .unwrap();
    let blank_results = || {
        count(
            &db,
            "SELECT COUNT(*) AS value FROM intelligence_agent_messages
             WHERE kind = 'codeExecution' AND outcome_text = ''",
        )
    };

    db.send_blocking(PeriodicCleanupCommand).unwrap();

    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM attachments WHERE actor_session_message_id = 1"
        ),
        1,
        "an open task keeps its images however old"
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM attachments WHERE actor_session_message_id IS NOT NULL"
        ),
        1,
        "a deleted task's images go with it"
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM attachments WHERE agent_message_id IS NOT NULL"
        ),
        1
    );
    assert_eq!(blank_results(), 1);

    keep_threads_for(&db, 2);
    db.send_blocking(PeriodicCleanupCommand).unwrap();
    assert_eq!(blank_results(), 2);
}

#[test]
fn a_deleted_item_goes_for_good_only_once_its_retention_and_rewinds_have_passed() {
    let db = context();
    for sql in [
        "INSERT INTO workspaces (id, title) VALUES (1, 'w')",
        "INSERT INTO tasks (id, workspace_id, title) VALUES (1, 1, 'old'), (2, 1, 'recent'), (3, 1, 'live')",
        "INSERT INTO documents (id, workspace_id, title, contents) VALUES (1, 1, 'old', '')",
        "INSERT INTO metadata (task_id, key, value) VALUES (1, 'k', 'v')",
        "UPDATE tasks SET deleted = 1 WHERE id IN (1, 2)",
        "UPDATE documents SET deleted = 1 WHERE id = 1",
    ] {
        db.send_blocking(RawSql(sql)).unwrap();
    }
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM tasks WHERE deleted_at IS NOT NULL"
        ),
        2,
        "the schema says when each went"
    );
    db.send_blocking(RawSql(
        "UPDATE tasks SET deleted_at = unixepoch() - 31 * 86400 WHERE id = 1",
    ))
    .unwrap();
    db.send_blocking(RawSql(
        "UPDATE documents SET deleted_at = unixepoch() - 31 * 86400 WHERE id = 1",
    ))
    .unwrap();

    db.send_blocking(PeriodicCleanupCommand).unwrap();

    assert_eq!(count(&db, "SELECT COUNT(*) AS value FROM tasks"), 2);
    assert_eq!(count(&db, "SELECT COUNT(*) AS value FROM documents"), 0);
    assert_eq!(
        count(&db, "SELECT COUNT(*) AS value FROM metadata"),
        0,
        "what hung off it goes too"
    );

    // Coming back clears the clock.
    db.send_blocking(RawSql("UPDATE tasks SET deleted = 0 WHERE id = 2"))
        .unwrap();
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) AS value FROM tasks WHERE deleted_at IS NOT NULL"
        ),
        0
    );
}
