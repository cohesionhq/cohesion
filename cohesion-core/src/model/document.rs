use cohesion_macros::{Fields, Model, TextEnum, TsWire};
use serde::{Deserialize, Serialize};

/// What a document is for; stored as text.
///
/// - `"normal"`: an ordinary document.
/// - `"journal"`: a daily journal entry; one per workspace per local day.
/// - `"databaseRow"`: a database row's own document. It never shows in the
///   document list or search, is never filed, and goes with its row.
#[derive(Serialize, Deserialize, Clone, Debug, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum DocumentType {
    Normal,
    Journal,
    DatabaseRow,
}

/// A piece of text in a workspace; journals are documents with a `journal_date`.
#[derive(Clone, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "documents", backup = Included)]
pub struct Document {
    /// Unique, stable identifier.
    pub id: i64,
    /// Id of the workspace this document belongs to.
    pub workspace_id: i64,
    /// Whether the document has been soft-deleted.
    ///
    /// Listings skip deleted documents, so this is normally `false`.
    pub deleted: bool,

    /// Whether this is an ordinary document or a daily journal.
    pub document_type: DocumentType,
    /// Display name.
    ///
    /// Journals are titled with their date, as `YYYY-MM-DD`.
    pub title: String,
    /// The full text of the document, as plain text / Markdown.
    pub contents: String,
    /// When the contents last changed, in Unix **seconds** (not
    /// milliseconds; in JavaScript use `new Date(modified * 1000)`).
    pub modified: i64,

    /// For journals, the day the entry belongs to, as `"YYYY-MM-DD"`.
    ///
    /// `null` for normal documents.
    pub journal_date: Option<String>,

    /// Id of the document folder this document is filed in, or `null` when
    /// it is in the Inbox.
    ///
    /// Always `null` for journals.
    pub folder_id: Option<i64>,
}
