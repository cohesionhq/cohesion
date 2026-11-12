use cohesion_macros::{Fields, Model, TextEnum, TsWire};
use serde::{Deserialize, Serialize};

/// What kind of item a search result points at: `"document"`, `"task"`,
/// `"resource"`, `"contact"`, `"taskFolder"`, `"documentFolder"` or `"feed"`.
#[derive(Clone, Serialize, Deserialize, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
pub enum SearchEntityKind {
    Document,
    Task,
    Resource,
    Contact,
    TaskFolder,
    DocumentFolder,
    Feed,
}

/// One full-text search hit: a snapshot of the matched document's, task's,
/// resource's, contact's, folder's or feed's indexed text, plus a pointer back
/// to it. A folder or a feed is indexed by its title alone.
#[derive(Clone, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "search_fts", backup = Included)]
pub struct SearchResult {
    /// Id of the search index entry.
    ///
    /// This is **not** the matched item's id; that is the entity id.
    pub id: i64,
    /// Whether the hit is a document, a task, a resource, a contact, a task
    /// or document folder, or a feed.
    pub entity_kind: SearchEntityKind,
    /// Id of the matched document, task, resource, contact, folder or feed
    /// (which one depends on the entity kind).
    pub entity_id: i64,
    /// Id of the workspace the matched item belongs to; for a feed, which
    /// belongs to none, the workspace searched.
    pub workspace_id: i64,

    /// The matched item's title; a contact's full name.
    pub title: Option<String>,
    /// The matched document's full contents, a Markdown resource's text or a
    /// contact's notes; always `""` for tasks, folders, feeds and JSON
    /// resources.
    pub contents: String,
    /// The matched task's status (`"incomplete"`, `"completed"` or
    /// `"cancelled"`); `"none"` for every other kind, which
    /// have no status, so every hit has the same shape.
    pub status: String,
}
