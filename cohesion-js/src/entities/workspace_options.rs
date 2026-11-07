//! Options objects that `Workspace` methods take.

use std::collections::HashMap;

use cohesion_core::model::{NewContactMethod, TagFilter};
use cohesion_core::{
    DayFilter, TaskAssignee, TaskFolderFilter, TaskPriority, TaskSize, TaskSort, TaskStatus,
};
use cohesion_db::TaskListing;
use cohesion_macros::JsOptions;

use crate::{conversion::OneOrMore, entities::task_folder::JsTaskFolder};

/// Options for `Workspace.tasks()`.
#[derive(JsOptions, Default)]
pub struct TaskListOptions {
    /// Only tasks with these IDs.
    pub ids: Option<Vec<i64>>,
    /// Only tasks with this status, or with any of these statuses
    /// (`{ status: ["completed", "cancelled"] }`).
    ///
    /// An unknown status throws a `TypeError`.
    pub status: Option<OneOrMore<TaskStatus>>,
    /// Only tasks in no folder (`{ inboxOnly: true }`).
    pub inbox_only: Option<bool>,
    /// Only tasks carrying every one of these `setMetadata` pairs
    /// (`{ metadata: { todoist_id: "123" } }`).
    ///
    /// The status filter still applies, so to find a task in any state also
    /// pass `status: ["incomplete", "completed", "cancelled"]`.
    pub metadata: Option<HashMap<String, String>>,
    /// Only tasks carrying this `setMetadata` key, or every one of these keys,
    /// whatever the value (`{ hasMetadata: "todoist_id" }`).
    ///
    /// Combines with `metadata` and `status`.
    pub has_metadata: Option<OneOrMore<String>>,
    /// Only tasks with this priority, or with any of these priorities
    /// (`{ priority: ["high", "urgent"] }`).
    ///
    /// Combines with `status`, so completed tasks stay hidden unless `status`
    /// includes them. An unknown priority throws a `TypeError`.
    pub priority: Option<OneOrMore<TaskPriority>>,
    /// Only tasks of this size, or of any of these sizes (`{ size: [4, 5] }`);
    /// a task with no size never matches.
    ///
    /// Combines with `status`. A size other than a whole number from 1 to 5
    /// throws a `TypeError`.
    pub size: Option<OneOrMore<TaskSize>>,
    /// Only tasks starting on this day, any of these days, or between these
    /// bounds (`{ startDate: { before: "2026-10-20" } }`).
    ///
    /// Combines with `status`, so completed tasks stay hidden unless `status`
    /// includes them.
    pub start_date: Option<DayFilter>,
    /// Only tasks due on this day, any of these days, or between these
    /// bounds (`{ dueDate: ["2026-10-20", "2026-10-21"] }`).
    pub due_date: Option<DayFilter>,
    /// Only tasks with this tag, by name, ignoring case (`{ tag: "errand" }`);
    /// none when the workspace has no such tag.
    ///
    /// Combines with `status`.
    pub tag: Option<String>,
    /// The order tasks come back in (`{ sort: TaskSort.dueDate }`); creation
    /// order, oldest first, when left out.
    ///
    /// Ties keep creation order, and on a date sort tasks without that date
    /// come last. An unknown sort throws a `TypeError`.
    pub sort: Option<TaskSort>,
    /// Reverses `sort` (`{ sort: TaskSort.priority, descending: true }` for
    /// most urgent first); tasks without a date still come last.
    pub descending: Option<bool>,
}

impl TaskListOptions {
    pub(crate) fn folder_filter(&self) -> TaskFolderFilter {
        match self.inbox_only {
            Some(true) => TaskFolderFilter::Inbox,
            _ => TaskFolderFilter::Any,
        }
    }

    /// The listing these options ask for, within the given scope.
    pub(crate) fn into_listing(
        self,
        workspace_id: Option<i64>,
        folder: TaskFolderFilter,
        tags: TagFilter,
    ) -> TaskListing {
        TaskListing {
            tags,
            ids: self.ids,
            workspace_id,
            folder,
            statuses: self.status.map(OneOrMore::into_vec),
            priorities: self.priority.map(OneOrMore::into_vec),
            sizes: self.size.map(OneOrMore::into_vec),
            start_date: self.start_date,
            due_date: self.due_date,
            assignee: None,
            metadata: self.metadata,
            has_metadata: self.has_metadata.map(OneOrMore::into_vec),
            sort: self.sort.unwrap_or_default(),
            descending: self.descending.unwrap_or_default(),
        }
    }
}

/// Options for `Workspace.createTask()`.
#[derive(JsOptions, Default)]
pub struct TaskCreateOptions {
    pub title: String,
    /// The folder to file the task in, from this workspace
    /// (`{ folder: workspace.taskFolders()[0] }`).
    ///
    /// In no folder when left out. A folder from another workspace throws.
    pub folder: Option<JsTaskFolder>,
    /// How urgent the task is (`{ priority: TaskPriority.high }`); `"none"`
    /// when left out.
    ///
    /// An unknown priority throws.
    pub priority: Option<TaskPriority>,
    /// How much time or effort the task takes, from 1 to 5 (`{ size: 2 }`);
    /// none when left out.
    ///
    /// Any other number throws.
    pub size: Option<TaskSize>,
    /// Who the task is assigned to (`{ assignee: { kind: "user" } }`,
    /// `{ kind: "agent", agentId: agent.id }` or
    /// `{ kind: "contact", contactId: contact.id }`); no one when left out.
    ///
    /// Any other `kind` throws, and so does an agent that doesn't exist or a
    /// contact that isn't a live one of the workspace.
    pub assignee: Option<TaskAssignee>,
    /// Notes on the task, as Markdown; none when left out.
    pub description: Option<String>,
    /// The day work can begin, as `"YYYY-MM-DD"` (`"2026-10-20"`); none when
    /// left out.
    ///
    /// Any other text throws.
    pub start_date: Option<String>,
    /// The day the task is due, in the same form as `startDate`; none when
    /// left out.
    pub due_date: Option<String>,
}

/// Options for `Workspace.documents()`.
#[derive(JsOptions, Default)]
pub struct DocumentListOptions {
    /// Only documents carrying every one of these `setMetadata` pairs
    /// (`{ metadata: { notion_id: "abc" } }`).
    pub metadata: Option<HashMap<String, String>>,
    /// Only documents carrying this `setMetadata` key, or every one of
    /// these keys, whatever the value (`{ hasMetadata: "notion_id" }`).
    pub has_metadata: Option<OneOrMore<String>>,
}

/// Options for task folder queries.
#[derive(JsOptions, Default)]
pub struct TaskFolderListOptions {
    /// Only tasks carrying every one of these `setMetadata` pairs
    /// (`{ metadata: { todoist_id: "123" } }`).
    ///
    /// The status filter still applies, so to find a task in any state also
    /// pass `status: ["incomplete", "completed", "cancelled"]`.
    pub metadata: Option<HashMap<String, String>>,
    /// Only tasks carrying this `setMetadata` key, or every one of these keys,
    /// whatever the value (`{ hasMetadata: "todoist_id" }`).
    ///
    /// Combines with `metadata` and `status`.
    pub has_metadata: Option<OneOrMore<String>>,
}

/// Options for creating task folders.
#[derive(JsOptions, Default)]
pub struct TaskFolderCreateOptions {
    pub title: String,
}

/// Options for `Workspace.createContact()`.
#[derive(JsOptions)]
pub struct ContactCreateOptions {
    /// May not be empty.
    pub first_name: String,
    /// None when left out or empty.
    pub middle_name: Option<String>,
    /// May not be empty.
    pub last_name: String,
    /// The email addresses and phone numbers that reach the contact
    /// (`[{ kind: "email", value: "ada@example.com" }]`); none when left out.
    pub methods: Option<Vec<NewContactMethod>>,
    /// Empty when left out.
    pub notes: Option<String>,
}

/// Options for `Workspace.resources()`.
#[derive(JsOptions, Default)]
pub struct ResourceListOptions {
    /// Only resources of this kind, as `Resource.kind` spells it, e.g.
    /// `"com.cohesion.notion.page"`.
    pub kind: Option<String>,
    /// Only resources whose title contains this text, ignoring case.
    pub search: Option<String>,
}
