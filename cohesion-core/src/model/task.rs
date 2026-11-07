use cohesion_macros::{Fields, Model, TextEnum, TsWire};
use serde::{Deserialize, Serialize};

use super::TagIds;
use crate::stored_model::ColumnGroup;
use crate::ts_documentation::TsDocumentation;
use crate::value::{FromValue, IntoValue, Row, RowError, Value, ValueError, read_column};

/// Where a task stands; stored as text.
///
/// Unknown stored text reads as `"incomplete"`.
///
/// - `"incomplete"`: still to do (new tasks start here).
/// - `"completed"`: done.
/// - `"cancelled"`: will not be done.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(default = Incomplete)]
pub enum TaskStatus {
    Incomplete,
    Completed,
    Cancelled,
}

/// How urgent a task is; stored as a number so tasks sort by it, `"none"`
/// lowest.
///
/// Unknown stored numbers read as `"none"`.
///
/// - `"none"`: no priority set (new tasks start here).
/// - `"low"`
/// - `"medium"`
/// - `"high"`
/// - `"urgent"`
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(default = None, store = "integer")]
pub enum TaskPriority {
    None = 0,
    Low = 1,
    Medium = 2,
    High = 3,
    Urgent = 4,
}

/// How much time or effort a task takes, from 1 (the least) to 5 (the
/// most); a bare number on the wire and in the database.
///
/// Clients name each size with the vocabulary's `task_sizes`, never their
/// own words, so the names can change without the numbers changing.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, TsWire)]
#[ts_wire(type = "1 | 2 | 3 | 4 | 5")]
#[serde(try_from = "u8", into = "u8")]
pub struct TaskSize(u8);

impl TaskSize {
    /// Every size, smallest first.
    pub const ALL: [Self; 5] = [Self(1), Self(2), Self(3), Self(4), Self(5)];

    /// The size's number, 1 to 5.
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for TaskSize {
    type Error = ValueError;

    fn try_from(number: u8) -> Result<Self, ValueError> {
        match number {
            1..=5 => Ok(Self(number)),
            other => Err(ValueError::Invalid(format!(
                "a task size is 1 to 5, not {other}"
            ))),
        }
    }
}

impl From<TaskSize> for u8 {
    fn from(size: TaskSize) -> Self {
        size.0
    }
}

impl IntoValue for TaskSize {
    fn into_value(self) -> Value {
        Value::Integer(i64::from(self.0))
    }
}

/// The schema's `CHECK` keeps a stored size within 1 to 5.
impl FromValue for TaskSize {
    fn from_value(value: Value) -> Result<Self, ValueError> {
        let number = i64::from_value(value)?;
        u8::try_from(number)
            .map_err(|_| ValueError::Invalid(format!("a task size is 1 to 5, not {number}")))?
            .try_into()
    }
}

impl TsDocumentation for TaskSize {
    const TS: &'static str = "/**
 * How much time or effort a task takes, from 1 (the least) to 5 (the most).
 *
 * Name a size as the apps do with `cohesion.vocabulary().task_sizes`.
 */
type TaskSize = 1 | 2 | 3 | 4 | 5;";
    const TS_BRIEF: &'static str = "/**
 * How much time or effort a task takes, from 1 (the least) to 5 (the most).
 */
type TaskSize = 1 | 2 | 3 | 4 | 5;";
}

/// Who a task is assigned to: the user, one of their agents, or one of the
/// task's workspace's contacts.
///
/// - `"user"`: the user.
/// - `"agent"`: agent `agent_id`. Deleting the agent leaves the task
///   unassigned.
/// - `"contact"`: contact `contact_id`, of the task's workspace. Deleting the
///   contact leaves the task unassigned.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TaskAssignee {
    User,
    Agent { agent_id: i64 },
    Contact { contact_id: i64 },
}

/// A task's assignee as three columns, `assigned_to_user`,
/// `assignee_agent_id` and `assignee_contact_id`; the table's checks keep
/// more than one from being set, its triggers keep a contact to the task's
/// workspace, and deleting the agent or contact clears its column.
impl ColumnGroup for Option<TaskAssignee> {
    const COLUMNS: &'static [&'static str] = &[
        "assigned_to_user",
        "assignee_agent_id",
        "assignee_contact_id",
    ];

    fn from_row(row: &dyn Row) -> Result<Self, RowError> {
        let user: bool = read_column(row, "assigned_to_user")?;
        let agent_id: Option<i64> = read_column(row, "assignee_agent_id")?;
        let contact_id: Option<i64> = read_column(row, "assignee_contact_id")?;
        Ok(match (user, agent_id, contact_id) {
            (_, Some(agent_id), _) => Some(TaskAssignee::Agent { agent_id }),
            (_, None, Some(contact_id)) => Some(TaskAssignee::Contact { contact_id }),
            (true, None, None) => Some(TaskAssignee::User),
            (false, None, None) => None,
        })
    }

    fn into_values(self) -> Vec<Value> {
        let (user, agent_id, contact_id) = match self {
            None => (false, None, None),
            Some(TaskAssignee::User) => (true, None, None),
            Some(TaskAssignee::Agent { agent_id }) => (false, Some(agent_id), None),
            Some(TaskAssignee::Contact { contact_id }) => (false, None, Some(contact_id)),
        };
        vec![
            user.into_value(),
            agent_id.into_value(),
            contact_id.into_value(),
        ]
    }
}

impl TsDocumentation for TaskAssignee {
    const TS: &'static str = "/**
 * Who a task is assigned to: the user, one of their agents by id, or one of
 * the task's workspace's contacts by id.
 *
 * Deleting the agent or contact leaves the task unassigned. Look the agent
 * up with `cohesion.agent(agentId)`, and the contact with
 * `workspace.contact(contactId)`.
 */
type TaskAssignee =
  | { kind: \"user\" }
  | { kind: \"agent\"; agentId: number }
  | { kind: \"contact\"; contactId: number };";
    const TS_BRIEF: &'static str = "/**
 * Who a task is assigned to: the user, one of their agents by id, or one of
 * the task's workspace's contacts by id.
 */
type TaskAssignee =
  | { kind: \"user\" }
  | { kind: \"agent\"; agentId: number }
  | { kind: \"contact\"; contactId: number };";
}

/// Which folders a task listing covers.
///
/// - `"any"`: every task, filed or not (the default).
/// - `"inbox"`: only tasks in no folder.
/// - a folder id: only tasks filed in that folder.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, TsWire)]
#[ts_wire(type = "'any' | 'inbox' | number")]
#[serde(from = "FolderFilterRepr", into = "FolderFilterRepr")]
pub enum TaskFolderFilter {
    #[default]
    Any,
    Inbox,
    Folder(i64),
}

/// The order a task listing comes back in.
///
/// Ties fall back to creation order, and tasks with no value for the key (no
/// size, start or due day) come last in either direction.
///
/// - `"created"`: oldest first (the default).
/// - `"title"`: alphabetical, ignoring case.
/// - `"priority"`: `"none"` first, `"urgent"` last.
/// - `"size"`: smallest first.
/// - `"startDate"`: earliest start day first.
/// - `"dueDate"`: earliest due day first.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum TaskSort {
    #[default]
    Created,
    Title,
    Priority,
    Size,
    StartDate,
    DueDate,
}

/// [`TaskFolderFilter`] on the wire: a bare folder id or a keyword.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum FolderFilterRepr {
    Folder(i64),
    Keyword(FolderKeyword),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum FolderKeyword {
    Any,
    Inbox,
}

impl From<FolderFilterRepr> for TaskFolderFilter {
    fn from(repr: FolderFilterRepr) -> Self {
        match repr {
            FolderFilterRepr::Folder(id) => Self::Folder(id),
            FolderFilterRepr::Keyword(FolderKeyword::Any) => Self::Any,
            FolderFilterRepr::Keyword(FolderKeyword::Inbox) => Self::Inbox,
        }
    }
}

impl From<TaskFolderFilter> for FolderFilterRepr {
    fn from(filter: TaskFolderFilter) -> Self {
        match filter {
            TaskFolderFilter::Any => Self::Keyword(FolderKeyword::Any),
            TaskFolderFilter::Inbox => Self::Keyword(FolderKeyword::Inbox),
            TaskFolderFilter::Folder(id) => Self::Folder(id),
        }
    }
}

/// A to-do item in a workspace, optionally filed in a task folder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "tasks", backup = Included)]
pub struct Task {
    /// Unique, stable identifier.
    pub id: i64,
    /// Id of the workspace this task belongs to.
    pub workspace_id: i64,
    /// Id of the task folder this task is filed in, or `null` when it is not
    /// in a folder.
    pub task_folder_id: Option<i64>,
    /// What the task is.
    pub title: String,
    /// Notes on the task, as Markdown; `""` when it has none.
    ///
    /// Searched along with the title.
    pub description: String,
    /// Where the task stands.
    pub status: TaskStatus,
    /// How urgent the task is.
    pub priority: TaskPriority,
    /// How much time or effort the task takes, from 1 to 5.
    ///
    /// `null` when it has none.
    pub size: Option<TaskSize>,
    /// Who the task is assigned to; `null` when no one is.
    #[model(group)]
    pub assignee: Option<TaskAssignee>,
    /// The day work on the task can begin, as `"YYYY-MM-DD"`; a calendar day,
    /// the same wherever it is read.
    ///
    /// `null` when it has none.
    pub start_date: Option<String>,
    /// The day the task is due, in the same form as `start_date`.
    ///
    /// `null` when it has none.
    pub due_date: Option<String>,
    /// Whether the task has been soft-deleted.
    pub deleted: bool,
    /// Ids of the tags on the task.
    #[model(expr = Task::TAG_IDS_SQL)]
    #[fields(skip)]
    pub tag_ids: TagIds,
}

impl Task {
    pub const TAG_IDS_SQL: &str =
        "(SELECT json_group_array(tag_id) FROM task_tags WHERE task_id = tasks.id)";
}
