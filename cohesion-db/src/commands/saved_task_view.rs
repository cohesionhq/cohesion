use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{
    SavedTaskView, SavedTaskViewPriority, SavedTaskViewSize, SavedTaskViewTag, TagFilter, TagMatch,
    TaskViewScope,
};
use cohesion_core::protocol::{
    CreateSavedTaskViewRequest, DeleteSavedTaskViewRequest, ListSavedTaskViewsRequest,
    RenameSavedTaskViewRequest, ReorderSavedTaskViewsRequest, UpdateSavedTaskViewRequest,
};
use cohesion_core::{FromRequest, TaskPriority, TaskSize, TaskSort};

use crate::changed::{Changed, Invalidates};
use crate::commands::workspace_scope::ensure_workspace;
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, DatabaseError, Filtered, Found, Read,
    ReadableDatabase, ValidationError, Write,
};

/// Saved task view `view_id` of workspace `workspace_id`.
pub struct GetSavedTaskViewCommand {
    pub workspace_id: i64,
    pub view_id: i64,
}
impl DatabaseCommand for GetSavedTaskViewCommand {
    type Response = SavedTaskView;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<SavedTaskView> {
        ctx.db
            .select()
            .filter_eq(SavedTaskView::WORKSPACE_ID, self.workspace_id)
            .filter_eq(SavedTaskView::ID, self.view_id)
            .one()
            .found()
    }
}

/// Every saved task view of a workspace, in its order.
#[derive(FromRequest)]
#[request(ListSavedTaskViewsRequest)]
pub struct ListSavedTaskViewsCommand {
    pub workspace_id: i64,
}
impl DatabaseCommand for ListSavedTaskViewsCommand {
    type Response = Vec<SavedTaskView>;
    type Access = Read;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Read>,
    ) -> DatabaseCommandResult<Vec<SavedTaskView>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        Ok(ctx
            .db
            .select()
            .filter_eq(SavedTaskView::WORKSPACE_ID, self.workspace_id)
            .order("sort_order, id")
            .all()?)
    }
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

/// A view written, or a refusal naming the name already taken.
fn named(
    written: Result<Option<SavedTaskView>, DatabaseError>,
    name: &str,
) -> DatabaseCommandResult<SavedTaskView> {
    match written {
        Err(error) if error.is_unique_violation() => Err(ValidationError::in_field(
            "name",
            format!("A view named “{name}” already exists."),
        )
        .into()),
        written => written.found(),
    }
}

/// Gives view `view_id` exactly the tags of `tags`, and replies with the view.
/// Makes the priorities and sizes view `view_id` narrows by exactly these.
fn set_priorities_and_sizes(
    ctx: &mut CommandContext<'_>,
    view_id: i64,
    priorities: Vec<TaskPriority>,
    sizes: Vec<TaskSize>,
) -> DatabaseCommandResult<()> {
    ctx.db
        .delete::<SavedTaskViewPriority>()
        .filter_eq(SavedTaskViewPriority::VIEW_ID, view_id)
        .execute()?;
    ctx.db
        .delete::<SavedTaskViewSize>()
        .filter_eq(SavedTaskViewSize::VIEW_ID, view_id)
        .execute()?;
    for priority in priorities {
        ctx.db
            .insert::<SavedTaskViewPriority>()
            .set(SavedTaskViewPriority::VIEW_ID, view_id)
            .set(SavedTaskViewPriority::PRIORITY, priority)
            .execute()?;
    }
    for size in sizes {
        ctx.db
            .insert::<SavedTaskViewSize>()
            .set(SavedTaskViewSize::VIEW_ID, view_id)
            .set(SavedTaskViewSize::SIZE, size)
            .execute()?;
    }
    Ok(())
}

fn set_tags(
    ctx: &mut CommandContext<'_>,
    workspace_id: i64,
    view_id: i64,
    tags: TagFilter,
) -> DatabaseCommandResult<SavedTaskView> {
    ctx.db
        .delete::<SavedTaskViewTag>()
        .filter_eq(SavedTaskViewTag::VIEW_ID, view_id)
        .execute()?;
    // The table's foreign keys refuse a tag that is not one of the workspace's
    // task tags.
    let wanted = tags.with.into_iter().map(|id| (id, TagMatch::With));
    let unwanted = tags.without.into_iter().map(|id| (id, TagMatch::Without));
    for (tag_id, tag_match) in wanted.chain(unwanted) {
        ctx.db
            .insert::<SavedTaskViewTag>()
            .set(SavedTaskViewTag::WORKSPACE_ID, workspace_id)
            .set(SavedTaskViewTag::VIEW_ID, view_id)
            .set(SavedTaskViewTag::TAG_ID, tag_id)
            .set(SavedTaskViewTag::TAG_MATCH, tag_match)
            .execute()?;
    }
    GetSavedTaskViewCommand {
        workspace_id,
        view_id,
    }
    .handle(&mut ctx.as_read())
}

#[derive(FromRequest)]
#[request(CreateSavedTaskViewRequest)]
pub struct CreateSavedTaskViewCommand {
    pub workspace_id: i64,
    pub name: String,
    pub scope: TaskViewScope,
    pub tags: TagFilter,
    pub priorities: Vec<TaskPriority>,
    pub sizes: Vec<TaskSize>,
    pub sort: TaskSort,
    pub descending: bool,
}
impl DatabaseCommand for CreateSavedTaskViewCommand {
    type Response = SavedTaskView;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<SavedTaskView>> {
        ensure_workspace(ctx.db.read(), self.workspace_id)?;
        ensure_name(&self.name)?;
        let written = ctx
            .db
            .insert()
            .set(SavedTaskView::WORKSPACE_ID, self.workspace_id)
            .set(SavedTaskView::NAME, self.name.clone())
            .set_group(self.scope)
            .set(SavedTaskView::SORT, self.sort)
            .set(SavedTaskView::DESCENDING, self.descending)
            .set_max_incremented_within(
                SavedTaskView::SORT_ORDER,
                SavedTaskView::WORKSPACE_ID,
                self.workspace_id,
            )
            .returning_one();
        let view = named(written, &self.name)?;
        set_priorities_and_sizes(ctx, view.id, self.priorities, self.sizes)?;
        Ok(set_tags(ctx, self.workspace_id, view.id, self.tags)?
            .invalidating([Invalidation::SavedTaskViews(self.workspace_id)]))
    }
}

#[derive(FromRequest)]
#[request(UpdateSavedTaskViewRequest)]
pub struct UpdateSavedTaskViewCommand {
    pub workspace_id: i64,
    pub view_id: i64,
    pub scope: TaskViewScope,
    pub tags: TagFilter,
    pub priorities: Vec<TaskPriority>,
    pub sizes: Vec<TaskSize>,
    pub sort: TaskSort,
    pub descending: bool,
}
impl DatabaseCommand for UpdateSavedTaskViewCommand {
    type Response = SavedTaskView;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<SavedTaskView>> {
        let view: SavedTaskView = ctx
            .db
            .update()
            .set_group(self.scope)
            .set(SavedTaskView::SORT, self.sort)
            .set(SavedTaskView::DESCENDING, self.descending)
            .filter_eq(SavedTaskView::WORKSPACE_ID, self.workspace_id)
            .filter_eq(SavedTaskView::ID, self.view_id)
            .returning_one()
            .found()?;
        set_priorities_and_sizes(ctx, view.id, self.priorities, self.sizes)?;
        Ok(set_tags(ctx, self.workspace_id, view.id, self.tags)?
            .invalidating([Invalidation::SavedTaskViews(self.workspace_id)]))
    }
}

#[derive(FromRequest)]
#[request(RenameSavedTaskViewRequest)]
pub struct RenameSavedTaskViewCommand {
    pub workspace_id: i64,
    pub view_id: i64,
    pub name: String,
}
impl DatabaseCommand for RenameSavedTaskViewCommand {
    type Response = SavedTaskView;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<SavedTaskView>> {
        ensure_name(&self.name)?;
        let written = ctx
            .db
            .update()
            .set(SavedTaskView::NAME, self.name.clone())
            .filter_eq(SavedTaskView::WORKSPACE_ID, self.workspace_id)
            .filter_eq(SavedTaskView::ID, self.view_id)
            .returning_one();
        let view = named(written, &self.name)?;
        Ok(view.invalidating([Invalidation::SavedTaskViews(self.workspace_id)]))
    }
}

/// Gives each view in `view_ids` its position in the list as its sort order.
#[derive(FromRequest)]
#[request(ReorderSavedTaskViewsRequest)]
pub struct ReorderSavedTaskViewsCommand {
    pub workspace_id: i64,
    pub view_ids: Vec<i64>,
}
impl DatabaseCommand for ReorderSavedTaskViewsCommand {
    type Response = Vec<SavedTaskView>;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Vec<SavedTaskView>>> {
        ensure_workspace(ctx.db.read(), self.workspace_id)?;
        for (position, view_id) in self.view_ids.into_iter().enumerate() {
            let _: SavedTaskView = ctx
                .db
                .update()
                .set(SavedTaskView::SORT_ORDER, position as i64)
                .filter_eq(SavedTaskView::WORKSPACE_ID, self.workspace_id)
                .filter_eq(SavedTaskView::ID, view_id)
                .returning_one()
                .found()?;
        }
        Ok(ListSavedTaskViewsCommand {
            workspace_id: self.workspace_id,
        }
        .handle(&mut ctx.as_read())?
        .invalidating([Invalidation::SavedTaskViews(self.workspace_id)]))
    }
}

#[derive(FromRequest)]
#[request(DeleteSavedTaskViewRequest)]
pub struct DeleteSavedTaskViewCommand {
    pub workspace_id: i64,
    pub view_id: i64,
}
impl DatabaseCommand for DeleteSavedTaskViewCommand {
    type Response = SavedTaskView;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<SavedTaskView>> {
        let view = GetSavedTaskViewCommand {
            workspace_id: self.workspace_id,
            view_id: self.view_id,
        }
        .handle(&mut ctx.as_read())?;
        ctx.db
            .delete::<SavedTaskView>()
            .filter_eq(SavedTaskView::ID, view.id)
            .execute()?;
        Ok(view.invalidating([Invalidation::SavedTaskViews(self.workspace_id)]))
    }
}

#[cfg(test)]
mod tests;
