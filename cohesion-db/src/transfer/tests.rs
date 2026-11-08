use std::time::{SystemTime, UNIX_EPOCH};

use cohesion_core::LocalIdentity;
use cohesion_core::model::IntelligenceAgentColor;

use super::*;
use crate::test_support::RawSql;
use crate::{CreateIntelligenceAgentCommand, IdentifyLocalClientCommand};

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

/// A daemon's database at `path`, open and migrated, holding a workspace
/// named `workspace` and a client named `client`.
fn database(path: &Path, workspace: &'static str, client: &str) -> DatabaseContext {
    let mut container = Container::open(path).unwrap();
    container.migrate().unwrap();
    let db = DatabaseContext::new(container, Secrets::new(), ());
    db.send_blocking(RawSql(workspace)).unwrap();
    db.send_blocking(IdentifyLocalClientCommand {
        identity: LocalIdentity {
            key: client.to_string(),
            name: client.to_string(),
        },
    })
    .unwrap();
    db
}

/// A source daemon's database at `path`: a workspace, a client, and an agent
/// chat and a model thread with an image each, the chat's larger than one
/// part of an export.
fn source(path: &Path) -> DatabaseContext {
    let db = database(
        path,
        "INSERT INTO workspaces (title) VALUES ('Moved')",
        "Source laptop",
    );
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
    for sql in [
        "INSERT INTO intelligence_agent_messages (agent_id, sender, kind, text) VALUES (1, 'user', 'text', 'hi')",
        "INSERT INTO attachments (agent_message_id, media_type, data) VALUES (1, 'image/png', zeroblob(9000000))",
        "INSERT INTO thread_prompts (prompt) VALUES ('')",
        "INSERT INTO intelligence_threads (prompt_id) VALUES (1)",
        "INSERT INTO intelligence_thread_messages (thread_id, kind, text) VALUES (1, 0, 'look')",
        "INSERT INTO intelligence_thread_images (message_id, media_type, data) VALUES (1, 'image/jpeg', x'ffd8ffe0')",
    ] {
        db.send_blocking(RawSql(sql)).unwrap();
    }
    db
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn count(connection: &Connection, sql: &str) -> i64 {
    connection.query_row(sql, [], |row| row.get(0)).unwrap()
}

/// Reads export `export` to the end, a part at a time.
async fn read_all(db: &DatabaseContext, export: &DatabaseExport) -> Vec<Vec<u8>> {
    let mut parts = Vec::new();
    let mut offset = 0;
    while offset < export.bytes {
        let part = db.read_export(export.id.clone(), offset).await.unwrap();
        offset += part.data.0.len() as i64;
        parts.push(part.data.0);
    }
    parts
}

#[test]
fn an_export_taken_while_the_daemon_runs_holds_every_row_and_goes_once_read() {
    let directory = directory("export");
    let db = source(&directory.join("cohesion.db"));
    let runtime = runtime();

    let export = runtime.block_on(db.export()).unwrap();
    let parts = runtime.block_on(read_all(&db, &export));

    assert!(parts.len() > 1);
    let file = directory.join("copy.db");
    std::fs::write(&file, parts.concat()).unwrap();
    let copy = Connection::open(&file).unwrap();
    assert_eq!(
        count(
            &copy,
            "SELECT COUNT(*) FROM workspaces WHERE title = 'Moved'"
        ),
        1
    );
    assert_eq!(
        count(&copy, "SELECT length(data) FROM attachments"),
        9_000_000
    );
    assert_eq!(
        count(
            &copy,
            "SELECT COUNT(*) FROM intelligence_thread_images WHERE data = x'ffd8ffe0'"
        ),
        1
    );
    assert!(matches!(
        runtime.block_on(db.read_export(export.id, 0)),
        Err(TransferError::Unknown)
    ));
    drop((copy, db));
    std::fs::remove_dir_all(directory).ok();
}

#[test]
fn an_import_replaces_every_row_and_keeps_this_servers_clients() {
    let directory = directory("import");
    let runtime = runtime();
    let source = source(&directory.join("source").join("cohesion.db"));
    let database_file = directory.join("destination").join("cohesion.db");
    let destination = database(
        &database_file,
        "INSERT INTO workspaces (title) VALUES ('Replaced')",
        "Destination phone",
    );

    let staged = runtime.block_on(async {
        let export = source.export().await.unwrap();
        let import = destination.begin_import().await.unwrap();
        let mut offset = 0;
        for part in read_all(&source, &export).await {
            let length = part.len() as i64;
            destination
                .write_import(import.id.clone(), offset, TransferBytes(part))
                .await
                .unwrap();
            offset += length;
        }
        let resent = destination
            .write_import(import.id.clone(), 0, TransferBytes(vec![0]))
            .await;
        assert!(matches!(resent, Err(TransferError::WrongOffset { .. })));
        let staged = destination.stage_import(import.id).await.unwrap();
        destination.close().await;
        staged
    });
    staged.replace().unwrap();

    let imported = Connection::open(&database_file).unwrap();
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
    assert_eq!(
        count(&imported, "SELECT length(data) FROM attachments"),
        9_000_000
    );
    assert_eq!(
        count(&imported, "SELECT COUNT(*) FROM intelligence_thread_images"),
        1
    );
    let live = |name: &str| {
        count(
            &imported,
            &format!("SELECT COUNT(*) FROM clients WHERE client_name = '{name}' AND NOT deleted"),
        )
    };
    assert_eq!(live("Destination phone"), 1);
    assert_eq!(live("Source laptop"), 0);
    assert_eq!(
        count(&imported, "SELECT COUNT(*) FROM pragma_foreign_key_check"),
        0
    );
    assert!(!directory.join("destination/cohesion.db.import").exists());
    assert_eq!(
        std::fs::read_dir(directory.join("destination/transfers"))
            .unwrap()
            .count(),
        0
    );
    drop((imported, source));
    std::fs::remove_dir_all(directory).ok();
}

#[test]
fn a_staged_import_waits_for_the_database_to_be_closed() {
    let directory = directory("in-use");
    let runtime = runtime();
    let source = source(&directory.join("source").join("cohesion.db"));
    let database_file = directory.join("destination").join("cohesion.db");
    let destination = database(
        &database_file,
        "INSERT INTO workspaces (title) VALUES ('Kept')",
        "Destination phone",
    );

    let staged = runtime.block_on(async {
        let export = source.export().await.unwrap();
        let import = destination.begin_import().await.unwrap();
        let data = read_all(&source, &export).await.concat();
        destination
            .write_import(import.id.clone(), 0, TransferBytes(data))
            .await
            .unwrap();
        destination.stage_import(import.id).await.unwrap()
    });

    assert!(matches!(staged.replace(), Err(TransferError::InUse)));
    runtime.block_on(destination.close());
    let kept = Connection::open(&database_file).unwrap();
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
fn an_import_of_what_no_daemon_exported_is_refused() {
    let directory = directory("not-cohesion");
    let runtime = runtime();
    let destination = database(
        &directory.join("cohesion.db"),
        "INSERT INTO workspaces (title) VALUES ('Kept')",
        "Destination phone",
    );
    let empty = directory.join("empty.db");
    Connection::open(&empty)
        .unwrap()
        .execute_batch("CREATE TABLE t (x)")
        .unwrap();
    let text = b"not a database, though long enough to have a header".to_vec();

    for data in [std::fs::read(&empty).unwrap(), text] {
        let refused = runtime.block_on(async {
            let import = destination.begin_import().await.unwrap();
            destination
                .write_import(import.id.clone(), 0, TransferBytes(data))
                .await
                .unwrap();
            destination.stage_import(import.id).await
        });
        assert!(matches!(refused, Err(TransferError::NotCohesion)));
    }
    assert!(!directory.join("cohesion.db.import").exists());
    drop(destination);
    std::fs::remove_dir_all(directory).ok();
}
