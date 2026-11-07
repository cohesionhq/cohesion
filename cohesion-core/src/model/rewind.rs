//! Rewind: the copies the database keeps of a task's and a document's text
//! as they change. Triggers on `tasks` and `documents` write them, so no
//! write can go unrecorded; clients and scripts read them and restore from
//! them, and only the hourly cleanup ever removes one.

use cohesion_macros::{ColumnGroup, Fields, Model, Projection, TextEnum, TsWire};
use serde::{Deserialize, Serialize};

use super::{Document, Task, TaskStatus};
use crate::ts_documentation::TsDocumentation;

/// What a kept version records; stored as text.
///
/// - `"created"`: the item as it was made.
/// - `"edited"`: the item after an edit. Edits within two minutes of a
///   version's start share it, whoever makes them.
/// - `"deleted"`: the item as it was when it was deleted.
/// - `"restored"`: the item after an older version was restored.
/// - `"earlier"`: the item before Rewind has a record of who changed it;
///   the first change after its older versions have gone keeps it.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum VersionChange {
    Created,
    Edited,
    Deleted,
    Restored,
    Earlier,
}

/// Who made a kept version; stored as text.
///
/// - `"user"`: the user, from any client.
/// - `"agent"`: an agent, named by the version's `author_name`.
/// - `"extension"`: an extension, named by the version's `author_name`.
/// - `"script"`: a script run by hand, outside any extension or agent.
/// - `"unknown"`: nobody on record, for an `"earlier"` version.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum VersionAuthor {
    User,
    Agent,
    Extension,
    Script,
    Unknown,
}

/// Who the command the database is running writes as, and whether it is
/// restoring a version: what the triggers that keep versions read, since a
/// trigger sees nothing of the command. One row, set by the database's
/// worker before each write command; no client or script reaches it.
#[derive(Clone, Model)]
#[model(table = "rewind_writer", backup = Included)]
pub struct RewindWriter {
    pub id: i64,
    pub author: VersionAuthor,
    pub agent_id: Option<i64>,
    pub extension: Option<String>,
    pub restoring: bool,
}

/// What a kept version is of, and what only that kind of item keeps. Real
/// foreign keys, so an item's versions cascade with it, and the table's checks
/// hold a version to exactly one.
///
/// - `"task"`: a task, with its status then.
/// - `"document"`: a document or journal entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ColumnGroup, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum VersionedItem {
    Task { task_id: i64, status: TaskStatus },
    Document { document_id: i64 },
}

/// A task's or document's text at one moment.
#[derive(Clone, Debug, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "versions", backup = Included)]
pub struct Version {
    /// Unique, stable identifier.
    pub id: i64,
    /// What it is a version of, and for a task, its status then.
    #[model(group)]
    pub item: VersionedItem,
    /// Id of the workspace the item belongs to.
    pub workspace_id: i64,
    /// The item's title then.
    pub title: String,
    /// A task's description, or a document's full text, then.
    pub body: String,
    /// What this version records.
    pub change: VersionChange,
    /// Who made it, or last edited it.
    pub author: VersionAuthor,
    /// The agent's or extension's name at the time, if `author` is either.
    pub author_name: Option<String>,
    /// When this version began, in Unix **seconds**.
    pub created: i64,
    /// When it last changed, in Unix **seconds**: its last edit.
    pub modified: i64,
}

/// A kept version without its text: a row of an item's history.
#[derive(Clone, Debug, Serialize, Deserialize, Fields, Projection, TsWire)]
#[projection(of = Version)]
#[projection(left_join = Task, on = "tasks.id = versions.task_id")]
#[projection(left_join = Document, on = "documents.id = versions.document_id")]
pub struct VersionSummary {
    /// Unique, stable identifier.
    pub id: i64,
    /// What it is a version of, and for a task, its status then.
    #[projection(group)]
    pub item: VersionedItem,
    /// Id of the workspace the item belongs to.
    pub workspace_id: i64,
    /// What this version records.
    pub change: VersionChange,
    /// Who made it, or last edited it.
    pub author: VersionAuthor,
    /// The agent's or extension's name at the time, if `author` is either.
    pub author_name: Option<String>,
    /// When it last changed, in Unix **seconds**: its last edit.
    pub modified: i64,
    /// Whether this is what the item holds now; `false` for every version of
    /// a deleted item, which restoring any of them brings back.
    #[projection(
        expr = "(coalesce(tasks.deleted, documents.deleted) = 0 AND versions.id = \
                         (SELECT MAX(latest.id) FROM versions latest \
                         WHERE latest.task_id = versions.task_id \
                         OR latest.document_id = versions.document_id))"
    )]
    pub current: bool,
}

/// A deleted item Rewind can still bring back: what its `"deleted"` version
/// holds.
#[derive(Clone, Debug, Serialize, Deserialize, Projection, TsWire)]
#[projection(of = Version)]
#[projection(left_join = Task, on = "tasks.id = versions.task_id")]
#[projection(left_join = Document, on = "documents.id = versions.document_id")]
pub struct DeletedItem {
    /// What was deleted.
    #[projection(group)]
    pub item: VersionedItem,
    pub title: String,
    /// For a journal entry, the day it belongs to.
    #[projection(from = Document)]
    pub journal_date: Option<String>,
    /// Who deleted it.
    pub author: VersionAuthor,
    pub author_name: Option<String>,
    /// When it was deleted, in Unix seconds.
    pub modified: i64,
}

/// What restoring a version gave back: the item as it now is.
#[derive(Clone, Serialize, Deserialize, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RestoredItem {
    Task { task: Task },
    Document { document: Document },
}

/// Which item a history is asked of.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum VersionTarget {
    Task { task_id: i64 },
    Document { document_id: i64 },
}

impl TsDocumentation for VersionedItem {
    const TS: &'static str = Self::TS_BRIEF;
    const TS_BRIEF: &'static str = "/**
 * What a kept version is of: a task, with its status then, or a document.
 */
type VersionedItem =
  | { kind: \"task\"; taskId: number; status: TaskStatus }
  | { kind: \"document\"; documentId: number };";
}
