use cohesion_core::FromRequest;
use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{
    ConnectorDefinition, Extension, ExtensionHook, ExtensionManifest, ExtensionPermission,
    ExtensionPermissionKind, ExtensionService, ExtensionServiceFunction, HookKind,
    InstalledService, ServiceManifest,
};
use cohesion_core::protocol::{
    AcceptedResponse, ExtensionSummary, ListExtensionHooksRequest, ListExtensionPermissionsRequest,
    ListExtensionsRequest, SetExtensionHookRequest, SetExtensionPermissionRequest,
};

use crate::changed::{Changed, Invalidates};
use crate::commands::connector::upsert_connector;
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, Filtered, Found, Read,
    ReadableDatabase, Select, ValidationError, Write,
};

pub enum GetExtensionQuery {
    Id(i64),
    ScriptIdentifier(String),
}
pub struct GetExtensionCommand {
    pub query: GetExtensionQuery,
}
impl DatabaseCommand for GetExtensionCommand {
    type Response = Extension;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        let mut query = ctx.db.select();
        match self.query {
            GetExtensionQuery::Id(id) => query = query.filter("id = ?").bind(id),
            GetExtensionQuery::ScriptIdentifier(script_id) => {
                query = query.filter("script_identifier = ?").bind(script_id)
            }
        };
        query.one().found()
    }
}

/// Every extension with `hook` enabled, in the order its hooks run: by
/// priority, then oldest first.
pub struct GetExtensionsWithHookCommand {
    pub hook: HookKind,
}
impl DatabaseCommand for GetExtensionsWithHookCommand {
    type Response = Vec<Extension>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx
            .db
            .select()
            .filter_in(
                "id",
                ctx.db
                    .select_column::<ExtensionHook, i64>("extension_id")
                    .filter("hook = ? AND enabled")
                    .bind(self.hook),
            )
            .order("priority, id")
            .all()?)
    }
}

/// Every extension with `permission` granted, by priority, then oldest
/// first.
pub struct GetExtensionsWithPermissionCommand {
    pub permission: ExtensionPermissionKind,
}
impl DatabaseCommand for GetExtensionsWithPermissionCommand {
    type Response = Vec<Extension>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx
            .db
            .select()
            .filter_in(
                "id",
                ctx.db
                    .select_column::<ExtensionPermission, i64>("extension_id")
                    .filter("permission = ? AND enabled")
                    .bind(self.permission),
            )
            .order("priority, id")
            .all()?)
    }
}

/// Every installed extension, without its script, in the order its hooks
/// run: by priority, then oldest first.
#[derive(FromRequest)]
#[request(ListExtensionsRequest)]
pub struct ListExtensionsCommand {}
impl DatabaseCommand for ListExtensionsCommand {
    type Response = Vec<ExtensionSummary>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx
            .db
            .select::<Extension>()
            .order("priority, id")
            .all()?
            .into_iter()
            .map(ExtensionSummary::from)
            .collect())
    }
}

/// The permissions recorded for an extension, enabled or not.
#[derive(FromRequest)]
#[request(ListExtensionPermissionsRequest)]
pub struct GetExtensionPermissionsCommand {
    pub extension_id: i64,
}
impl DatabaseCommand for GetExtensionPermissionsCommand {
    type Response = Vec<ExtensionPermission>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx
            .db
            .select()
            .filter("extension_id = ?")
            .bind(self.extension_id)
            .all()?)
    }
}

/// The hooks recorded for an extension, enabled or not.
#[derive(FromRequest)]
#[request(ListExtensionHooksRequest)]
pub struct GetExtensionHooksCommand {
    pub extension_id: i64,
}
impl DatabaseCommand for GetExtensionHooksCommand {
    type Response = Vec<ExtensionHook>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx
            .db
            .select()
            .filter("extension_id = ?")
            .bind(self.extension_id)
            .all()?)
    }
}

/// Whether an extension has `hook` enabled, read as one `EXISTS` so it is
/// cheap to ask at every call.
pub struct IsExtensionHookEnabledCommand {
    pub extension_id: i64,
    pub hook: HookKind,
}
impl DatabaseCommand for IsExtensionHookEnabledCommand {
    type Response = bool;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx
            .db
            .select::<ExtensionHook>()
            .filter("extension_id = ? AND hook = ? AND enabled")
            .bind(self.extension_id)
            .bind(self.hook)
            .exists()?)
    }
}

pub struct CheckExtensionExistsCommand {
    pub identifier: String,
}
impl DatabaseCommand for CheckExtensionExistsCommand {
    type Response = bool;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(!ctx
            .db
            .select_id::<Extension>()
            .filter("script_identifier = ?")
            .bind(self.identifier)
            .all()?
            .is_empty())
    }
}

pub struct CreateExtensionCommand {
    pub script: String,
    pub manifest: ExtensionManifest,
}
impl DatabaseCommand for CreateExtensionCommand {
    type Response = Extension;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let extension: Extension = ctx
            .db
            .insert()
            .set(Extension::SCRIPT, self.script)
            .set(Extension::SCRIPT_IDENTIFIER, self.manifest.identifier)
            .set(Extension::NAME, self.manifest.name)
            .set(Extension::VERSION, self.manifest.version)
            .set(
                Extension::DESCRIPTION,
                self.manifest.description.unwrap_or_default(),
            )
            .set(Extension::AUTHOR, self.manifest.author.unwrap_or_default())
            .set(Extension::ICON, self.manifest.icon)
            .on_conflict_update(&["script_identifier"])
            .returning_one()
            .found()?;
        Ok(extension.invalidating_nothing())
    }
}

/// Installs `script` with `manifest`, or, when `allow_overwrite`, replaces
/// the installed extension with its identifier, keeping its id. Grants the
/// manifest no longer lists are dropped, and the rest are recorded switched
/// off unless the user already granted them; its service declaration
/// replaces the one recorded, and its connector definition, for an `oauth`
/// extension, is stored or replaces an older version. One command, so an
/// install is all or nothing.
pub struct InstallExtensionCommand {
    pub script: String,
    pub manifest: ExtensionManifest,
    /// What the extension's `serviceDescription()` defines, when it lists
    /// `oauth`.
    pub connector: Option<ConnectorDefinition>,
    pub allow_overwrite: bool,
}
impl DatabaseCommand for InstallExtensionCommand {
    type Response = Extension;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let existing = CheckExtensionExistsCommand {
            identifier: self.manifest.identifier.clone(),
        }
        .handle(&mut ctx.as_read())?;
        if existing && !self.allow_overwrite {
            return Err(ValidationError::new(
                "This extension is already installed. To replace it, install it again with overwrite on.",
            )
            .into());
        }
        let permissions = self.manifest.permissions.clone();
        let hooks = self.manifest.hooks.clone();
        let service = self.manifest.service.clone();
        let extension = ctx.run(CreateExtensionCommand {
            script: self.script,
            manifest: self.manifest,
        })?;
        ctx.run(ClearExtensionPermissionsCommand {
            extension_id: extension.id,
            excluded_permissions: permissions.clone(),
        })?;
        ctx.run(ClearExtensionHooksCommand {
            extension_id: extension.id,
            excluded_hooks: hooks.clone(),
        })?;
        for permission in permissions {
            ctx.run(EnsureExtensionPermissionCommand {
                extension_id: extension.id,
                permission,
            })?;
        }
        for hook in hooks {
            ctx.run(EnsureExtensionHookCommand {
                extension_id: extension.id,
                hook,
            })?;
        }
        ctx.run(ReplaceExtensionServiceCommand {
            extension_id: extension.id,
            service,
        })?;
        let connector = match self.connector {
            Some(connector) => upsert_connector(ctx, connector)?.map(|_| ()),
            None => ().invalidating_nothing(),
        };
        // An update for the version just installed no longer applies.
        Ok(extension
            .invalidating([Invalidation::ExtensionUpdates])
            .merge(connector))
    }
}

/// Records `service` as the extension's declaration in place of the one
/// recorded, or drops it when `None`.
pub struct ReplaceExtensionServiceCommand {
    pub extension_id: i64,
    pub service: Option<ServiceManifest>,
}
impl DatabaseCommand for ReplaceExtensionServiceCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        // Its functions cascade.
        ctx.db
            .delete::<ExtensionService>()
            .filter_eq(ExtensionService::EXTENSION_ID, self.extension_id)
            .execute()?;
        let Some(service) = self.service else {
            return Ok(().invalidating_nothing());
        };
        let row: ExtensionService = ctx
            .db
            .insert()
            .set(ExtensionService::EXTENSION_ID, self.extension_id)
            .set(
                ExtensionService::DESCRIPTION,
                service.description.unwrap_or_default(),
            )
            .returning_one()
            .found()?;
        for (name, function) in service.functions {
            ctx.db
                .insert::<ExtensionServiceFunction>()
                .set(ExtensionServiceFunction::SERVICE_ID, row.id)
                .set(ExtensionServiceFunction::NAME, name)
                .set(
                    ExtensionServiceFunction::DESCRIPTION,
                    function.description.unwrap_or_default(),
                )
                .set(ExtensionServiceFunction::ARGS, function.args)
                .set(ExtensionServiceFunction::RETURNS, function.returns)
                .set(
                    ExtensionServiceFunction::READ_ONLY,
                    function.read_only.unwrap_or(false),
                )
                .execute()?;
        }
        Ok(().invalidating_nothing())
    }
}

/// The service extension `extension_id` declared, whatever its grants, or
/// `None` when its manifest has no `service` block.
pub struct GetExtensionServiceCommand {
    pub extension_id: i64,
}
impl DatabaseCommand for GetExtensionServiceCommand {
    type Response = Option<InstalledService>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        let service = ctx
            .db
            .select::<ExtensionService>()
            .filter_eq(ExtensionService::EXTENSION_ID, self.extension_id)
            .one()?;
        service
            .map(|service| installed_service(ctx, service))
            .transpose()
    }
}

/// The extensions scripts may call as services: those with `service`
/// enabled. Those listing `service:isolated` instead serve clients only.
fn callable_services<'c>(db: impl ReadableDatabase<'c>) -> Select<'c, ExtensionPermission, i64> {
    db.select_column("extension_id")
        .filter("permission = ? AND enabled")
        .bind(ExtensionPermissionKind::Service)
}

/// The service the extension with `script_identifier` declared, provided
/// scripts may call it: it has `service` enabled. `None` otherwise, as
/// when no extension has that identifier.
pub struct GetCallableServiceCommand {
    pub script_identifier: String,
}
impl DatabaseCommand for GetCallableServiceCommand {
    type Response = Option<InstalledService>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        let service = ctx
            .db
            .select::<ExtensionService>()
            .filter_in(
                "extension_id",
                ctx.db
                    .select_column::<Extension, i64>("id")
                    .filter("script_identifier = ?")
                    .bind(self.script_identifier),
            )
            .filter_in("extension_id", callable_services(ctx.db))
            .one()?;
        service
            .map(|service| installed_service(ctx, service))
            .transpose()
    }
}

/// Every declared service scripts may call, as
/// [`GetCallableServiceCommand`] finds them, by script identifier.
pub struct ListServicesCommand {}
impl DatabaseCommand for ListServicesCommand {
    type Response = Vec<InstalledService>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        let services: Vec<ExtensionService> = ctx
            .db
            .select()
            .filter_in("extension_id", callable_services(ctx.db))
            .all()?;
        let mut installed = services
            .into_iter()
            .map(|service| installed_service(ctx, service))
            .collect::<DatabaseCommandResult<Vec<_>>>()?;
        installed.sort_by(|a, b| a.script_identifier.cmp(&b.script_identifier));
        Ok(installed)
    }
}

/// `service` with its extension and its functions, by name.
fn installed_service(
    ctx: &mut CommandContext<'_, Read>,
    service: ExtensionService,
) -> DatabaseCommandResult<InstalledService> {
    let extension: Extension = ctx
        .db
        .select()
        .filter_eq(Extension::ID, service.extension_id)
        .one()
        .found()?;
    let functions: Vec<ExtensionServiceFunction> = ctx
        .db
        .select()
        .filter_eq(ExtensionServiceFunction::SERVICE_ID, service.id)
        .order("name")
        .all()?;
    Ok(InstalledService {
        extension_id: extension.id,
        script_identifier: extension.script_identifier,
        name: extension.name,
        description: service.description,
        functions: functions.into_iter().map(Into::into).collect(),
    })
}

/// Enables or disables `permission` for an extension. A running extension
/// scope keeps its globals until it is shut down and respawned.
#[derive(FromRequest)]
#[request(SetExtensionPermissionRequest)]
pub struct SetExtensionPermissionCommand {
    pub extension_id: i64,
    pub permission: ExtensionPermissionKind,
    pub enabled: bool,
}
impl DatabaseCommand for SetExtensionPermissionCommand {
    type Response = AcceptedResponse;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ctx.db
            .insert::<ExtensionPermission>()
            .set(ExtensionPermission::EXTENSION_ID, self.extension_id)
            .set(ExtensionPermission::PERMISSION, self.permission)
            .set(ExtensionPermission::ENABLED, self.enabled)
            .on_conflict_update(&["extension_id", "permission"])
            .returning_one()
            .found()?;
        Ok(AcceptedResponse {}.invalidating_nothing())
    }
}

/// Records `permission` for an extension, switched off, unless it is
/// already recorded, in which case its grant stays as it is.
pub struct EnsureExtensionPermissionCommand {
    pub extension_id: i64,
    pub permission: ExtensionPermissionKind,
}
impl DatabaseCommand for EnsureExtensionPermissionCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        // `enabled` defaults to false; with only the key columns set, the
        // upsert does nothing on conflict.
        ctx.db
            .insert::<ExtensionPermission>()
            .set(ExtensionPermission::EXTENSION_ID, self.extension_id)
            .set(ExtensionPermission::PERMISSION, self.permission)
            .on_conflict_update(&["extension_id", "permission"])
            .execute()?;
        Ok(().invalidating_nothing())
    }
}

pub struct ClearExtensionPermissionsCommand {
    pub extension_id: i64,
    pub excluded_permissions: Vec<ExtensionPermissionKind>,
}
impl DatabaseCommand for ClearExtensionPermissionsCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ctx.db
            .delete::<ExtensionPermission>()
            .filter_eq(ExtensionPermission::EXTENSION_ID, self.extension_id)
            .filter_not_in_values("permission", self.excluded_permissions)
            .execute()?;
        Ok(().invalidating_nothing())
    }
}

/// Enables or disables `hook` for an extension, from the next time it
/// fires.
#[derive(FromRequest)]
#[request(SetExtensionHookRequest)]
pub struct SetExtensionHookCommand {
    pub extension_id: i64,
    pub hook: HookKind,
    pub enabled: bool,
}
impl DatabaseCommand for SetExtensionHookCommand {
    type Response = AcceptedResponse;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ctx.db
            .insert::<ExtensionHook>()
            .set(ExtensionHook::EXTENSION_ID, self.extension_id)
            .set(ExtensionHook::HOOK, self.hook)
            .set(ExtensionHook::ENABLED, self.enabled)
            .on_conflict_update(&["extension_id", "hook"])
            .returning_one()
            .found()?;
        Ok(AcceptedResponse {}.invalidating_nothing())
    }
}

/// Records `hook` for an extension, switched off, unless it is already
/// recorded, in which case it stays as it is.
pub struct EnsureExtensionHookCommand {
    pub extension_id: i64,
    pub hook: HookKind,
}
impl DatabaseCommand for EnsureExtensionHookCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        // As for permissions: `enabled` defaults to false, and a conflict
        // does nothing.
        ctx.db
            .insert::<ExtensionHook>()
            .set(ExtensionHook::EXTENSION_ID, self.extension_id)
            .set(ExtensionHook::HOOK, self.hook)
            .on_conflict_update(&["extension_id", "hook"])
            .execute()?;
        Ok(().invalidating_nothing())
    }
}

pub struct ClearExtensionHooksCommand {
    pub extension_id: i64,
    pub excluded_hooks: Vec<HookKind>,
}
impl DatabaseCommand for ClearExtensionHooksCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ctx.db
            .delete::<ExtensionHook>()
            .filter_eq(ExtensionHook::EXTENSION_ID, self.extension_id)
            .filter_not_in_values("hook", self.excluded_hooks)
            .execute()?;
        Ok(().invalidating_nothing())
    }
}

/// Records `version` as the update announced for an extension, answering
/// whether it wasn't already: one statement, so of two checks finding the
/// same update only one announces it.
pub struct AnnounceExtensionUpdateCommand {
    pub extension_id: i64,
    pub version: i64,
}
impl DatabaseCommand for AnnounceExtensionUpdateCommand {
    type Response = bool;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let changed = ctx
            .db
            .update::<Extension>()
            .set(Extension::ANNOUNCED_VERSION, self.version)
            .filter("id = ? AND announced_version IS NOT ?")
            .bind(self.extension_id)
            .bind(self.version)
            .execute()?;
        Ok((changed == 1).invalidating_nothing())
    }
}

/// Removes an extension; its permission and hook grants and service
/// declaration cascade with it, and the schema orphans its connector
/// accounts, which keep their resources but lose the endpoints signed in
/// with them.
pub struct DeleteExtensionCommand {
    pub extension_id: i64,
}
impl DatabaseCommand for DeleteExtensionCommand {
    type Response = Extension;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let extension: Extension = ctx
            .db
            .delete()
            .filter_eq(Extension::ID, self.extension_id)
            .returning_one()
            .found()?;
        Ok(extension.invalidating([
            Invalidation::ConnectorAccounts,
            Invalidation::ExtensionUpdates,
            Invalidation::Intelligence,
        ]))
    }
}

#[cfg(test)]
mod tests;
