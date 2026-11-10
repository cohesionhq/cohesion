use cohesion_macros::{Fields, Model, TsWire};
use serde::{Deserialize, Serialize};

/// A named group of documents in a workspace, which may sit inside another.
///
/// Folders nest without limit. A document in no folder is in the Inbox;
/// journals are never in a folder. A folder with properties
/// (`DocumentFolderProperty`) is a table, whose documents are its rows.
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
    /// Whether the folder has properties, so is a table; it stops being one
    /// when its last property is deleted.
    #[model(expr = DocumentFolder::IS_TABLE_SQL)]
    pub is_table: bool,
}

impl DocumentFolder {
    /// The SQL of [`is_table`](Self::is_table).
    pub const IS_TABLE_SQL: &'static str = "EXISTS (SELECT 1 FROM document_folder_properties \
        WHERE document_folder_properties.folder_id = document_folders.id)";
}
