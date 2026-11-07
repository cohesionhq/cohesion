use cohesion_macros::{Fields, Model, TextEnum, TsWire};
use serde::{Deserialize, Serialize};

/// What kind of item a search result points at: `"document"`, `"task"` or
/// `"resource"`.
#[derive(Clone, Serialize, Deserialize, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
pub enum SearchEntityKind {
    Document,
    Task,
    Resource,
}

/// One full-text search hit: a snapshot of the matched document's, task's or
/// resource's indexed text, plus a pointer back to it.
#[derive(Clone, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "search_fts", backup = Included)]
pub struct SearchResult {
    /// Id of the search index entry.
    ///
    /// This is **not** the matched item's id; that is the entity id.
    pub id: i64,
    /// Whether the hit is a document, a task or a resource.
    pub entity_kind: SearchEntityKind,
    /// Id of the matched document, task or resource (which one depends on the
    /// entity kind).
    pub entity_id: i64,
    /// Id of the workspace the matched item belongs to.
    pub workspace_id: i64,

    /// The matched item's title.
    pub title: Option<String>,
    /// The matched document's full contents or a Markdown resource's text;
    /// always `""` for tasks and JSON resources.
    pub contents: String,
}
