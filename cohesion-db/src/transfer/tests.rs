use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use cohesion_core::LocalIdentity;
use cohesion_core::model::IntelligenceAgentColor;
use rusqlite::Connection;

use super::*;
use crate::test_support::RawSql;
use crate::{CreateIntelligenceAgentCommand, IdentifyLocalClientCommand, ListClientsCommand};

/// A new, empty directory for one test.
fn directory(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "cohesion-transfer-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

/// A daemon's database at `path`, open and migrated, holding `rows`.
fn database(path: &Path, rows: &[&'static str]) -> DatabaseContext {
    let mut container = Container::open(path).unwrap();
    container.migrate().unwrap();
    let db = DatabaseContext::new(container, Secrets::new(), ());
    for sql in rows {
        db.send_blocking(RawSql(sql)).unwrap();
    }
    db
}

/// A source daemon's database at `path`: an agent, then [`SOURCE_ROWS`].
fn source(path: &Path) -> DatabaseContext {
    let db = database(path, &[]);
    db.send_blocking(CreateIntelligenceAgentCommand {
        name: Some("Ada".to_string()),
        color: IntelligenceAgentColor::Blue,
        emoji: None,
        personality: None,
        verbosity: None,
        role: None,
        directive: None,
        team_id: None,
    })
    .unwrap();
    for sql in SOURCE_ROWS {
        db.send_blocking(RawSql(sql)).unwrap();
    }
    db
}

/// A workspace, an agent chat with an image and a model thread with an
/// image, for agent 1: binary rows among the rest.
const SOURCE_ROWS: &[&str] = &[
    "INSERT INTO workspaces (title) VALUES ('Moved')",
    "INSERT INTO intelligence_agent_messages (agent_id, sender, kind, text) VALUES (1, 'user', 'text', 'hi')",
    "INSERT INTO attachments (agent_message_id, media_type, data) VALUES (1, 'image/png', x'89504e47')",
    "INSERT INTO thread_prompts (prompt) VALUES ('')",
    "INSERT INTO intelligence_threads (prompt_id) VALUES (1)",
    "INSERT INTO intelligence_thread_messages (thread_id, kind, text) VALUES (1, 0, 'look')",
    "INSERT INTO intelligence_thread_images (message_id, media_type, data) VALUES (1, 'image/jpeg', x'ffd8ffe0')",
];

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

fn count(connection: &Connection, sql: &str) -> i64 {
    connection.query_row(sql, [], |row| row.get(0)).unwrap()
}

fn identify(db: &DatabaseContext, key: &str) {
    db.send_blocking(IdentifyLocalClientCommand {
        identity: LocalIdentity {
            key: key.to_string(),
            name: key.to_string(),
        },
    })
    .unwrap();
}

#[test]
fn an_export_taken_while_the_daemon_runs_holds_every_row_and_stands_alone() {
    let directory = directory("export");
    let source = source(&directory.join("cohesion.db"));
    let file = directory.join("export.db");

    let bytes = export_database(&directory.join("cohesion.db"), &file).unwrap();

    assert_eq!(bytes, std::fs::metadata(&file).unwrap().len());
    let copy = Connection::open(&file).unwrap();
    assert_eq!(
        count(
            &copy,
            "SELECT COUNT(*) FROM workspaces WHERE title = 'Moved'"
        ),
        1
    );
    assert_eq!(
        count(
            &copy,
            "SELECT COUNT(*) FROM attachments WHERE data = x'89504e47'"
        ),
        1
    );
    assert_eq!(
        count(
            &copy,
            "SELECT COUNT(*) FROM intelligence_thread_images WHERE data = x'ffd8ffe0'"
        ),
        1
    );
    let journal: String = copy
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap();
    assert_eq!(journal, "delete");
    assert!(matches!(
        export_database(&directory.join("cohesion.db"), &file),
        Err(TransferError::Exists(_))
    ));
    drop((copy, source));
    std::fs::remove_dir_all(directory).ok();
}

#[test]
fn an_import_replaces_every_row_and_revokes_the_copied_clients() {
    let directory = directory("import");
    let source = source(&directory.join("source.db"));
    identify(&source, "source laptop");
    let file = directory.join("export.db");
    export_database(&directory.join("source.db"), &file).unwrap();
    drop(source);
    let destination_file = directory.join("cohesion.db");
    let destination = database(
        &destination_file,
        &["INSERT INTO workspaces (title) VALUES ('Replaced')"],
    );
    identify(&destination, "destination laptop");
    block_on(destination.close());

    block_on(import_database(&file, &destination_file)).unwrap();

    let imported = Connection::open(&destination_file).unwrap();
    assert_eq!(
        count(
            &imported,
            "SELECT COUNT(*) FROM workspaces WHERE title = 'Replaced'"
        ),
        0
    );
    assert_eq!(
        count(
            &imported,
            "SELECT COUNT(*) FROM workspaces WHERE title = 'Moved'"
        ),
        1
    );
    assert_eq!(count(&imported, "SELECT COUNT(*) FROM attachments"), 1);
    assert_eq!(
        count(&imported, "SELECT COUNT(*) FROM intelligence_thread_images"),
        1
    );
    assert_eq!(
        count(
            &imported,
            "SELECT COUNT(*) FROM clients WHERE client_name = 'destination laptop'"
        ),
        0
    );
    drop(imported);
    assert!(!directory.join("cohesion.db.import").exists());
    let reopened = database(&destination_file, &[]);
    assert!(
        reopened
            .send_blocking(ListClientsCommand {})
            .unwrap()
            .is_empty()
    );
    drop(reopened);
    std::fs::remove_dir_all(directory).ok();
}

#[test]
fn an_import_refuses_a_database_a_daemon_has_open_and_leaves_it_as_it_was() {
    let directory = directory("in-use");
    let source = source(&directory.join("source.db"));
    let file = directory.join("export.db");
    export_database(&directory.join("source.db"), &file).unwrap();
    let destination_file = directory.join("cohesion.db");
    let destination = database(
        &destination_file,
        &["INSERT INTO workspaces (title) VALUES ('Kept')"],
    );

    let refused = block_on(import_database(&file, &destination_file));

    assert!(matches!(refused, Err(TransferError::InUse(_))));
    block_on(destination.close());
    let kept = Connection::open(&destination_file).unwrap();
    assert_eq!(
        count(
            &kept,
            "SELECT COUNT(*) FROM workspaces WHERE title = 'Kept'"
        ),
        1
    );
    drop((kept, source));
    std::fs::remove_dir_all(directory).ok();
}

#[test]
fn an_import_refuses_what_no_daemon_exported() {
    let directory = directory("not-cohesion");
    let empty = directory.join("empty.db");
    Connection::open(&empty)
        .unwrap()
        .execute_batch("CREATE TABLE t (x)")
        .unwrap();
    let text = directory.join("notes.txt");
    std::fs::write(&text, "not a database, though long enough to have a header").unwrap();
    let destination_file = directory.join("cohesion.db");

    for file in [&empty, &text] {
        let refused = block_on(import_database(file, &destination_file));
        assert!(
            matches!(refused, Err(TransferError::NotCohesion(_))),
            "{file:?}"
        );
    }
    assert!(!destination_file.exists());
    std::fs::remove_dir_all(directory).ok();
}
