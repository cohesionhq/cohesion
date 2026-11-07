//! Requests about documents and journals.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

use crate::model::DocumentType;

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
pub struct CreateDocumentRequest {
    pub workspace_id: i64,
    pub title: String,
    pub contents: String,
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
