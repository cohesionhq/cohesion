use cohesion_macros::{Fields, Model, Projection, TextEnum, TsWire};
use serde::{Deserialize, Serialize};

/// What a document is for; stored as text.
///
/// - `"normal"`: an ordinary document.
/// - `"journal"`: a daily journal entry; one per workspace per local day.
#[derive(Serialize, Deserialize, Clone, Debug, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum DocumentType {
    Normal,
    Journal,
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
    /// it is in the Inbox. In a folder that is a table, the document is a
    /// row of it.
    ///
    /// Always `null` for journals.
    pub folder_id: Option<i64>,

    /// Whether the document is pinned to the Dashboard, as part of the
    /// user's working set.
    pub pinned: bool,
}

/// How many characters of a document's text [`DocumentSummary::opening`]
/// holds, as a literal, so [`OPENING_CHARS`] and [`OPENING_SQL`] are one
/// number.
macro_rules! opening_chars {
    () => {
        500
    };
}

/// How many characters of a document's text [`DocumentSummary::opening`]
/// holds.
pub const OPENING_CHARS: usize = opening_chars!();

/// [`DocumentSummary::opening`] in SQL: the first [`OPENING_CHARS`]
/// characters of the text.
const OPENING_SQL: &str = concat!("substr(documents.contents, 1, ", opening_chars!(), ")");

/// A document as a listing shows it: everything but its full text, with the
/// start of it to show a line of. Scripts see it as `Document`, whose
/// `contents` reads the text when asked.
#[derive(Clone, Serialize, Deserialize, Projection, Fields, TsWire)]
#[projection(of = Document)]
pub struct DocumentSummary {
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
    /// The first [`OPENING_CHARS`] characters of the document's text, as
    /// plain text / Markdown; the whole text when it is shorter.
    #[projection(expr = OPENING_SQL)]
    #[fields(skip)]
    pub opening: String,
    /// When the contents last changed, in Unix **seconds** (not
    /// milliseconds; in JavaScript use `new Date(modified * 1000)`).
    pub modified: i64,
    /// For journals, the day the entry belongs to, as `"YYYY-MM-DD"`.
    ///
    /// `null` for normal documents.
    pub journal_date: Option<String>,
    /// Id of the document folder this document is filed in, or `null` when
    /// it is in the Inbox. In a folder that is a table, the document is a
    /// row of it.
    ///
    /// Always `null` for journals.
    pub folder_id: Option<i64>,
    /// Whether the document is pinned to the Dashboard, as part of the
    /// user's working set.
    pub pinned: bool,
}

impl From<Document> for DocumentSummary {
    fn from(document: Document) -> Self {
        Self {
            id: document.id,
            workspace_id: document.workspace_id,
            deleted: document.deleted,
            document_type: document.document_type,
            title: document.title,
            opening: document.contents.chars().take(OPENING_CHARS).collect(),
            modified: document.modified,
            journal_date: document.journal_date,
            folder_id: document.folder_id,
            pinned: document.pinned,
        }
    }
}
