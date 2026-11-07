use cohesion_macros::{ColumnGroup, Model, TextEnum, TsWire};
use serde::{Deserialize, Serialize};

use super::{TagFilter, TaskPriority, TaskSize, TaskSort};
use crate::value::{FromValue, Value, ValueError};

/// A way of looking at a workspace's tasks kept under a name: which tasks it
/// lists, the tags, priorities and sizes that narrow them, and the order they
/// come in.
#[derive(Clone, Debug, Serialize, Deserialize, Model, TsWire)]
#[model(table = "saved_task_views", backup = Included)]
pub struct SavedTaskView {
    pub id: i64,
    pub workspace_id: i64,
    /// Unique, ignoring case, among the workspace's views.
    pub name: String,
    /// The tasks listed before `tags` narrow them.
    #[model(group)]
    pub scope: TaskViewScope,
    /// The tags a listed task must have, and those it must not.
    #[model(expr = SavedTaskView::TAGS_SQL)]
    pub tags: TagFilter,
    /// The priorities a listed task has one of; every priority when empty.
    #[model(expr = SavedTaskView::PRIORITIES_SQL)]
    pub priorities: TaskPriorityList,
    /// The sizes a listed task has one of; every task, sized or not, when
    /// empty.
    #[model(expr = SavedTaskView::SIZES_SQL)]
    pub sizes: TaskSizeList,
    pub sort: TaskSort,
    /// Reverses `sort`.
    pub descending: bool,
    /// Where the view sits among its workspace's views; lower comes first.
    pub sort_order: i64,
}

impl SavedTaskView {
    pub const PRIORITIES_SQL: &str = "(SELECT json_group_array(priority ORDER BY priority) \
        FROM saved_task_view_priorities WHERE view_id = saved_task_views.id)";
    pub const SIZES_SQL: &str = "(SELECT json_group_array(size ORDER BY size) \
        FROM saved_task_view_sizes WHERE view_id = saved_task_views.id)";
    pub const TAGS_SQL: &str = "json_object(\
        'with', json((SELECT json_group_array(tag_id) FROM saved_task_view_tags \
            WHERE view_id = saved_task_views.id AND tag_match = 'with')), \
        'without', json((SELECT json_group_array(tag_id) FROM saved_task_view_tags \
            WHERE view_id = saved_task_views.id AND tag_match = 'without')))";
}

/// The priorities a saved view lists; read from a JSON array of their numbers.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TsWire)]
#[serde(transparent)]
pub struct TaskPriorityList(pub Vec<TaskPriority>);

impl FromValue for TaskPriorityList {
    fn from_value(value: Value) -> Result<Self, ValueError> {
        let text = String::from_value(value)?;
        let numbers: Vec<i64> =
            serde_json::from_str(&text).map_err(|e| ValueError::Invalid(e.to_string()))?;
        numbers
            .into_iter()
            .map(|number| TaskPriority::from_value(Value::Integer(number)))
            .collect::<Result<_, _>>()
            .map(Self)
    }
}

/// The sizes a saved view lists; read from a JSON array.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TsWire)]
#[serde(transparent)]
pub struct TaskSizeList(pub Vec<TaskSize>);

impl FromValue for TaskSizeList {
    fn from_value(value: Value) -> Result<Self, ValueError> {
        let text = String::from_value(value)?;
        serde_json::from_str(&text)
            .map(Self)
            .map_err(|e| ValueError::Invalid(e.to_string()))
    }
}

/// One priority a saved view lists.
#[derive(Clone, Model)]
#[model(table = "saved_task_view_priorities", backup = Included)]
pub struct SavedTaskViewPriority {
    pub id: i64,
    pub view_id: i64,
    pub priority: TaskPriority,
}

/// One size a saved view lists.
#[derive(Clone, Model)]
#[model(table = "saved_task_view_sizes", backup = Included)]
pub struct SavedTaskViewSize {
    pub id: i64,
    pub view_id: i64,
    pub size: TaskSize,
}

/// The tasks a saved view lists: one of the lists every workspace has, or
/// the open tasks of one of its folders. On the wire, a list's name or the
/// folder's id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ColumnGroup, TsWire)]
#[ts_wire(type = "'inbox' | 'all' | 'done' | 'cancelled' | number")]
#[serde(from = "ScopeRepr", into = "ScopeRepr")]
pub enum TaskViewScope {
    List { list: TaskViewList },
    Folder { task_folder_id: i64 },
}

/// The lists every workspace has.
///
/// - `"inbox"`: the open tasks in no folder.
/// - `"all"`: every open task.
/// - `"done"`: every completed task.
/// - `"cancelled"`: every cancelled task.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
pub enum TaskViewList {
    Inbox,
    All,
    Done,
    Cancelled,
}

/// [`TaskViewScope`] on the wire: a bare folder id or a list's name.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum ScopeRepr {
    Folder(i64),
    List(TaskViewList),
}

impl From<ScopeRepr> for TaskViewScope {
    fn from(repr: ScopeRepr) -> Self {
        match repr {
            ScopeRepr::Folder(task_folder_id) => Self::Folder { task_folder_id },
            ScopeRepr::List(list) => Self::List { list },
        }
    }
}

impl From<TaskViewScope> for ScopeRepr {
    fn from(scope: TaskViewScope) -> Self {
        match scope {
            TaskViewScope::List { list } => Self::List(list),
            TaskViewScope::Folder { task_folder_id } => Self::Folder(task_folder_id),
        }
    }
}

/// One tag a saved view narrows by, and whether a listed task must have it
/// or must not.
#[derive(Clone, Serialize, Deserialize, Model)]
#[model(table = "saved_task_view_tags", backup = Included)]
pub struct SavedTaskViewTag {
    pub id: i64,
    pub workspace_id: i64,
    pub view_id: i64,
    pub tag_id: i64,
    pub tag_match: TagMatch,
}

/// Whether a listed task must have a tag or must not.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum)]
#[serde(rename_all = "camelCase")]
pub enum TagMatch {
    With,
    Without,
}
