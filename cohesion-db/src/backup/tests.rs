use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use super::*;
use crate::test_support::RawSql;
use crate::{Container, CreateIntelligenceAgentCommand, DatabaseContext, Secrets};

#[test]
fn a_backup_holds_everything_but_the_left_out_tables_and_stands_alone() {
    let directory = std::env::temp_dir().join(format!(
        "cohesion-backup-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut container = Container::open(directory.join("cohesion.db")).unwrap();
    container.migrate().unwrap();
    let db = DatabaseContext::new(container, Secrets::new(), ());
    let agent = db
        .send_blocking(CreateIntelligenceAgentCommand {
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
    assert_eq!(agent.id, 1);
    for sql in [
        "INSERT INTO workspaces (title) VALUES ('Kept')",
        "INSERT INTO intelligence_agent_messages (agent_id, sender, kind, text) VALUES (1, 'user', 'text', 'hi')",
        "INSERT INTO attachments (agent_message_id, media_type, data) VALUES (1, 'image/png', x'00')",
    ] {
        db.send_blocking(RawSql(sql)).unwrap();
    }

    let backup = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(db.back_up())
        .unwrap();

    let copy = Connection::open(&backup.path).unwrap();
    let count = |sql: &str| -> i64 { copy.query_row(sql, [], |row| row.get(0)).unwrap() };
    assert_eq!(
        count("SELECT COUNT(*) FROM workspaces WHERE title = 'Kept'"),
        1
    );
    assert_eq!(count("SELECT COUNT(*) FROM intelligence_agent_messages"), 1);
    assert_eq!(count("SELECT COUNT(*) FROM attachments"), 0);
    let journal: String = copy
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap();
    assert_eq!(journal, "delete");
    assert_eq!(
        backup.bytes,
        std::fs::metadata(&backup.path).unwrap().len() as i64
    );
    drop(copy);
    drop(db);
    std::fs::remove_dir_all(directory).ok();
}
