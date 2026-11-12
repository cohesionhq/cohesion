use cohesion_macros::{Fields, Model, TextEnum, TsWire};
use serde::{Deserialize, Serialize};

/// How a document folder shows its documents; stored as text.
///
/// - `"list"`: as a list of documents (new folders start here).
/// - `"table"`: as a table, a row per document and a column per property.
///
/// A folder gaining a property becomes a table; one losing its last stays
/// one, an empty table, until it is set back to a list. A list keeps its
/// properties and its documents their values.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum DocumentFolderRepresentation {
    List,
    Table,
}

/// A named group of documents in a workspace, which may sit inside another.
///
/// Folders nest without limit. A document in no folder is in the Inbox;
/// journals are never in a folder. A folder's properties
/// (`DocumentFolderProperty`) are a table's columns, and its documents are
/// its rows; `representation` says whether it shows as one.
#[derive(Clone, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "document_folders", backup = Included)]
pub struct DocumentFolder {
    pub id: i64,
    pub workspace_id: i64,
    /// Id of the folder this one sits in, or `null` at the top level.
    pub parent_id: Option<i64>,
    pub title: String,
    /// Where the folder sits among its siblings; lower comes first.
    pub sort_order: i64,
    pub deleted: bool,
    /// Whether the folder shows as a list or a table.
    pub representation: DocumentFolderRepresentation,
}
