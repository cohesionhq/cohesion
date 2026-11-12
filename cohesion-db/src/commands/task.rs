use std::collections::HashMap;

use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{TagFilter, TaskTag};
use cohesion_core::protocol::{
    ChangeTasksStatusRequest, CreateTaskRequest, DeleteTaskRequest, EditTaskDescriptionRequest,
    ListTasksRequest, MoveTaskRequest, RenameTaskRequest, ScheduleTaskRequest,
    SetTaskAssigneeRequest, SetTaskPriorityRequest, SetTaskSizeRequest, TaskPage,
};
use cohesion_core::value::Value;
use cohesion_core::{
    ColumnGroup, DayFilter, FromRequest, Task, TaskAssignee, TaskFolder, TaskFolderFilter,
    TaskPriority, TaskSize, TaskSort, TaskStatus,
};

use crate::changed::{Changed, Invalidates};
use crate::commands::backlink::FollowLinksCommand;
use crate::commands::page::page_size;
use crate::commands::tag::tasks_tagged;
use crate::commands::title::ensure_title;
use crate::commands::workspace_scope::ensure_workspace;
use crate::{
    CommandContext, CommandError, DatabaseCommand, DatabaseCommandResult, DatabaseEventKind,
    Filtered, Found, Read, ReadableDatabase, Select, ValidationError, Write,
};

/// The invalidation for the list `task` sits in.
pub(crate) fn task_list(task: &Task) -> Invalidation {
    Invalidation::TaskList {
        workspace_id: task.workspace_id,
        folder_id: task.task_folder_id,
    }
}

/// `task`, just written, having changed the list it sits in, and keeping
/// what its description links to ([`followed`]).
fn listed(ctx: &mut CommandContext<'_>, task: Task) -> DatabaseCommandResult<Changed<Task>> {
    followed(ctx, &task)?;
    let key = task_list(&task);
    Ok(task.invalidating([key]))
}

/// Keeps what `task`, just written, links to ([`FollowLinksCommand`]).
/// Every write of a task runs it, most through [`listed`].
pub(crate) fn followed(ctx: &mut CommandContext<'_>, task: &Task) -> DatabaseCommandResult<()> {
    ctx.run(FollowLinksCommand::of(task))
}

/// Fails with a validation error unless `workspace_id` has a live task folder
/// `folder_id`: a task is put only in a folder that lists it.
fn ensure_folder<'c>(
    db: impl ReadableDatabase<'c>,
    workspace_id: i64,
    folder_id: i64,
) -> DatabaseCommandResult<()> {
    let exists = db
        .select_live::<TaskFolder>()
        .filter_eq(TaskFolder::WORKSPACE_ID, workspace_id)
        .filter_eq(TaskFolder::ID, folder_id)
        .exists()?;
    if exists {
        Ok(())
    } else {
        Err(CommandError::Validation(ValidationError::new(
            "That task folder doesn’t exist.",
        )))
    }
}

/// The live tasks some filters cover, in `sort`'s order.
#[derive(Clone)]
pub struct TaskListing {
    pub ids: Option<Vec<i64>>,
    pub workspace_id: Option<i64>,
    pub folder: TaskFolderFilter,
    pub priorities: Option<Vec<TaskPriority>>,
    pub sizes: Option<Vec<TaskSize>>,
    pub start_date: Option<DayFilter>,
    pub due_date: Option<DayFilter>,
    pub assignee: Option<TaskAssignee>,
    pub tags: TagFilter,
    pub statuses: Option<Vec<TaskStatus>>,
    pub metadata: Option<HashMap<String, String>>,
    pub has_metadata: Option<Vec<String>>,
    pub sort: TaskSort,
    pub descending: bool,
}
impl TaskListing {
    fn select<'c>(self, db: impl ReadableDatabase<'c>) -> DatabaseCommandResult<Select<'c, Task>> {
        let mut inner_query = db.select_live::<Task>();

        if let Some(workspace_id) = self.workspace_id {
            ensure_workspace(db, workspace_id)?;
            inner_query = inner_query.filter("workspace_id = ?").bind(workspace_id);
        };
        let statuses = self.statuses.unwrap_or(vec![TaskStatus::Incomplete]);
        let mut query = inner_query
            .with_metadata(self.metadata.unwrap_or_default())
            .with_metadata_keys(self.has_metadata.unwrap_or_default())
            .filter_in_values("status", statuses);
        match self.folder {
            TaskFolderFilter::Any => {}
            TaskFolderFilter::Inbox => query = query.filter("task_folder_id IS NULL"),
            TaskFolderFilter::Folder(folder_id) => {
                query = query.filter("task_folder_id = ?").bind(folder_id)
            }
        }
        if let Some(priorities) = self.priorities {
            query = query.filter_in_values("priority", priorities)
        }
        if let Some(sizes) = self.sizes {
            query = query.filter_in_values("size", sizes)
        }
        if let Some(start_date) = self.start_date {
            query = query.filter_day("start_date", start_date)
        }
        if let Some(due_date) = self.due_date {
            query = query.filter_day("due_date", due_date)
        }
        if let Some(assignee) = self.assignee {
            query = query.filter_group(Some(assignee))
        }
        if let Some(ids) = self.ids {
            query = query.filter_in_values("id", ids)
        }
        for tag_id in self.tags.with {
            query = query.filter_in("id", tasks_tagged(db, tag_id))
        }
        for tag_id in self.tags.without {
            query = query.filter_not_in("id", tasks_tagged(db, tag_id))
        }
        let column = match self.sort {
            TaskSort::Created => "id",
            TaskSort::Title => "title COLLATE NOCASE",
            TaskSort::Priority => "priority",
            TaskSort::Size => "size",
            TaskSort::StartDate => "start_date",
            TaskSort::DueDate => "due_date",
        };
        let direction = if self.descending { "DESC" } else { "ASC" };
        // `IS NULL` sorts false first, so unsized and undated tasks trail
        // either way.
        let order = match self.sort {
            TaskSort::Size | TaskSort::StartDate | TaskSort::DueDate => {
                format!("{column} IS NULL, {column} {direction}, id")
            }
            _ => format!("{column} {direction}, id"),
        };
        Ok(query.order(&order))
    }
}

/// A page of the live tasks the filters cover, in `sort`'s order.
#[derive(FromRequest)]
#[request(ListTasksRequest)]
pub struct ListTaskPageCommand {
    pub ids: Option<Vec<i64>>,
    pub workspace_id: Option<i64>,
    pub folder: TaskFolderFilter,
    pub priorities: Option<Vec<TaskPriority>>,
    pub sizes: Option<Vec<TaskSize>>,
    pub start_date: Option<DayFilter>,
    pub due_date: Option<DayFilter>,
    pub assignee: Option<TaskAssignee>,
    pub tags: TagFilter,
    pub statuses: Option<Vec<TaskStatus>>,
    pub metadata: Option<HashMap<String, String>>,
    pub has_metadata: Option<Vec<String>>,
    pub sort: TaskSort,
    pub descending: bool,
    pub offset: u64,
    pub limit: Option<u64>,
    pub around: Option<i64>,
}
impl DatabaseCommand for ListTaskPageCommand {
    type Response = TaskPage;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<TaskPage> {
        let listing = TaskListing {
            ids: self.ids,
            workspace_id: self.workspace_id,
            folder: self.folder,
            priorities: self.priorities,
            sizes: self.sizes,
            start_date: self.start_date,
            due_date: self.due_date,
            assignee: self.assignee,
            tags: self.tags,
            statuses: self.statuses,
            metadata: self.metadata,
            has_metadata: self.has_metadata,
            sort: self.sort,
            descending: self.descending,
        };
        let limit = page_size(self.limit);
        let offset = match self.around {
            Some(id) => listing
                .clone()
                .select(ctx.db)?
                .position_of(id)?
                .map_or(self.offset, |position| position / limit * limit),
            None => self.offset,
        };
        Ok(TaskPage {
            total: listing.clone().select(ctx.db)?.count()?,
            tasks: listing.select(ctx.db)?.limit(limit).offset(offset).all()?,
            offset,
        })
    }
}

/// Every live task a listing covers, in its order; for scripts, which read
/// them all.
pub struct ListTasksCommand(pub TaskListing);
impl DatabaseCommand for ListTasksCommand {
    type Response = Vec<Task>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Vec<Task>> {
        Ok(self.0.select(ctx.db)?.all()?)
    }
}

pub struct GetTaskCommand {
    pub workspace_id: i64,
    pub task_id: i64,
}
impl DatabaseCommand for GetTaskCommand {
    type Response = Task;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        ctx.db
            .select_live()
            .filter("workspace_id = ? AND id = ?")
            .bind(self.workspace_id)
            .bind(self.task_id)
            .one()
            .found()
    }
}

#[derive(FromRequest)]
#[request(CreateTaskRequest)]
pub struct CreateTaskCommand {
    pub workspace_id: i64,
    pub title: String,
    pub folder_id: Option<i64>,
    pub priority: Option<TaskPriority>,
    pub size: Option<TaskSize>,
    pub assignee: Option<TaskAssignee>,
    pub description: Option<String>,
    pub start_date: Option<String>,
    pub due_date: Option<String>,
    pub tag_ids: Vec<i64>,
}
impl DatabaseCommand for CreateTaskCommand {
    type Response = Task;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Task>> {
        ensure_workspace(ctx.db, self.workspace_id)?;

        if let Some(folder_id) = self.folder_id {
            ensure_folder(ctx.db, self.workspace_id, folder_id)?;
        }

        let task: Task = ctx
            .db
            .insert()
            .set(Task::TITLE, self.title)
            .set(Task::WORKSPACE_ID, self.workspace_id)
            .set(Task::TASK_FOLDER_ID, self.folder_id)
            .set(Task::PRIORITY, self.priority.unwrap_or(TaskPriority::None))
            .set(Task::SIZE, self.size)
            .set_group(self.assignee)
            .set(Task::DESCRIPTION, self.description.unwrap_or_default())
            .set(Task::START_DATE, self.start_date)
            .set(Task::DUE_DATE, self.due_date)
            .returning_one()
            .found()?;
        ctx.events
            .send(ctx.trace.event(DatabaseEventKind::TaskCreated {
                task_id: task.id,
                workspace_id: task.workspace_id,
            }));
        if self.tag_ids.is_empty() {
            return listed(ctx, task);
        }
        // The table's foreign keys refuse a tag of another workspace or kind,
        // which rolls the task back with it.
        for tag_id in self.tag_ids {
            ctx.db
                .insert::<TaskTag>()
                .set(TaskTag::WORKSPACE_ID, task.workspace_id)
                .set(TaskTag::TAG_ID, tag_id)
                .set(TaskTag::TASK_ID, task.id)
                .execute()?;
            ctx.events
                .send(ctx.trace.event(DatabaseEventKind::TaskTagged {
                    task_id: task.id,
                    workspace_id: task.workspace_id,
                }));
        }
        let task = GetTaskCommand {
            workspace_id: task.workspace_id,
            task_id: task.id,
        }
        .handle(&mut ctx.as_read())?;
        listed(ctx, task)
    }
}

pub struct ChangeTaskStatusCommand {
    pub workspace_id: i64,
    pub task_id: i64,
    pub status: TaskStatus,
}
impl DatabaseCommand for ChangeTaskStatusCommand {
    type Response = Task;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        let changed: Option<Task> = ctx
            .db
            .update_live()
            .set(Task::STATUS, self.status.clone())
            .filter("workspace_id = ? AND id = ? AND status != ?")
            .bind(self.workspace_id)
            .bind(self.task_id)
            .bind(self.status)
            .returning_one()?;
        let Some(task) = changed else {
            return Ok(GetTaskCommand {
                workspace_id: self.workspace_id,
                task_id: self.task_id,
            }
            .handle(&mut ctx.as_read())?
            .invalidating_nothing());
        };
        ctx.events
            .send(ctx.trace.event(DatabaseEventKind::TaskStatusChanged {
                task_id: task.id,
                workspace_id: task.workspace_id,
            }));
        listed(ctx, task)
    }
}

#[derive(FromRequest)]
#[request(RenameTaskRequest)]
pub struct ChangeTaskTitleCommand {
    pub workspace_id: i64,
    pub task_id: i64,
    pub title: String,
}
impl DatabaseCommand for ChangeTaskTitleCommand {
    type Response = Task;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ensure_title(&self.title)?;
        ensure_workspace(ctx.db, self.workspace_id)?;
        let changed: Option<Task> = ctx
            .db
            .update_live()
            .set(Task::TITLE, self.title.clone())
            .filter("workspace_id = ? AND id = ? AND title != ?")
            .bind(self.workspace_id)
            .bind(self.task_id)
            .bind(self.title)
            .returning_one()?;
        let Some(task) = changed else {
            return Ok(GetTaskCommand {
                workspace_id: self.workspace_id,
                task_id: self.task_id,
            }
            .handle(&mut ctx.as_read())?
            .invalidating_nothing());
        };
        ctx.events
            .send(ctx.trace.event(DatabaseEventKind::TaskRenamed {
                task_id: task.id,
                workspace_id: task.workspace_id,
            }));
        listed(ctx, task)
    }
}

#[derive(FromRequest)]
#[request(SetTaskPriorityRequest)]
pub struct ChangeTaskPriorityCommand {
    pub workspace_id: i64,
    pub task_id: i64,
    pub priority: TaskPriority,
}
impl DatabaseCommand for ChangeTaskPriorityCommand {
    type Response = Task;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        let changed: Option<Task> = ctx
            .db
            .update_live()
            .set(Task::PRIORITY, self.priority)
            .filter("workspace_id = ? AND id = ? AND priority != ?")
            .bind(self.workspace_id)
            .bind(self.task_id)
            .bind(self.priority)
            .returning_one()?;
        let Some(task) = changed else {
            return Ok(GetTaskCommand {
                workspace_id: self.workspace_id,
                task_id: self.task_id,
            }
            .handle(&mut ctx.as_read())?
            .invalidating_nothing());
        };
        ctx.events
            .send(ctx.trace.event(DatabaseEventKind::TaskPriorityChanged {
                task_id: task.id,
                workspace_id: task.workspace_id,
            }));
        listed(ctx, task)
    }
}

#[derive(FromRequest)]
#[request(SetTaskSizeRequest)]
pub struct ChangeTaskSizeCommand {
    pub workspace_id: i64,
    pub task_id: i64,
    pub size: Option<TaskSize>,
}
impl DatabaseCommand for ChangeTaskSizeCommand {
    type Response = Task;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        let changed: Option<Task> = ctx
            .db
            .update_live()
            .set(Task::SIZE, self.size)
            .filter("workspace_id = ? AND id = ? AND size IS NOT ?")
            .bind(self.workspace_id)
            .bind(self.task_id)
            .bind(self.size)
            .returning_one()?;
        let Some(task) = changed else {
            return Ok(GetTaskCommand {
                workspace_id: self.workspace_id,
                task_id: self.task_id,
            }
            .handle(&mut ctx.as_read())?
            .invalidating_nothing());
        };
        ctx.events
            .send(ctx.trace.event(DatabaseEventKind::TaskSizeChanged {
                task_id: task.id,
                workspace_id: task.workspace_id,
            }));
        listed(ctx, task)
    }
}

#[derive(FromRequest)]
#[request(SetTaskAssigneeRequest)]
pub struct ChangeTaskAssigneeCommand {
    pub workspace_id: i64,
    pub task_id: i64,
    pub assignee: Option<TaskAssignee>,
}
impl DatabaseCommand for ChangeTaskAssigneeCommand {
    type Response = Task;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        let [user, agent_id, contact_id] = <[Value; 3]>::try_from(self.assignee.into_values())
            .expect("an assignee is three columns");
        let changed: Option<Task> = ctx
            .db
            .update_live()
            .set_group(self.assignee)
            .filter(
                "workspace_id = ? AND id = ? \
                 AND NOT (assigned_to_user IS ? AND assignee_agent_id IS ? \
                 AND assignee_contact_id IS ?)",
            )
            .bind(self.workspace_id)
            .bind(self.task_id)
            .bind(user)
            .bind(agent_id)
            .bind(contact_id)
            .returning_one()?;
        let Some(task) = changed else {
            return Ok(GetTaskCommand {
                workspace_id: self.workspace_id,
                task_id: self.task_id,
            }
            .handle(&mut ctx.as_read())?
            .invalidating_nothing());
        };
        ctx.events
            .send(ctx.trace.event(DatabaseEventKind::TaskAssigneeChanged {
                task_id: task.id,
                workspace_id: task.workspace_id,
            }));
        listed(ctx, task)
    }
}

#[derive(FromRequest)]
#[request(EditTaskDescriptionRequest)]
pub struct ChangeTaskDescriptionCommand {
    pub workspace_id: i64,
    pub task_id: i64,
    pub description: String,
}
impl DatabaseCommand for ChangeTaskDescriptionCommand {
    type Response = Task;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        let changed: Option<Task> = ctx
            .db
            .update_live()
            .set(Task::DESCRIPTION, self.description.clone())
            .filter("workspace_id = ? AND id = ? AND description != ?")
            .bind(self.workspace_id)
            .bind(self.task_id)
            .bind(self.description)
            .returning_one()?;
        let Some(task) = changed else {
            return Ok(GetTaskCommand {
                workspace_id: self.workspace_id,
                task_id: self.task_id,
            }
            .handle(&mut ctx.as_read())?
            .invalidating_nothing());
        };
        ctx.events
            .send(ctx.trace.event(DatabaseEventKind::TaskDescriptionChanged {
                task_id: task.id,
                workspace_id: task.workspace_id,
            }));
        followed(ctx, &task)?;
        let key = Invalidation::TaskDescription {
            workspace_id: task.workspace_id,
            task_id: task.id,
        };
        Ok(task.invalidating([key]))
    }
}

#[derive(FromRequest)]
#[request(ScheduleTaskRequest)]
pub struct ScheduleTaskCommand {
    pub workspace_id: i64,
    pub task_id: i64,
    pub start_date: Option<String>,
    pub due_date: Option<String>,
}
impl DatabaseCommand for ScheduleTaskCommand {
    type Response = Task;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        let before = GetTaskCommand {
            workspace_id: self.workspace_id,
            task_id: self.task_id,
        }
        .handle(&mut ctx.as_read())?;
        let task: Task = ctx
            .db
            .update()
            .set(Task::START_DATE, self.start_date)
            .set(Task::DUE_DATE, self.due_date)
            .filter_eq(Task::WORKSPACE_ID, self.workspace_id)
            .filter_eq(Task::ID, self.task_id)
            .returning_one()
            .found()?;
        if task.start_date != before.start_date {
            ctx.events
                .send(ctx.trace.event(DatabaseEventKind::TaskStartDateChanged {
                    task_id: task.id,
                    workspace_id: task.workspace_id,
                }));
        }
        if task.due_date != before.due_date {
            ctx.events
                .send(ctx.trace.event(DatabaseEventKind::TaskDueDateChanged {
                    task_id: task.id,
                    workspace_id: task.workspace_id,
                }));
        }
        listed(ctx, task)
    }
}

#[derive(FromRequest)]
#[request(ChangeTasksStatusRequest)]
pub struct ChangeTasksStatusCommand {
    pub workspace_id: i64,
    pub task_ids: Vec<i64>,
    pub status: TaskStatus,
}
impl DatabaseCommand for ChangeTasksStatusCommand {
    type Response = Vec<Task>;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let changed: Vec<Task> = ctx
            .db
            .update_live()
            .set(Task::STATUS, self.status.clone())
            .filter_in_values("id", self.task_ids)
            .filter("workspace_id = ? AND status != ?")
            .bind(self.workspace_id)
            .bind(self.status)
            .returning_all()?;
        for task in &changed {
            ctx.events
                .send(ctx.trace.event(DatabaseEventKind::TaskStatusChanged {
                    task_id: task.id,
                    workspace_id: task.workspace_id,
                }));
        }
        for task in &changed {
            followed(ctx, task)?;
        }
        let keys: Vec<_> = changed.iter().map(task_list).collect();
        Ok(changed.invalidating(keys))
    }
}

#[derive(FromRequest)]
#[request(MoveTaskRequest)]
pub struct MoveTaskToFolderCommand {
    pub workspace_id: i64,
    pub task_id: i64,
    pub folder_id: Option<i64>,
}
impl DatabaseCommand for MoveTaskToFolderCommand {
    type Response = Task;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Self::Access>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        let before = GetTaskCommand {
            workspace_id: self.workspace_id,
            task_id: self.task_id,
        }
        .handle(&mut ctx.as_read())?;
        if let Some(folder_id) = self.folder_id {
            ensure_folder(ctx.db, self.workspace_id, folder_id)?;
        }
        let v = ctx
            .db
            .update()
            .set(Task::TASK_FOLDER_ID, self.folder_id)
            .filter_eq(Task::WORKSPACE_ID, self.workspace_id)
            .filter_eq(Task::ID, self.task_id)
            .returning_one()
            .found()?;
        ctx.events
            .send(ctx.trace.event(DatabaseEventKind::TaskMoved {
                task_id: self.task_id,
                workspace_id: self.workspace_id,
            }));
        followed(ctx, &v)?;
        let keys = [task_list(&before), task_list(&v)];
        Ok(v.invalidating(keys))
    }
}

/// Soft-deletes a task in a workspace.
#[derive(FromRequest)]
#[request(DeleteTaskRequest)]
pub struct DeleteTaskCommand {
    pub workspace_id: i64,
    pub task_id: i64,
}
impl DatabaseCommand for DeleteTaskCommand {
    type Response = Task;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Task>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        let task = ctx
            .db
            .update_live()
            .set_true(Task::DELETED)
            .filter_eq(Task::WORKSPACE_ID, self.workspace_id)
            .filter_eq(Task::ID, self.task_id)
            .returning_one()
            .found()?;
        ctx.events.send(
            ctx.trace
                .event(DatabaseEventKind::TaskDeleted { task: task.clone() }),
        );
        listed(ctx, task)
    }
}

#[cfg(test)]
mod tests;
