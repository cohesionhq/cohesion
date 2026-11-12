use std::collections::HashMap;

use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::SearchResult;
use cohesion_core::protocol::{
    CreateWorkspaceRequest, DeleteWorkspaceRequest, ListWorkspacesRequest, RenameWorkspaceRequest,
    ReorderWorkspacesRequest, SearchWorkspaceRequest, SetWorkspaceEmojiRequest,
};
use cohesion_core::value::IntoValue;
use cohesion_core::{FromRequest, Workspace};

use super::emoji::emoji;
use crate::changed::{Changed, Invalidates};
use crate::command_error::searched;
use crate::commands::title::ensure_title;
use crate::commands::workspace_scope::ensure_workspace;
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, DatabaseEventKind, Filtered, Found,
    Read, ReadableDatabase, Write,
};

/// Every workspace that has not been deleted and carries every one of the
/// `metadata` pairs and `has_metadata` keys, in their order.
#[derive(FromRequest, Default)]
#[request(ListWorkspacesRequest)]
pub struct ListWorkspacesCommand {
    pub metadata: HashMap<String, String>,
    pub has_metadata: Vec<String>,
}
impl DatabaseCommand for ListWorkspacesCommand {
    type Response = Vec<Workspace>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Vec<Workspace>> {
        Ok(ctx
            .db
            .select_live()
            .with_metadata(self.metadata)
            .with_metadata_keys(self.has_metadata)
            .order("sort_order, id")
            .all()?)
    }
}

/// Creates a workspace with a non-empty title and an optional emoji.
#[derive(FromRequest)]
#[request(CreateWorkspaceRequest)]
pub struct CreateWorkspaceCommand {
    pub title: String,
    pub emoji: Option<String>,
}
impl DatabaseCommand for CreateWorkspaceCommand {
    type Response = Workspace;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ensure_title(&self.title)?;
        let workspace: Workspace = ctx
            .db
            .insert()
            .set(Workspace::TITLE, self.title)
            .set(Workspace::EMOJI, emoji(self.emoji)?)
            .set_max_incremented(Workspace::SORT_ORDER)
            .returning_one()
            .found()?;
        ctx.events
            .send(ctx.trace.event(DatabaseEventKind::WorkspaceCreated {
                workspace_id: workspace.id,
            }));
        let key = Invalidation::Workspace(workspace.id);
        Ok(workspace.invalidating([key]))
    }
}

/// Soft-deletes a workspace.
#[derive(FromRequest)]
#[request(DeleteWorkspaceRequest)]
pub struct DeleteWorkspaceCommand {
    pub id: i64,
}
impl DatabaseCommand for DeleteWorkspaceCommand {
    type Response = Workspace;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let workspace: Workspace = ctx
            .db
            .update()
            .set_true(Workspace::DELETED)
            .filter_eq(Workspace::ID, self.id)
            .returning_one()
            .found()?;
        ctx.events
            .send(ctx.trace.event(DatabaseEventKind::WorkspaceDeleted {
                workspace_id: workspace.id,
            }));
        let key = Invalidation::Workspace(workspace.id);
        Ok(workspace.invalidating([key]))
    }
}

/// Changes a workspace's title.
#[derive(FromRequest)]
#[request(RenameWorkspaceRequest)]
pub struct RenameWorkspaceCommand {
    pub id: i64,
    pub title: String,
}
impl DatabaseCommand for RenameWorkspaceCommand {
    type Response = Workspace;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ensure_title(&self.title)?;
        let workspace: Workspace = ctx
            .db
            .update()
            .set(Workspace::TITLE, self.title)
            .filter_eq(Workspace::ID, self.id)
            .returning_one()
            .found()?;
        let key = Invalidation::Workspace(workspace.id);
        Ok(workspace.invalidating([key]))
    }
}

/// Sets or clears a workspace's emoji.
#[derive(FromRequest)]
#[request(SetWorkspaceEmojiRequest)]
pub struct SetWorkspaceEmojiCommand {
    pub id: i64,
    pub emoji: Option<String>,
}
impl DatabaseCommand for SetWorkspaceEmojiCommand {
    type Response = Workspace;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let workspace: Workspace = ctx
            .db
            .update()
            .set(Workspace::EMOJI, emoji(self.emoji)?)
            .filter_eq(Workspace::ID, self.id)
            .returning_one()
            .found()?;
        let key = Invalidation::Workspace(workspace.id);
        Ok(workspace.invalidating([key]))
    }
}

/// Gives each workspace in `ids` its position in the list as its sort order.
#[derive(FromRequest)]
#[request(ReorderWorkspacesRequest)]
pub struct ReorderWorkspacesCommand {
    pub ids: Vec<i64>,
}
impl DatabaseCommand for ReorderWorkspacesCommand {
    type Response = Vec<Workspace>;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        for (position, id) in self.ids.iter().enumerate() {
            ctx.db
                .update_live::<Workspace>()
                .set(Workspace::SORT_ORDER, position as i64)
                .filter_eq(Workspace::ID, *id)
                .returning_one()
                .found()?;
        }
        let workspaces = ListWorkspacesCommand::default().handle(&mut ctx.as_read())?;
        let keys = self.ids.into_iter().map(Invalidation::Workspace);
        Ok(workspaces.invalidating(keys))
    }
}

/// Full-text matches in a workspace, best first: every match in a title
/// before any match only in contents, the titles the query covers most (an
/// exact title) first, then by title and contents together, the title
/// weighing 5 to the contents' 1.
#[derive(FromRequest)]
#[request(SearchWorkspaceRequest)]
pub struct SearchWorkspaceCommand {
    pub workspace_id: i64,
    pub query: String,
}
impl DatabaseCommand for SearchWorkspaceCommand {
    type Response = Vec<SearchResult>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        let query = "SELECT entry.id,
            idx.title,
            idx.contents,
            entry.entity_kind,
            entry.entity_id,
            COALESCE(entry.workspace_id, ?2) AS workspace_id,
            CASE WHEN entry.entity_kind = 'task'
                THEN COALESCE(task.status, 'none') ELSE 'none' END AS status
        FROM search_fts AS idx
        INNER JOIN search_entries AS entry ON entry.id = idx.rowid
        LEFT JOIN tasks AS task
            ON entry.entity_kind = 'task' AND task.id = entry.entity_id
        WHERE idx.search_fts MATCH ?1
            -- A feed belongs to no workspace, so every workspace finds it.
            AND (entry.workspace_id = ?2
                OR (entry.workspace_id IS NULL AND entry.entity_kind = 'feed'))
        -- The title alone scores 0 when only the contents matched, and
        -- best for the shortest title holding the query.
        ORDER BY bm25(search_fts, 1.0, 0.0), bm25(search_fts, 5.0, 1.0)
        ";
        searched(ctx.db.container().query_all(
            query,
            &[self.query.into_value(), self.workspace_id.into_value()],
        ))
    }
}

/// A live workspace by id.
pub struct GetWorkspaceCommand {
    pub id: i64,
}
impl DatabaseCommand for GetWorkspaceCommand {
    type Response = Workspace;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Workspace> {
        ctx.db
            .select_live()
            .filter("id = ?")
            .bind(self.id)
            .one()
            .found()
    }
}

/// The live workspace with exactly this title.
pub struct LookupWorkspaceByTitleCommand {
    pub title: String,
}
impl DatabaseCommand for LookupWorkspaceByTitleCommand {
    type Response = Workspace;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Workspace> {
        ctx.db
            .select_live()
            .filter("title = ?")
            .bind(self.title)
            .one()
            .found()
    }
}

/// A workspace by id, including one that has been soft-deleted.
pub struct GetWorkspaceIncludingDeletedCommand {
    pub id: i64,
}
impl DatabaseCommand for GetWorkspaceIncludingDeletedCommand {
    type Response = Workspace;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Workspace> {
        ctx.db.select().filter("id = ?").bind(self.id).one().found()
    }
}

#[cfg(test)]
mod tests;
