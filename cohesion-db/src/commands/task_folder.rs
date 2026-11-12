use std::collections::HashMap;

use cohesion_core::invalidation::Invalidation;
use cohesion_core::protocol::{
    CreateTaskFolderRequest, DeleteTaskFolderRequest, ListTaskFoldersRequest,
    RenameTaskFolderRequest, ReorderTaskFoldersRequest, SetTaskFolderPinnedRequest,
};
use cohesion_core::{FromRequest, Task, TaskFolder};

use crate::changed::{Changed, Invalidates};
use crate::commands::task::followed;
use crate::commands::title::ensure_title;
use crate::commands::workspace_scope::ensure_workspace;
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, DatabaseEventKind, Filtered, Found,
    Read, ReadableDatabase, Write,
};

/// One task folder by id, deleted or not.
pub struct GetTaskFolderIncludingDeletedCommand {
    pub workspace_id: i64,
    pub folder_id: i64,
}
impl DatabaseCommand for GetTaskFolderIncludingDeletedCommand {
    type Response = TaskFolder;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<TaskFolder> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        ctx.db
            .select()
            .filter("workspace_id = ? AND id = ?")
            .bind(self.workspace_id)
            .bind(self.folder_id)
            .one()
            .found()
    }
}

/// One live task folder by id; a deleted one is not found.
pub struct GetTaskFolderCommand {
    pub workspace_id: i64,
    pub folder_id: i64,
}
impl DatabaseCommand for GetTaskFolderCommand {
    type Response = TaskFolder;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<TaskFolder> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        ctx.db
            .select_live()
            .filter("workspace_id = ? AND id = ?")
            .bind(self.workspace_id)
            .bind(self.folder_id)
            .one()
            .found()
    }
}

/// Every live task folder in a workspace carrying every one of the
/// `metadata` pairs and `has_metadata` keys, in their sort order.
#[derive(FromRequest)]
#[request(ListTaskFoldersRequest)]
pub struct ListTaskFoldersCommand {
    pub workspace_id: i64,
    pub metadata: HashMap<String, String>,
    pub has_metadata: Vec<String>,
}
impl DatabaseCommand for ListTaskFoldersCommand {
    type Response = Vec<TaskFolder>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Vec<TaskFolder>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        Ok(ctx
            .db
            .select_live()
            .filter("workspace_id = ?")
            .bind(self.workspace_id)
            .with_metadata(self.metadata)
            .with_metadata_keys(self.has_metadata)
            .order("sort_order, id")
            .all()?)
    }
}

#[derive(FromRequest)]
#[request(CreateTaskFolderRequest)]
pub struct CreateTaskFolderCommand {
    pub workspace_id: i64,
    pub title: String,
}
impl DatabaseCommand for CreateTaskFolderCommand {
    type Response = TaskFolder;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        let val = ctx
            .db
            .insert::<TaskFolder>()
            .set(TaskFolder::WORKSPACE_ID, self.workspace_id)
            .set(TaskFolder::TITLE, self.title)
            .set_max_incremented_within(
                TaskFolder::SORT_ORDER,
                TaskFolder::WORKSPACE_ID,
                self.workspace_id,
            )
            .returning_one()
            .found()?;
        ctx.events
            .send(ctx.trace.event(DatabaseEventKind::TaskFolderCreated {
                folder_id: val.id,
                workspace_id: self.workspace_id,
            }));
        Ok(val.invalidating([Invalidation::TaskFolders(self.workspace_id)]))
    }
}

#[derive(FromRequest)]
#[request(DeleteTaskFolderRequest)]
pub struct DeleteTaskFolderCommand {
    pub workspace_id: i64,
    pub folder_id: i64,
}
impl DatabaseCommand for DeleteTaskFolderCommand {
    type Response = TaskFolder;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let result: TaskFolder = ctx
            .db
            .update()
            .set_true(TaskFolder::DELETED)
            .filter_eq(TaskFolder::WORKSPACE_ID, self.workspace_id)
            .filter_eq(TaskFolder::ID, self.folder_id)
            .returning_one()
            .found()?;
        let tasks: Vec<Task> = ctx
            .db
            .update()
            .set_true(Task::DELETED)
            .filter("task_folder_id = ? AND workspace_id = ?")
            .bind(self.folder_id)
            .bind(self.workspace_id)
            .returning_all()?;
        for task in &tasks {
            followed(ctx, task)?;
        }
        ctx.events
            .send(ctx.trace.event(DatabaseEventKind::TaskFolderDeleted {
                folder_id: self.folder_id,
                workspace_id: self.workspace_id,
            }));
        // The schema deletes the saved views that list the folder.
        Ok(result.invalidating([
            Invalidation::TaskFolders(self.workspace_id),
            Invalidation::SavedTaskViews(self.workspace_id),
            Invalidation::TaskList {
                workspace_id: self.workspace_id,
                folder_id: Some(self.folder_id),
            },
        ]))
    }
}

/// Changes a live task folder's title.
#[derive(FromRequest)]
#[request(RenameTaskFolderRequest)]
pub struct RenameTaskFolderCommand {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub title: String,
}
impl DatabaseCommand for RenameTaskFolderCommand {
    type Response = TaskFolder;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<TaskFolder>> {
        ensure_title(&self.title)?;
        ensure_workspace(ctx.db, self.workspace_id)?;
        let folder: TaskFolder = ctx
            .db
            .update_live()
            .set(TaskFolder::TITLE, self.title)
            .filter_eq(TaskFolder::WORKSPACE_ID, self.workspace_id)
            .filter_eq(TaskFolder::ID, self.folder_id)
            .returning_one()
            .found()?;
        Ok(folder.invalidating([Invalidation::TaskFolders(self.workspace_id)]))
    }
}

/// Pins a live task folder to the sidebar, or unpins it.
#[derive(FromRequest)]
#[request(SetTaskFolderPinnedRequest)]
pub struct SetTaskFolderPinnedCommand {
    pub workspace_id: i64,
    pub folder_id: i64,
    pub pinned: bool,
}
impl DatabaseCommand for SetTaskFolderPinnedCommand {
    type Response = TaskFolder;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<TaskFolder>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        let folder: TaskFolder = ctx
            .db
            .update_live()
            .set(TaskFolder::PINNED, self.pinned)
            .filter_eq(TaskFolder::WORKSPACE_ID, self.workspace_id)
            .filter_eq(TaskFolder::ID, self.folder_id)
            .returning_one()
            .found()?;
        Ok(folder.invalidating([Invalidation::TaskFolders(self.workspace_id)]))
    }
}

/// Gives each folder in `folder_ids` its position in the list as its sort order.
#[derive(FromRequest)]
#[request(ReorderTaskFoldersRequest)]
pub struct ReorderTaskFoldersCommand {
    pub workspace_id: i64,
    pub folder_ids: Vec<i64>,
}
impl DatabaseCommand for ReorderTaskFoldersCommand {
    type Response = Vec<TaskFolder>;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Vec<TaskFolder>>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        for (position, folder_id) in self.folder_ids.into_iter().enumerate() {
            ctx.db
                .update_live::<TaskFolder>()
                .set(TaskFolder::SORT_ORDER, position as i64)
                .filter_eq(TaskFolder::WORKSPACE_ID, self.workspace_id)
                .filter_eq(TaskFolder::ID, folder_id)
                .returning_one()
                .found()?;
        }
        Ok(ListTaskFoldersCommand {
            workspace_id: self.workspace_id,
            metadata: HashMap::new(),
            has_metadata: Vec::new(),
        }
        .handle(&mut ctx.as_read())?
        .invalidating([Invalidation::TaskFolders(self.workspace_id)]))
    }
}

#[cfg(test)]
mod tests;
