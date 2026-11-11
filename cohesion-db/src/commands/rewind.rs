//! Reading Rewind's kept versions and restoring from them. The triggers on
//! `tasks` and `documents` keep the versions; nothing here changes or removes
//! one but the cleanup's [`DeleteOldVersionsCommand`].

use cohesion_core::model::{
    DeletedItem, RestoredItem, RewindWriter, TaskFolder, Version, VersionChange, VersionSummary,
    VersionTarget, VersionedItem,
};
use cohesion_core::protocol::{
    GetVersionRequest, ListDeletedItemsRequest, ListVersionsRequest, RestoreVersionRequest,
};
use cohesion_core::{Document, FromRequest, Task, TaskStatus};

use crate::changed::{Changed, Invalidates};
use crate::commands::document::{changed, saved, table_of};
use crate::commands::task::{followed, task_list};
use crate::commands::workspace_scope::ensure_workspace;
use crate::retention::days_in_secs;
use crate::{
    CommandContext, CommandError, DatabaseCommand, DatabaseCommandResult, DatabaseEventKind,
    Filtered, Found, Read, ReadableDatabase, ValidationError, Write,
};

/// The `"deleted"` version that says when an item went: its latest, should
/// the item have been deleted and restored before.
const LATEST_DELETED: &str = "versions.id = (SELECT MAX(latest.id) FROM versions latest \
     WHERE (latest.task_id = versions.task_id OR latest.document_id = versions.document_id) \
     AND latest.change = ?)";

/// Marks what `write` changes as a restore, so its trigger keeps a version of
/// its own rather than merging into the edit it may overwrite. A failure
/// rolls the mark back with the command.
fn restoring<T>(
    ctx: &mut CommandContext<'_>,
    write: impl FnOnce(&mut CommandContext<'_>) -> DatabaseCommandResult<T>,
) -> DatabaseCommandResult<T> {
    ctx.db
        .update::<RewindWriter>()
        .set_true(RewindWriter::RESTORING)
        .execute()?;
    let written = write(ctx)?;
    ctx.db
        .update::<RewindWriter>()
        .set_false(RewindWriter::RESTORING)
        .execute()?;
    Ok(written)
}

/// A live or deleted task in a workspace by id; a hook or a script holding a
/// deleted task's id still reads it.
pub struct GetTaskIncludingDeletedCommand {
    pub workspace_id: i64,
    pub task_id: i64,
}
impl DatabaseCommand for GetTaskIncludingDeletedCommand {
    type Response = Task;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Task> {
        ctx.db
            .select()
            .filter_eq(Task::WORKSPACE_ID, self.workspace_id)
            .filter_eq(Task::ID, self.task_id)
            .one()
            .found()
    }
}

/// A live or deleted document in a workspace by id; a hook or a script holding
/// a deleted document's id still reads it.
pub struct GetDocumentIncludingDeletedCommand {
    pub workspace_id: i64,
    pub document_id: i64,
}
impl DatabaseCommand for GetDocumentIncludingDeletedCommand {
    type Response = Document;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Document> {
        ctx.db
            .select()
            .filter_eq(Document::WORKSPACE_ID, self.workspace_id)
            .filter_eq(Document::ID, self.document_id)
            .one()
            .found()
    }
}

/// An item's kept versions, newest first, without their text.
#[derive(FromRequest)]
#[request(ListVersionsRequest)]
pub struct ListVersionsCommand {
    pub workspace_id: i64,
    pub item: VersionTarget,
}
impl DatabaseCommand for ListVersionsCommand {
    type Response = Vec<VersionSummary>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        let (column, id) = match self.item {
            VersionTarget::Task { task_id } => ("versions.task_id = ?", task_id),
            VersionTarget::Document { document_id } => ("versions.document_id = ?", document_id),
        };
        Ok(ctx
            .db
            .select_as::<VersionSummary>()
            .filter("versions.workspace_id = ?")
            .bind(self.workspace_id)
            .filter(column)
            .bind(id)
            .order("versions.id DESC")
            .all()?)
    }
}

/// One kept version, by its id, within a workspace.
#[derive(FromRequest)]
#[request(GetVersionRequest)]
pub struct GetVersionCommand {
    pub workspace_id: i64,
    pub version_id: i64,
}
impl DatabaseCommand for GetVersionCommand {
    type Response = Version;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Version> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        ctx.db
            .select()
            .filter_eq(Version::ID, self.version_id)
            .filter_eq(Version::WORKSPACE_ID, self.workspace_id)
            .one()
            .found()
    }
}

/// Puts an item back to a version and brings it back if it was deleted,
/// keeping the result as a new version so this can be undone in turn.
#[derive(FromRequest)]
#[request(RestoreVersionRequest)]
pub struct RestoreVersionCommand {
    pub workspace_id: i64,
    pub version_id: i64,
}
impl DatabaseCommand for RestoreVersionCommand {
    type Response = RestoredItem;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<RestoredItem>> {
        let version = GetVersionCommand {
            workspace_id: self.workspace_id,
            version_id: self.version_id,
        }
        .handle(&mut ctx.as_read())?;
        match version.item.clone() {
            VersionedItem::Task { task_id, status } => {
                restore_task(ctx, self.workspace_id, task_id, status, version)
                    .map(|task| task.map(|task| RestoredItem::Task { task }))
            }
            VersionedItem::Document { document_id } => {
                restore_document(ctx, self.workspace_id, document_id, version)
                    .map(|document| document.map(|document| RestoredItem::Document { document }))
            }
        }
    }
}

/// The workspace's deleted items whose deletion Rewind still has, most
/// recently deleted first.
#[derive(FromRequest)]
#[request(ListDeletedItemsRequest)]
pub struct ListDeletedItemsCommand {
    pub workspace_id: i64,
}
impl DatabaseCommand for ListDeletedItemsCommand {
    type Response = Vec<DeletedItem>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        Ok(ctx
            .db
            .select_as::<DeletedItem>()
            .filter("versions.workspace_id = ? AND coalesce(tasks.deleted, documents.deleted) = 1")
            .bind(self.workspace_id)
            .filter(LATEST_DELETED)
            .bind(VersionChange::Deleted)
            .order("versions.modified DESC, versions.id DESC")
            .all()?)
    }
}

/// Puts a task's text and status back to a version's. Nothing else about the
/// task changes, except that a task coming back from a folder deleted since
/// goes to the inbox. Each change raises its own event, as editing would, so
/// hooks and extensions keep up; a task coming back raises `TaskCreated`.
fn restore_task(
    ctx: &mut CommandContext<'_>,
    workspace_id: i64,
    task_id: i64,
    status: TaskStatus,
    version: Version,
) -> DatabaseCommandResult<Changed<Task>> {
    let before = GetTaskIncludingDeletedCommand {
        workspace_id,
        task_id,
    }
    .handle(&mut ctx.as_read())?;
    let renamed = before.title != version.title;
    let described = before.description != version.body;
    let status_changed = before.status != status;
    if !(renamed || described || status_changed || before.deleted) {
        return Ok(before.invalidating_nothing());
    }

    let folder_id = match before.task_folder_id {
        Some(folder_id)
            if ctx
                .db
                .select_live::<TaskFolder>()
                .filter_eq(TaskFolder::ID, folder_id)
                .exists()? =>
        {
            Some(folder_id)
        }
        _ => None,
    };
    let task: Task = restoring(ctx, |ctx| {
        ctx.db
            .update()
            .set(Task::TITLE, version.title)
            .set(Task::DESCRIPTION, version.body)
            .set(Task::STATUS, status)
            .set_false(Task::DELETED)
            .set(Task::TASK_FOLDER_ID, folder_id)
            .filter_eq(Task::ID, task_id)
            .returning_one()
            .found()
    })?;

    let mut events = Vec::new();
    if before.deleted {
        events.push(DatabaseEventKind::TaskCreated {
            task_id,
            workspace_id,
        });
    }
    if renamed {
        events.push(DatabaseEventKind::TaskRenamed {
            task_id,
            workspace_id,
        });
    }
    if described {
        events.push(DatabaseEventKind::TaskDescriptionChanged {
            task_id,
            workspace_id,
        });
    }
    if status_changed {
        events.push(DatabaseEventKind::TaskStatusChanged {
            task_id,
            workspace_id,
        });
    }
    for event in events {
        ctx.events.send(ctx.trace.event(event));
    }
    followed(ctx, &task)?;
    let keys = [task_list(&before), task_list(&task)];
    Ok(task.invalidating(keys))
}

/// Puts a document's title and contents back to a version's. A change to its
/// text, or its coming back, raises the save event editing would.
fn restore_document(
    ctx: &mut CommandContext<'_>,
    workspace_id: i64,
    document_id: i64,
    version: Version,
) -> DatabaseCommandResult<Changed<Document>> {
    let before = GetDocumentIncludingDeletedCommand {
        workspace_id,
        document_id,
    }
    .handle(&mut ctx.as_read())?;
    let rewritten = before.contents != version.body;
    if !(rewritten || before.title != version.title || before.deleted) {
        return Ok(before.invalidating_nothing());
    }
    let document: Document = restoring(ctx, |ctx| {
        ctx.db
            .update()
            .set(Document::TITLE, version.title)
            .set(Document::CONTENTS, version.body)
            .set_false(Document::DELETED)
            .set_now(Document::MODIFIED)
            .filter_eq(Document::ID, document_id)
            .returning_one()
            .found()
            .map_err(day_taken)
    })?;
    let table = table_of(&document);
    let restored = if rewritten || before.deleted {
        saved(ctx, document)
    } else {
        changed(ctx, document)
    };
    Ok(restored?.and(table))
}

/// The only unique value a restore can collide with is a journal's day, the
/// schema's `documents_live_journal`; says so in the words of the page.
fn day_taken(error: CommandError) -> CommandError {
    match error {
        CommandError::Validation(_) => ValidationError::new(
            "This day has a new journal entry. Restore a version of that entry instead.",
        )
        .into(),
        other => other,
    }
}

/// Removes versions last changed more than `days` ago, in the hourly cleanup.
/// The one way a version goes: no request or script reaches this.
pub(crate) struct DeleteOldVersionsCommand {
    pub days: i64,
}
impl DatabaseCommand for DeleteOldVersionsCommand {
    type Response = ();
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<()>> {
        let cutoff = ctx.now.saturating_sub(days_in_secs(self.days));
        ctx.db
            .delete::<Version>()
            .filter("modified < ?")
            .bind(cutoff)
            .execute()?;
        Ok(().invalidating_nothing())
    }
}

/// Removes for good the tasks and documents deleted more than `days` ago, in
/// the hourly cleanup; `days` is never less than Rewind's retention, so only
/// what Rewind can no longer bring back goes. Foreign keys take what hangs off
/// them. The one way an item leaves the database.
pub(crate) struct PurgeDeletedItemsCommand {
    pub days: i64,
}
impl DatabaseCommand for PurgeDeletedItemsCommand {
    type Response = ();
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<()>> {
        let cutoff = ctx.now.saturating_sub(days_in_secs(self.days));
        ctx.db
            .delete::<Task>()
            .filter("deleted_at < ?")
            .bind(cutoff)
            .execute()?;
        ctx.db
            .delete::<Document>()
            .filter("deleted_at < ?")
            .bind(cutoff)
            .execute()?;
        Ok(().invalidating_nothing())
    }
}

#[cfg(test)]
mod tests;
