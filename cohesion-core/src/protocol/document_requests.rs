//! Requests about documents and journals.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

use crate::model::{DocumentFolderRepresentation, DocumentType};

/// Fetches the journal for `date` (`"YYYY-MM-DD"`) in a workspace, or `None`
/// if nothing has been written that day. Never creates one.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct GetJournalRequest {
    pub workspace_id: i64,
    pub date: String,
}

/// Appends `content` to the journal for `date` (`"YYYY-MM-DD"`) in a
/// workspace, creating it if needed.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct AppendToJournalRequest {
    pub workspace_id: i64,
    pub date: String,
    pub content: String,
}

/// Replaces the contents of the journal for `date` (`"YYYY-MM-DD"`) in a
/// workspace, creating it if needed.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct EditJournalRequest {
    pub workspace_id: i64,
    pub date: String,
    pub content: String,
}

/// Lists the live documents in a workspace whose title contains `query`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SearchDocumentRequest {
    pub workspace_id: i64,
    pub query: String,
}

/// Lists the live documents in a workspace, most recently modified first.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct ListDocumentsRequest {
    pub workspace_id: i64,
    /// Only documents of this type; every type when absent.
    pub document_type: Option<DocumentType>,
    /// Only journals dated on or after this `"YYYY-MM-DD"`.
    pub journal_from: Option<String>,
    /// Only journals dated before this `"YYYY-MM-DD"`.
    pub journal_to: Option<String>,
}

/// Fetches live document `document_id` in a workspace.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct GetDocumentRequest {
    pub workspace_id: i64,
    pub document_id: i64,
}

/// Replaces the contents of live document `document_id` in a workspace.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct EditDocumentRequest {
    pub workspace_id: i64,
    pub document_id: i64,
    pub contents: String,
}

/// Creates an ordinary document in a workspace.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct CreateDocumentRequest {
    pub workspace_id: i64,
    pub title: String,
    pub contents: String,
    /// The live document folder to file it in; the Inbox when absent.
    pub folder_id: Option<i64>,
}

/// Files ordinary document `document_id` in a workspace into folder
/// `folder_id`, or the Inbox when absent.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct MoveDocumentRequest {
    pub workspace_id: i64,
    pub document_id: i64,
    pub folder_id: Option<i64>,
}

/// Lists a workspace's live document folders, each with its `parent_id`,
/// in their sort order among their siblings.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListDocumentFoldersRequest {
    pub workspace_id: i64,
}

/// Creates a document folder in a workspace, last among its siblings.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct CreateDocumentFolderRequest {
    pub workspace_id: i64,
    pub title: String,
    /// The live folder to create it in; the top level when absent.
    pub parent_id: Option<i64>,
}

/// Retitles document folder `folder_id` in a workspace.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RenameDocumentFolderRequest {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub title: String,
}

/// Moves document folder `folder_id` into folder `parent_id`, or the top
/// level when absent, just before its sibling `before_id`, or last when
/// absent. A folder can't move into itself or a folder inside it.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct MoveDocumentFolderRequest {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub parent_id: Option<i64>,
    pub before_id: Option<i64>,
}

/// Sets how document folder `folder_id` in a workspace shows its
/// documents: as a list or a table. Its properties and values stay either
/// way.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetDocumentFolderRepresentationRequest {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub representation: DocumentFolderRepresentation,
}

/// Pins document folder `folder_id` in a workspace to the sidebar, or unpins
/// it. The folders inside it are left as they are.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetDocumentFolderPinnedRequest {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub pinned: bool,
}

/// Deletes document folder `folder_id` in a workspace and every folder
/// inside it; the documents they held move to the Inbox.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DeleteDocumentFolderRequest {
    pub workspace_id: i64,
    pub folder_id: i64,
}

/// Retitles live document `document_id` in a workspace.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RenameDocumentRequest {
    pub workspace_id: i64,
    pub document_id: i64,
    pub title: String,
}

/// Deletes document `document_id` in a workspace.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DeleteDocumentRequest {
    pub workspace_id: i64,
    pub document_id: i64,
}
