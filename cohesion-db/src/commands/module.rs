use cohesion_core::FromRequest;
use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{EnabledModule, Module, ModuleSetting};
use cohesion_core::protocol::{ListWorkspaceModulesRequest, SetModuleEnabledRequest};

use crate::changed::{Changed, Invalidates};
use crate::commands::workspace_scope::ensure_workspace;
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, DatabaseError, Filtered, Read,
    ReadableDatabase, Write,
};

/// Every module, in order, with whether it is on in `workspace_id`: on
/// unless its row says otherwise.
fn settings<'c>(
    db: impl ReadableDatabase<'c>,
    workspace_id: i64,
) -> Result<Vec<ModuleSetting>, DatabaseError> {
    let rows: Vec<EnabledModule> = db
        .select()
        .filter_eq(EnabledModule::WORKSPACE_ID, workspace_id)
        .all()?;
    Ok(Module::ALL
        .iter()
        .map(|module| ModuleSetting {
            module: *module,
            title: module.title().to_string(),
            description: module.description().to_string(),
            enabled: rows
                .iter()
                .find(|row| row.module_name == module.as_str())
                .is_none_or(|row| row.enabled),
        })
        .collect())
}

/// Every module and whether it is on in a workspace.
#[derive(FromRequest)]
#[request(ListWorkspaceModulesRequest)]
pub struct ListWorkspaceModulesCommand {
    pub workspace_id: i64,
}
impl DatabaseCommand for ListWorkspaceModulesCommand {
    type Response = Vec<ModuleSetting>;
    type Access = Read;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Read>,
    ) -> DatabaseCommandResult<Vec<ModuleSetting>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        Ok(settings(ctx.db, self.workspace_id)?)
    }
}

/// Turns a module on or off in a workspace. Returns every module and
/// whether it is on there.
#[derive(FromRequest)]
#[request(SetModuleEnabledRequest)]
pub struct SetModuleEnabledCommand {
    pub workspace_id: i64,
    pub module: Module,
    pub enabled: bool,
}
impl DatabaseCommand for SetModuleEnabledCommand {
    type Response = Vec<ModuleSetting>;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Vec<ModuleSetting>>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        ctx.db
            .insert::<EnabledModule>()
            .set(EnabledModule::WORKSPACE_ID, self.workspace_id)
            .set(EnabledModule::MODULE_NAME, self.module.as_str().to_string())
            .set(EnabledModule::ENABLED, self.enabled)
            .on_conflict_update(&["workspace_id", "module_name"])
            .execute()?;
        Ok(settings(ctx.db.read(), self.workspace_id)?
            .invalidating([Invalidation::Modules(self.workspace_id)]))
    }
}

#[cfg(test)]
mod tests;
