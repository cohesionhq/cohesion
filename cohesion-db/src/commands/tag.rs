use cohesion_core::FromRequest;
use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{Tag, Task, TaskTag};
use cohesion_core::protocol::{
    CreateTagRequest, DeleteTagRequest, ListTagsRequest, RenameTagRequest, ReorderTagsRequest,
    SetTaggedRequest,
};

use crate::changed::{Changed, Invalidates};
use crate::commands::task::{GetTaskCommand, task_list};
use crate::commands::workspace_scope::ensure_workspace;
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, DatabaseError, DatabaseEventKind,
    Filtered, Found, Read, ReadableDatabase, Select, ValidationError, Write,
};

/// The ids of the tasks tag `tag_id` is on.
pub(crate) fn tasks_tagged<'c>(
    db: impl ReadableDatabase<'c>,
    tag_id: i64,
) -> Select<'c, TaskTag, i64> {
    db.select_column("task_id")
        .filter("tag_id = ?")
        .bind(tag_id)
}

/// Tag `tag_id` of workspace `workspace_id`.
fn tag_in<'c>(
    db: impl ReadableDatabase<'c>,
    workspace_id: i64,
    tag_id: i64,
) -> DatabaseCommandResult<Tag> {
    db.select()
        .filter_eq(Tag::WORKSPACE_ID, workspace_id)
        .filter_eq(Tag::ID, tag_id)
        .one()
        .found()
}

/// Rejects an empty name.
fn ensure_name(name: &str) -> Result<(), ValidationError> {
    if name.is_empty() {
        return Err(ValidationError::in_field(
            "name",
            "Name can’t be empty.".to_string(),
        ));
    }
    Ok(())
}

/// A tag written, or a refusal naming the name already taken.
fn named(written: Result<Option<Tag>, DatabaseError>, name: &str) -> DatabaseCommandResult<Tag> {
    match written {
        Err(error) if error.is_unique_violation() => Err(ValidationError::in_field(
            "name",
            format!("A tag named “{name}” already exists."),
        )
        .into()),
        written => written.found(),
    }
}

/// The list showing task `task_id`, if the task exists.
fn list_of(
    ctx: &mut CommandContext<'_>,
    task_id: i64,
) -> DatabaseCommandResult<Option<Invalidation>> {
    Ok(ctx
        .db
        .select::<Task>()
        .filter_eq(Task::ID, task_id)
        .one()?
        .map(|task| task_list(&task)))
}

/// A workspace's tags, in their order.
#[derive(FromRequest)]
#[request(ListTagsRequest)]
pub struct ListTagsCommand {
    pub workspace_id: i64,
}
impl DatabaseCommand for ListTagsCommand {
    type Response = Vec<Tag>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Vec<Tag>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        Ok(ctx
            .db
            .select()
            .filter_eq(Tag::WORKSPACE_ID, self.workspace_id)
            .order("sort_order, id")
            .all()?)
    }
}

/// Creates a tag in a workspace.
#[derive(FromRequest)]
#[request(CreateTagRequest)]
pub struct CreateTagCommand {
    pub workspace_id: i64,
    pub name: String,
}
impl DatabaseCommand for CreateTagCommand {
    type Response = Tag;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Tag>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        ensure_name(&self.name)?;
        let written = ctx
            .db
            .insert::<Tag>()
            .set(Tag::WORKSPACE_ID, self.workspace_id)
            .set(Tag::NAME, self.name.clone())
            .set_max_incremented_within(Tag::SORT_ORDER, Tag::WORKSPACE_ID, self.workspace_id)
            .returning_one();
        let tag = named(written, &self.name)?;
        Ok(tag.invalidating([Invalidation::Tags(self.workspace_id)]))
    }
}

/// Renames a tag of a workspace.
#[derive(FromRequest)]
#[request(RenameTagRequest)]
pub struct RenameTagCommand {
    pub workspace_id: i64,
    pub tag_id: i64,
    pub name: String,
}
impl DatabaseCommand for RenameTagCommand {
    type Response = Tag;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Tag>> {
        ensure_name(&self.name)?;
        let written = ctx
            .db
            .update::<Tag>()
            .set(Tag::NAME, self.name.clone())
            .filter_eq(Tag::WORKSPACE_ID, self.workspace_id)
            .filter_eq(Tag::ID, self.tag_id)
            .returning_one();
        let tag = named(written, &self.name)?;
        Ok(tag.invalidating([Invalidation::Tags(self.workspace_id)]))
    }
}

/// Gives each tag in `tag_ids` its position in the list as its sort order.
#[derive(FromRequest)]
#[request(ReorderTagsRequest)]
pub struct ReorderTagsCommand {
    pub workspace_id: i64,
    pub tag_ids: Vec<i64>,
}
impl DatabaseCommand for ReorderTagsCommand {
    type Response = Vec<Tag>;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Vec<Tag>>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        for (position, tag_id) in self.tag_ids.into_iter().enumerate() {
            ctx.db
                .update::<Tag>()
                .set(Tag::SORT_ORDER, position as i64)
                .filter_eq(Tag::WORKSPACE_ID, self.workspace_id)
                .filter_eq(Tag::ID, tag_id)
                .returning_one()
                .found()?;
        }
        let tags = ListTagsCommand {
            workspace_id: self.workspace_id,
        }
        .handle(&mut ctx.as_read())?;
        Ok(tags.invalidating([Invalidation::Tags(self.workspace_id)]))
    }
}

/// Deletes a tag of a workspace; the schema takes it off every task, and out
/// of every saved task view.
#[derive(FromRequest)]
#[request(DeleteTagRequest)]
pub struct DeleteTagCommand {
    pub workspace_id: i64,
    pub tag_id: i64,
}
impl DatabaseCommand for DeleteTagCommand {
    type Response = Tag;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Tag>> {
        let tag = tag_in(ctx.db.read(), self.workspace_id, self.tag_id)?;
        let tasks: Vec<i64> = tasks_tagged(ctx.db.read(), tag.id).all()?;
        let mut lists = vec![
            Invalidation::Tags(self.workspace_id),
            Invalidation::SavedTaskViews(self.workspace_id),
        ];
        for task in tasks {
            lists.extend(list_of(ctx, task)?);
        }
        ctx.db
            .delete::<Tag>()
            .filter_eq(Tag::ID, tag.id)
            .execute()?;
        Ok(tag.invalidating(lists))
    }
}

/// Puts a tag on a task of its workspace, or takes it off.
#[derive(FromRequest)]
#[request(SetTaggedRequest)]
pub struct SetTaggedCommand {
    pub workspace_id: i64,
    pub tag_id: i64,
    pub task_id: i64,
    pub tagged: bool,
}
impl DatabaseCommand for SetTaggedCommand {
    type Response = Task;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Task>> {
        let tag = tag_in(ctx.db.read(), self.workspace_id, self.tag_id)?;
        let on = tasks_tagged(ctx.db.read(), tag.id)
            .filter("task_id = ?")
            .bind(self.task_id)
            .exists()?;
        match (self.tagged, on) {
            (true, false) => {
                ctx.db
                    .insert::<TaskTag>()
                    .set(TaskTag::WORKSPACE_ID, tag.workspace_id)
                    .set(TaskTag::TAG_ID, tag.id)
                    .set(TaskTag::TASK_ID, self.task_id)
                    .execute()?;
            }
            (false, true) => {
                ctx.db
                    .delete::<TaskTag>()
                    .filter_eq(TaskTag::TAG_ID, tag.id)
                    .filter_eq(TaskTag::TASK_ID, self.task_id)
                    .execute()?;
            }
            _ => {}
        }
        let task = GetTaskCommand {
            workspace_id: self.workspace_id,
            task_id: self.task_id,
        }
        .handle(&mut ctx.as_read())?;
        if self.tagged == on {
            return Ok(task.invalidating_nothing());
        }
        ctx.events
            .send(ctx.trace.event(DatabaseEventKind::TaskTagged {
                task_id: self.task_id,
                workspace_id: self.workspace_id,
            }));
        let list = task_list(&task);
        Ok(task.invalidating([list]))
    }
}

#[cfg(test)]
mod tests;
