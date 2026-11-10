use cohesion_macros::{Fields, Model, TextEnum, TsWire};
use serde::{Deserialize, Serialize};

/// What kind of item a search result points at: `"document"`, `"task"`,
/// `"resource"` or `"contact"`.
#[derive(Clone, Serialize, Deserialize, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
pub enum SearchEntityKind {
    Document,
    Task,
    Resource,
    Contact,
}

/// One full-text search hit: a snapshot of the matched document's, task's,
/// resource's or contact's indexed text, plus a pointer back to it.
#[derive(Clone, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "search_fts", backup = Included)]
pub struct SearchResult {
    /// Id of the search index entry.
    ///
    /// This is **not** the matched item's id; that is the entity id.
    pub id: i64,
    /// Whether the hit is a document, a task, a resource or a contact.
    pub entity_kind: SearchEntityKind,
    /// Id of the matched document, task, resource or contact (which one
    /// depends on the entity kind).
    pub entity_id: i64,
    /// Id of the workspace the matched item belongs to.
    pub workspace_id: i64,

    /// The matched item's title; a contact's full name.
    pub title: Option<String>,
    /// The matched document's full contents, a Markdown resource's text or a
    /// contact's notes; always `""` for tasks and JSON resources.
    pub contents: String,
}
