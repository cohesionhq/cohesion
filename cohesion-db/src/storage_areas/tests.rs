use cohesion_core::protocol::{DatabaseSize, StorageArea};

use super::placement;
use crate::GetDatabaseSizeCommand;
use crate::test_support::{RawSql, actor_session, context};
use crate::{CommandContext, DatabaseCommand, DatabaseCommandResult, Read, ReadableDatabase};

/// The name of every table the migrations make.
struct Tables;
impl DatabaseCommand for Tables {
    type Response = Vec<String>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Vec<String>> {
        let connection = ctx.db.container().connection();
        let mut statement = connection
            .prepare("SELECT name FROM sqlite_schema WHERE type = 'table'")
            .unwrap();
        Ok(statement
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap())
    }
}

fn area(size: &DatabaseSize, area: StorageArea) -> i64 {
    size.areas.iter().find(|a| a.area == area).unwrap().bytes
}

#[test]
fn every_table_belongs_to_an_area() {
    let db = context();
    let unplaced: Vec<String> = db
        .send_blocking(Tables)
        .unwrap()
        .into_iter()
        .filter(|table| placement(table).is_none())
        .collect();
    assert!(unplaced.is_empty(), "place these in an area: {unplaced:?}");
}

#[test]
fn the_areas_and_free_pages_add_up_to_the_file() {
    let db = context();
    db.send_blocking(RawSql(
        "INSERT INTO memories (memory) VALUES (zeroblob(500000))",
    ))
    .unwrap();
    let size = db.send_blocking(GetDatabaseSizeCommand {}).unwrap();
    let areas: i64 = size.areas.iter().map(|a| a.bytes).sum();
    assert_eq!(areas + size.free_bytes, size.bytes);
    assert!(area(&size, StorageArea::Memories) > 500_000);
    assert_eq!(size.areas.first().unwrap().area, StorageArea::Memories);
    assert_eq!(size.areas.last().unwrap().area, StorageArea::Other);
}

#[test]
fn an_actor_sessions_images_count_toward_actor_sessions() {
    let db = context();
    actor_session(&db);
    db.send_blocking(RawSql(
        "INSERT INTO actor_session_messages (session_id, sender, kind, text)
         VALUES ((SELECT id FROM actor_sessions), 'actor', 'text', 'Screenshot')",
    ))
    .unwrap();
    db.send_blocking(RawSql(
        "INSERT INTO attachments (actor_session_message_id, media_type, data)
         VALUES ((SELECT id FROM actor_session_messages), 'image/png', zeroblob(1000000))",
    ))
    .unwrap();
    let size = db.send_blocking(GetDatabaseSizeCommand {}).unwrap();
    assert!(area(&size, StorageArea::ActorSessions) > 1_000_000);
    assert!(area(&size, StorageArea::AgentChats) < 100_000);
}
