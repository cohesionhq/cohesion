//! Requests about tasks, task folders and saved task views.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

use crate::model::{
    DayFilter, IntelligenceAgentColor, TagFilter, Task, TaskAssignee, TaskFolderFilter,
    TaskPriority, TaskSize, TaskSort, TaskStatus, TaskViewScope,
};

/// Lists task folders
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListTaskFoldersRequest {
    pub workspace_id: i64,
}

/// Create task folder
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct CreateTaskFolderRequest {
    pub workspace_id: i64,
    pub title: String,
}

/// Puts a workspace's folders in the order of `folder_ids`, first to last.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ReorderTaskFoldersRequest {
    pub workspace_id: i64,
    /// Every live folder in the workspace, in its new order.
    pub folder_ids: Vec<i64>,
}

/// Delete task folder
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DeleteTaskFolderRequest {
    pub workspace_id: i64,
    pub folder_id: i64,
}

/// Lists one page of the live tasks the filters cover, in `sort`'s order.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct ListTasksRequest {
    pub ids: Option<Vec<i64>>,
    /// Only tasks in this workspace; every workspace when absent.
    pub workspace_id: Option<i64>,
    /// Every task when absent.
    #[serde(default)]
    pub folder: TaskFolderFilter,
    pub statuses: Option<Vec<TaskStatus>>,
    /// Only tasks with these priorities.
    pub priorities: Option<Vec<TaskPriority>>,
    /// Only tasks with these sizes; a task with no size never matches.
    pub sizes: Option<Vec<TaskSize>>,
    /// Only tasks whose start day matches.
    pub start_date: Option<DayFilter>,
    /// Only tasks whose due day matches.
    pub due_date: Option<DayFilter>,
    /// Only tasks assigned to this assignee.
    pub assignee: Option<TaskAssignee>,
    /// Only tasks with every tag in `with` and none in `without`.
    #[serde(default)]
    pub tags: TagFilter,
    /// Creation order when absent.
    #[serde(default)]
    pub sort: TaskSort,
    /// Reverses `sort`; tasks without a value for it still come last.
    #[serde(default)]
    pub descending: bool,
    /// How many of the covered tasks to skip; none when absent.
    #[serde(default)]
    pub offset: u64,
    /// The most tasks to list: 50 when absent, and never over 500.
    pub limit: Option<u64>,
    /// In place of `offset`, start at the page holding task `around`: the
    /// one of pages `limit` tasks long that it falls on. `offset` stands
    /// when the listing doesn't cover the task.
    pub around: Option<i64>,
}

/// One page of a listing: its tasks, and how many the whole listing has.
#[derive(Serialize, Deserialize, Clone, TsWire)]
pub struct TaskPage {
    pub tasks: Vec<Task>,
    /// How many of the covered tasks come before this page's first.
    pub offset: u64,
    /// Every task the listing covers, on any page.
    pub total: i64,
}

/// Create task
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct CreateTaskRequest {
    pub workspace_id: i64,
    pub title: String,
    pub folder_id: Option<i64>,
    /// `"none"` when absent.
    pub priority: Option<TaskPriority>,
    /// None when absent.
    pub size: Option<TaskSize>,
    /// No one when absent.
    pub assignee: Option<TaskAssignee>,
    /// Markdown notes; none when absent.
    pub description: Option<String>,
    /// The start day, as `"YYYY-MM-DD"`; none when absent.
    pub start_date: Option<String>,
    /// The due day, as `"YYYY-MM-DD"`; none when absent.
    pub due_date: Option<String>,
    /// Ids of the workspace's task tags to put on the task; none when absent.
    #[serde(default)]
    pub tag_ids: Vec<i64>,
}

/// Change task's status. Responds with only the tasks whose status changed;
/// tasks already in `status` are left untouched.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ChangeTasksStatusRequest {
    pub workspace_id: i64,
    pub task_ids: Vec<i64>,
    pub status: TaskStatus,
}

/// Retitles task `task_id` in a workspace.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RenameTaskRequest {
    pub workspace_id: i64,
    pub task_id: i64,
    pub title: String,
}

/// Sets the priority of task `task_id` in a workspace.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetTaskPriorityRequest {
    pub workspace_id: i64,
    pub task_id: i64,
    pub priority: TaskPriority,
}

/// Sets the size of task `task_id` in a workspace; `null` clears it.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetTaskSizeRequest {
    pub workspace_id: i64,
    pub task_id: i64,
    pub size: Option<TaskSize>,
}

/// Assigns task `task_id` in a workspace to the user or one of their
/// agents; `null` leaves it unassigned.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetTaskAssigneeRequest {
    pub workspace_id: i64,
    pub task_id: i64,
    pub assignee: Option<TaskAssignee>,
}

/// Replaces the description of task `task_id` in a workspace; `""` clears it.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct EditTaskDescriptionRequest {
    pub workspace_id: i64,
    pub task_id: i64,
    pub description: String,
}

/// Sets both dates of task `task_id` in a workspace, each a `"YYYY-MM-DD"`
/// day; `null` clears one. Any other text is a validation error.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ScheduleTaskRequest {
    pub workspace_id: i64,
    pub task_id: i64,
    pub start_date: Option<String>,
    pub due_date: Option<String>,
}

/// Moves task `task_id` into folder `folder_id`, or the inbox when absent.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct MoveTaskRequest {
    pub workspace_id: i64,
    pub task_id: i64,
    pub folder_id: Option<i64>,
}

/// Deletes task `task_id` in a workspace.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DeleteTaskRequest {
    pub workspace_id: i64,
    pub task_id: i64,
}

/// Retitles task folder `folder_id` in a workspace.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RenameTaskFolderRequest {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub title: String,
}

/// Pins task folder `folder_id` in a workspace to the sidebar, or unpins it.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetTaskFolderPinnedRequest {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub pinned: bool,
}

/// Sets the color task folder `folder_id` in a workspace is drawn in, or
/// takes it away when `color` is left out.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct SetTaskFolderColorRequest {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub color: Option<IntelligenceAgentColor>,
}

/// Lists a workspace's saved task views, in their order.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListSavedTaskViewsRequest {
    pub workspace_id: i64,
}

/// Saves a way of looking at a workspace's tasks under a name, last among
/// its views.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct CreateSavedTaskViewRequest {
    pub workspace_id: i64,
    /// Unique, ignoring case, among the workspace's views.
    pub name: String,
    pub scope: TaskViewScope,
    pub tags: TagFilter,
    /// Every priority when empty.
    pub priorities: Vec<TaskPriority>,
    /// Every task, sized or not, when empty or absent.
    #[serde(default)]
    pub sizes: Vec<TaskSize>,
    pub sort: TaskSort,
    pub descending: bool,
}

/// Replaces what saved view `view_id` lists, and in what order, keeping its
/// name and place.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct UpdateSavedTaskViewRequest {
    pub workspace_id: i64,
    pub view_id: i64,
    pub scope: TaskViewScope,
    pub tags: TagFilter,
    /// Every priority when empty.
    pub priorities: Vec<TaskPriority>,
    /// Every task, sized or not, when empty or absent.
    #[serde(default)]
    pub sizes: Vec<TaskSize>,
    pub sort: TaskSort,
    pub descending: bool,
}

/// Renames saved view `view_id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RenameSavedTaskViewRequest {
    pub workspace_id: i64,
    pub view_id: i64,
    pub name: String,
}

/// Puts a workspace's saved task views in the order of `view_ids`, first to
/// last.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ReorderSavedTaskViewsRequest {
    pub workspace_id: i64,
    /// Every view in the workspace, in its new order.
    pub view_ids: Vec<i64>,
}

/// Deletes saved view `view_id`; its tasks stay as they are.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DeleteSavedTaskViewRequest {
    pub workspace_id: i64,
    pub view_id: i64,
}
