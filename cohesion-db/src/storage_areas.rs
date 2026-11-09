//! What each area of Cohesion takes up of the database file, for
//! Settings › Storage. SQLite's `dbstat` table counts the pages of every
//! table and index, so a size is exact to the page: each table belongs to one
//! [`StorageArea`], its indexes with it. The one table two areas share,
//! `attachments`, has its pages divided between agent chats and actor sessions
//! by how many bytes of images each holds there.

use cohesion_core::protocol::{StorageArea, StorageAreaSize};
use rusqlite::Connection;

use crate::database_error::DatabaseError;

/// Where a table's pages go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placement {
    In(StorageArea),
    /// `attachments`: the images of both agent messages and actor sessions.
    Attachments,
}

/// Which area table `table` belongs to; `None` for one this doesn't name,
/// which a test refuses for every table the migrations make.
fn placement(table: &str) -> Option<Placement> {
    use StorageArea::*;
    let area = match table {
        "attachments" => return Some(Placement::Attachments),
        "intelligence_agent_messages"
        | "intelligence_agent_runs"
        | "intelligence_threads"
        | "intelligence_thread_messages"
        | "intelligence_thread_images"
        | "thread_prompts"
        | "agent_thread_marks" => AgentChats,
        "actor_sessions" | "actor_session_messages" | "activities" => ActorSessions,
        t if t.starts_with("actor_sessions_fts") || t.starts_with("actor_session_messages_fts") => {
            ActorSessions
        }
        "documents"
        | "document_folders"
        | "tasks"
        | "task_folders"
        | "tags"
        | "task_tags"
        | "saved_task_views"
        | "saved_task_view_tags"
        | "saved_task_view_priorities"
        | "saved_task_view_sizes" => DocumentsAndTasks,
        "feeds" | "articles" => FeedsAndArticles,
        t if t.starts_with("feed_corpus_fts") => FeedsAndArticles,
        "resources" | "resource_kinds" | "workspace_resource_kinds" => Resources,
        "memories" => Memories,
        "versions" | "rewind_writer" => RewindVersions,
        "workspaces"
        | "metadata"
        | "search_entries"
        | "contacts"
        | "contact_methods"
        | "clients"
        | "security_settings"
        | "security_events"
        | "storage_settings"
        | "profile"
        | "catalog_models"
        | "intelligence_endpoints"
        | "endpoint_models"
        | "intelligence_defaults"
        | "intelligence_spend_limits"
        | "intelligence_cost_events"
        | "intelligence_agents"
        | "intelligence_agent_cues"
        | "intelligence_agent_teams"
        | "actors"
        | "extensions"
        | "extension_permissions"
        | "extension_hooks"
        | "extension_services"
        | "extension_service_functions"
        | "connectors"
        | "connector_accounts"
        | "storage_entries"
        | "sqlite_schema"
        | "sqlite_sequence" => Other,
        t if t.starts_with("search_fts") => Other,
        _ => return None,
    };
    Some(Placement::In(area))
}

/// The bytes each area's tables and indexes take up, largest first and
/// `Other` last. `Other` also holds what `dbstat` doesn't count as a table
/// (the lock-byte page), so with `free` the areas add up to `total`.
pub(crate) fn measure(
    connection: &Connection,
    total: i64,
    free: i64,
) -> Result<Vec<StorageAreaSize>, DatabaseError> {
    // One row a table or index, named for its table; `sqlite_schema` has no
    // row of its own in itself.
    let mut statement = connection.prepare(
        "SELECT coalesce(s.tbl_name, d.name), sum(d.pgsize)
         FROM dbstat AS d LEFT JOIN sqlite_schema AS s ON s.name = d.name
         WHERE d.aggregate = TRUE
         GROUP BY 1",
    )?;
    let tables = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    // `length` reads a blob's size from its row, not its pages.
    let (agent_images, actor_images): (i64, i64) = connection.query_row(
        "SELECT coalesce(sum(length(data)) FILTER (WHERE agent_message_id IS NOT NULL), 0),
                coalesce(sum(length(data)) FILTER (WHERE actor_session_message_id IS NOT NULL), 0)
         FROM attachments",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    let mut areas = AREAS.map(|area| StorageAreaSize { area, bytes: 0 });
    let mut add = |area: StorageArea, bytes: i64| {
        let index = AREAS
            .iter()
            .position(|a| *a == area)
            .expect("every area is listed");
        areas[index].bytes += bytes;
    };
    let mut counted = 0;
    for (table, bytes) in tables {
        counted += bytes;
        match placement(&table) {
            Some(Placement::In(area)) => add(area, bytes),
            Some(Placement::Attachments) => {
                let images = agent_images + actor_images;
                let to_agents = if images == 0 {
                    0
                } else {
                    (bytes as i128 * agent_images as i128 / images as i128) as i64
                };
                add(StorageArea::AgentChats, to_agents);
                add(StorageArea::ActorSessions, bytes - to_agents);
            }
            None => add(StorageArea::Other, bytes),
        }
    }
    add(StorageArea::Other, total - free - counted);

    let mut areas = areas.to_vec();
    areas.sort_by_key(|size| (size.area == StorageArea::Other, -size.bytes));
    Ok(areas)
}

const AREAS: [StorageArea; 8] = [
    StorageArea::AgentChats,
    StorageArea::ActorSessions,
    StorageArea::DocumentsAndTasks,
    StorageArea::FeedsAndArticles,
    StorageArea::Resources,
    StorageArea::Memories,
    StorageArea::RewindVersions,
    StorageArea::Other,
];

#[cfg(test)]
mod tests;
