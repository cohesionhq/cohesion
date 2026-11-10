//! The data a case starts from: workspaces and what is in them. A state is
//! written as a script in `cases/states/` whose value is the state, plain
//! data that the bench reads into these types (so a state could as well be
//! JSON), refusing a field it doesn't know. The scratch daemon makes the
//! workspaces, then `B.load` in `cases/prelude.js` fills them through the
//! script API, the same calls an agent makes. Days are counted from today,
//! so "overdue" and "due today" always mean something.

use cohesionkit::model::{ContactMethodKind, TaskPriority, TaskStatus};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct StartingState {
    pub workspaces: Vec<WorkspaceState>,
}

impl StartingState {
    /// The state of a case that names none: an empty `Personal` workspace.
    pub fn empty() -> Self {
        Self {
            workspaces: vec![WorkspaceState::titled("Personal")],
        }
    }

    pub fn workspace_titles(&self) -> Vec<String> {
        self.workspaces.iter().map(|w| w.title.clone()).collect()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct WorkspaceState {
    pub title: String,
    pub tags: Vec<String>,
    /// Task folders, by title, in order.
    pub task_folders: Vec<String>,
    pub contacts: Vec<ContactState>,
    pub tasks: Vec<TaskState>,
    /// Top-level document folders, with what they hold.
    pub document_folders: Vec<DocumentFolderState>,
    /// Documents in the Inbox.
    pub documents: Vec<DocumentState>,
    pub journals: Vec<JournalState>,
}

impl WorkspaceState {
    pub fn titled(title: &str) -> Self {
        Self {
            title: title.to_string(),
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct ContactState {
    pub first_name: String,
    pub last_name: String,
    pub methods: Vec<ContactMethodState>,
    pub notes: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContactMethodState {
    pub kind: ContactMethodKind,
    pub value: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct TaskState {
    pub title: String,
    /// The task folder it is in, by title; none when absent.
    pub folder: Option<String>,
    pub priority: Option<TaskPriority>,
    /// From 1 to 5.
    pub size: Option<u8>,
    /// Days from today.
    pub start: Option<i64>,
    /// Days from today.
    pub due: Option<i64>,
    pub description: Option<String>,
    /// Tag names, from the workspace's tags.
    pub tags: Vec<String>,
    /// `incomplete` when absent.
    pub status: Option<TaskStatus>,
    /// Who it is assigned to; no one when absent.
    pub assignee: Option<AssigneeState>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum AssigneeState {
    User,
    /// One of the workspace's contacts, by name.
    #[serde(rename_all = "camelCase")]
    Contact {
        first_name: String,
        last_name: String,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct DocumentFolderState {
    pub title: String,
    pub folders: Vec<DocumentFolderState>,
    pub documents: Vec<DocumentState>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct DocumentState {
    pub title: String,
    pub contents: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct JournalState {
    /// Days from today: -1 is yesterday's entry.
    pub day: i64,
    pub contents: String,
}

#[cfg(test)]
mod tests;
