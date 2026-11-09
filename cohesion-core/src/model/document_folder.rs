use cohesion_macros::{Fields, Model, TsWire};
use serde::{Deserialize, Serialize};

/// A named group of documents in a workspace, which may sit inside another.
///
/// Folders nest without limit. A document in no folder is in the Inbox;
/// journals are never in a folder.
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
}
