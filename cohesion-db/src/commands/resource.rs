use std::collections::BTreeMap;

use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{
    ConnectorAccount, ConnectorResource, Metadata, MetadataKey, MetadataOwner, Resource,
    ResourceKind, ResourceSort, WorkspaceResourceKind,
};
use cohesion_core::protocol::{
    DeleteResourceRequest, GetResourceRequest, ListResourcesRequest,
    ListWorkspaceResourceKindsRequest, ReorderWorkspaceResourceKindsRequest,
    SetResourceKindShownRequest,
};
use cohesion_core::{FromRequest, Projection};

use crate::changed::{Changed, Invalidates};
use crate::commands::workspace_scope::ensure_workspace;
use crate::{
    CommandContext, CommandError, DatabaseCommand, DatabaseCommandResult, DatabaseEventKind,
    Filtered, Found, Read, ReadableDatabase, ValidationError, Write,
};

/// A workspace's resources in `sort`'s order, optionally only one kind or
/// those whose title contains `search`.
#[derive(FromRequest)]
#[request(ListResourcesRequest)]
pub struct ListResourcesCommand {
    pub workspace_id: i64,
    pub kind: Option<String>,
    pub search: Option<String>,
    pub sort: ResourceSort,
    pub descending: bool,
}
impl DatabaseCommand for ListResourcesCommand {
    type Response = Vec<Resource>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Vec<Resource>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        let mut query = ctx
            .db
            .select::<Resource>()
            .filter_eq(Resource::WORKSPACE_ID, self.workspace_id);
        if let Some(kind) = self.kind {
            query = query
                .filter(&format!("{} = ?", Resource::KIND_SQL))
                .bind(kind);
        }
        if let Some(search) = self.search {
            query = query
                .filter("resources.title LIKE ?")
                .bind(format!("%{search}%"));
        }
        let direction = if self.descending { "DESC" } else { "ASC" };
        let order = match self.sort {
            ResourceSort::Title => format!("resources.title COLLATE NOCASE {direction}"),
            ResourceSort::LastUpdated => format!("resources.last_updated {direction}"),
            // `IS NULL` sorts false first, so resources without the key trail
            // either way.
            ResourceSort::Metadata(key) => {
                let value = format!(
                    "(SELECT value FROM metadata WHERE resource_id = resources.id AND key = '{}')",
                    key.as_str()
                );
                format!("{value} IS NULL, {value} COLLATE NOCASE {direction}")
            }
        };
        Ok(query.order(&format!("{order}, resources.id DESC")).all()?)
    }
}

/// The resources imported from a connector account, in one workspace or
/// all.
pub struct ListAccountResourcesCommand {
    pub account_id: i64,
    pub workspace_id: Option<i64>,
}
impl DatabaseCommand for ListAccountResourcesCommand {
    type Response = Vec<Resource>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Vec<Resource>> {
        let mut query = ctx
            .db
            .select::<Resource>()
            .filter_eq(Resource::ACCOUNT_ID, self.account_id);
        if let Some(workspace_id) = self.workspace_id {
            query = query.filter_eq(Resource::WORKSPACE_ID, workspace_id);
        }
        Ok(query.order("resources.id").all()?)
    }
}

#[derive(FromRequest)]
#[request(GetResourceRequest)]
pub struct GetResourceCommand {
    pub workspace_id: i64,
    pub resource_id: i64,
}
impl DatabaseCommand for GetResourceCommand {
    type Response = Resource;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Resource> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        ctx.db
            .select()
            .filter_eq(Resource::WORKSPACE_ID, self.workspace_id)
            .filter_eq(Resource::ID, self.resource_id)
            .one()
            .found()
    }
}

/// One metadata pair of a resource.
#[derive(Projection)]
#[projection(of = Metadata)]
struct MetadataPair {
    key: String,
    value: String,
}

/// Resource `resource_id`'s metadata.
fn stored_metadata<'c>(
    db: impl ReadableDatabase<'c>,
    resource_id: i64,
) -> DatabaseCommandResult<BTreeMap<MetadataKey, String>> {
    Ok(db
        .select_as::<MetadataPair>()
        .filter("resource_id = ?")
        .bind(resource_id)
        .all()?
        .into_iter()
        .map(|pair| {
            // Only `replace_metadata` writes a resource's pairs.
            let key: MetadataKey = pair
                .key
                .parse()
                .expect("a resource's keys are metadata keys");
            (key, pair.value)
        })
        .collect())
}

/// Makes resource `resource_id`'s metadata exactly `metadata`.
fn replace_metadata(
    ctx: &mut CommandContext<'_>,
    resource_id: i64,
    metadata: BTreeMap<MetadataKey, String>,
) -> DatabaseCommandResult<()> {
    let owner = MetadataOwner::Resource { resource_id };
    ctx.db.delete::<Metadata>().filter_group(owner).execute()?;
    for (key, value) in metadata {
        ctx.db
            .insert::<Metadata>()
            .set_group(owner)
            .set(Metadata::KEY, key.as_str())
            .set(Metadata::VALUE, value)
            .execute()?;
    }
    Ok(())
}

/// The id of kind `identifier` of account `account_id`'s connector, refusing
/// a kind the connector doesn't declare.
fn declared_kind<'c>(
    db: impl ReadableDatabase<'c>,
    account_id: i64,
    identifier: &str,
) -> DatabaseCommandResult<i64> {
    let connector = db
        .select_column::<ConnectorAccount, i64>("connector_id")
        .filter_eq(ConnectorAccount::ID, account_id);
    db.select_column::<ResourceKind, i64>("id")
        .filter_in("connector_id", connector)
        .filter_eq(ResourceKind::IDENTIFIER, identifier)
        .one()?
        .ok_or_else(|| {
            CommandError::Validation(ValidationError::in_field(
                "kindIdentifier",
                format!("The connector doesn't declare the kind \"{identifier}\"."),
            ))
        })
}

/// Imports `resource` into a workspace, or overwrites the copy already
/// there from the same account, metadata and all.
pub struct ImportResourceCommand {
    pub workspace_id: i64,
    pub account_id: i64,
    pub resource: ConnectorResource,
}
impl DatabaseCommand for ImportResourceCommand {
    type Response = Resource;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Resource>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        let fetched = self.resource;
        let kind_id = declared_kind(ctx.db, self.account_id, &fetched.kind_identifier)?;
        let resource: Resource = ctx
            .db
            .insert()
            .set(Resource::WORKSPACE_ID, self.workspace_id)
            .set(Resource::ACCOUNT_ID, self.account_id)
            .set(Resource::IDENTIFIER, fetched.identifier)
            .set(Resource::KIND_ID, kind_id)
            .set(Resource::TITLE, fetched.title)
            .set(Resource::CONTENT_KIND, fetched.content_kind)
            .set(Resource::CONTENTS, fetched.contents)
            .set(Resource::LAST_UPDATED, fetched.last_updated)
            .set_now(Resource::MODIFIED)
            .on_conflict_update(&["workspace_id", "account_id", "identifier"])
            .returning_one()
            .found()?;
        replace_metadata(ctx, resource.id, fetched.metadata)?;
        ctx.events
            .send(ctx.trace.event(DatabaseEventKind::ResourceImported {
                resource_id: resource.id,
                workspace_id: self.workspace_id,
            }));
        Ok(resource.invalidating([Invalidation::Resources(self.workspace_id)]))
    }
}

/// Rewrites a resource from a fresh copy of its item; `None`, writing
/// nothing, when the copy matches what is stored.
pub struct UpdateResourceCommand {
    pub workspace_id: i64,
    pub resource_id: i64,
    pub resource: ConnectorResource,
}
impl DatabaseCommand for UpdateResourceCommand {
    type Response = Option<Resource>;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Option<Resource>>> {
        let current = GetResourceCommand {
            workspace_id: self.workspace_id,
            resource_id: self.resource_id,
        }
        .handle(&mut ctx.as_read())?;
        let fresh = self.resource;
        let kind_id = declared_kind(ctx.db, current.account_id, &fresh.kind_identifier)?;
        if current.kind_id == kind_id
            && current.title == fresh.title
            && current.contents == fresh.contents
            && current.content_kind == fresh.content_kind
            && current.last_updated == fresh.last_updated
            && stored_metadata(ctx.db, self.resource_id)? == fresh.metadata
        {
            return Ok(None.invalidating_nothing());
        }
        let resource: Resource = ctx
            .db
            .update()
            .set(Resource::KIND_ID, kind_id)
            .set(Resource::TITLE, fresh.title)
            .set(Resource::CONTENTS, fresh.contents)
            .set(Resource::CONTENT_KIND, fresh.content_kind)
            .set(Resource::LAST_UPDATED, fresh.last_updated)
            .set_now(Resource::MODIFIED)
            .filter_eq(Resource::ID, self.resource_id)
            .returning_one()
            .found()?;
        replace_metadata(ctx, resource.id, fresh.metadata)?;
        ctx.events
            .send(ctx.trace.event(DatabaseEventKind::ResourceUpdated {
                resource_id: resource.id,
                workspace_id: self.workspace_id,
            }));
        Ok(Some(resource).invalidating([Invalidation::Resources(self.workspace_id)]))
    }
}

/// Removes a resource from a workspace. Importing its item again brings it
/// back.
#[derive(FromRequest)]
#[request(DeleteResourceRequest)]
pub struct DeleteResourceCommand {
    pub workspace_id: i64,
    pub resource_id: i64,
}
impl DatabaseCommand for DeleteResourceCommand {
    type Response = Resource;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Resource>> {
        let resource = GetResourceCommand {
            workspace_id: self.workspace_id,
            resource_id: self.resource_id,
        }
        .handle(&mut ctx.as_read())?;
        ctx.db
            .delete::<Resource>()
            .filter_eq(Resource::ID, self.resource_id)
            .execute()?;
        ctx.events
            .send(ctx.trace.event(DatabaseEventKind::ResourceDeleted {
                resource: resource.clone(),
            }));
        Ok(resource.invalidating([Invalidation::Resources(self.workspace_id)]))
    }
}

/// The kinds of resource shown in a workspace, in their sort order.
#[derive(FromRequest)]
#[request(ListWorkspaceResourceKindsRequest)]
pub struct ListWorkspaceResourceKindsCommand {
    pub workspace_id: i64,
}
impl DatabaseCommand for ListWorkspaceResourceKindsCommand {
    type Response = Vec<WorkspaceResourceKind>;
    type Access = Read;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Read>,
    ) -> DatabaseCommandResult<Vec<WorkspaceResourceKind>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        Ok(ctx
            .db
            .select()
            .filter("workspace_id = ?")
            .bind(self.workspace_id)
            .order("sort_order, id")
            .all()?)
    }
}

/// Shows a kind of resource in a workspace, or hides it there; its
/// resources stay either way. Returns the kinds now shown there.
#[derive(FromRequest)]
#[request(SetResourceKindShownRequest)]
pub struct SetResourceKindShownCommand {
    pub workspace_id: i64,
    pub kind_id: i64,
    pub shown: bool,
}
impl DatabaseCommand for SetResourceKindShownCommand {
    type Response = Vec<WorkspaceResourceKind>;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Vec<WorkspaceResourceKind>>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        let changed = if self.shown {
            // Showing a shown kind again leaves it where it is.
            let shown = ctx
                .db
                .select::<WorkspaceResourceKind>()
                .filter("workspace_id = ? AND kind_id = ?")
                .bind(self.workspace_id)
                .bind(self.kind_id)
                .exists()?;
            if shown {
                0
            } else {
                ctx.db
                    .insert::<WorkspaceResourceKind>()
                    .set(WorkspaceResourceKind::WORKSPACE_ID, self.workspace_id)
                    .set(WorkspaceResourceKind::KIND_ID, self.kind_id)
                    .set_max_incremented_within(
                        WorkspaceResourceKind::SORT_ORDER,
                        WorkspaceResourceKind::WORKSPACE_ID,
                        self.workspace_id,
                    )
                    .execute()?
            }
        } else {
            ctx.db
                .delete::<WorkspaceResourceKind>()
                .filter_eq(WorkspaceResourceKind::WORKSPACE_ID, self.workspace_id)
                .filter_eq(WorkspaceResourceKind::KIND_ID, self.kind_id)
                .execute()?
        };
        Ok(ListWorkspaceResourceKindsCommand {
            workspace_id: self.workspace_id,
        }
        .handle(&mut ctx.as_read())?
        .invalidating((changed > 0).then_some(Invalidation::ResourceKinds(self.workspace_id))))
    }
}

/// Gives each kind in `kind_ids` its position in the list as its sort order
/// in the workspace.
#[derive(FromRequest)]
#[request(ReorderWorkspaceResourceKindsRequest)]
pub struct ReorderWorkspaceResourceKindsCommand {
    pub workspace_id: i64,
    pub kind_ids: Vec<i64>,
}
impl DatabaseCommand for ReorderWorkspaceResourceKindsCommand {
    type Response = Vec<WorkspaceResourceKind>;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Vec<WorkspaceResourceKind>>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        for (position, kind_id) in self.kind_ids.into_iter().enumerate() {
            ctx.db
                .update::<WorkspaceResourceKind>()
                .set(WorkspaceResourceKind::SORT_ORDER, position as i64)
                .filter_eq(WorkspaceResourceKind::WORKSPACE_ID, self.workspace_id)
                .filter_eq(WorkspaceResourceKind::KIND_ID, kind_id)
                .returning_one()
                .found()?;
        }
        Ok(ListWorkspaceResourceKindsCommand {
            workspace_id: self.workspace_id,
        }
        .handle(&mut ctx.as_read())?
        .invalidating([Invalidation::ResourceKinds(self.workspace_id)]))
    }
}

#[cfg(test)]
mod tests;
