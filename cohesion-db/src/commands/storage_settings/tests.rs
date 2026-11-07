use crate::GetDatabaseSizeCommand;
use crate::test_support::{RawSql, context};

#[test]
fn the_database_size_counts_its_free_pages() {
    let db = context();
    let before = db.send_blocking(GetDatabaseSizeCommand {}).unwrap();
    assert!(before.bytes > 0);
    db.send_blocking(RawSql(
        "INSERT INTO security_events (kind, client_name) VALUES ('clientPaired', zeroblob(1000000))",
    ))
    .unwrap();
    db.send_blocking(RawSql("DELETE FROM security_events"))
        .unwrap();
    let after = db.send_blocking(GetDatabaseSizeCommand {}).unwrap();
    // The file grew to hold the value, and its pages are free once it went.
    assert!(after.bytes > before.bytes + 900_000);
    assert!(after.free_bytes > before.free_bytes + 900_000);
}
