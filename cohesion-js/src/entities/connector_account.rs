//! `ConnectorAccount` in scripts.

use cohesion_core::model::{AccountHost, ConnectorAccount, ConnectorResourceFilters};
use cohesion_db::{GetAccountConnectorCommand, ImportResourceCommand, ListAccountResourcesCommand};
use cohesion_macros::js_entity;
use rquickjs::function::Opt;
use rquickjs::{Class, Ctx, Value};

use crate::connector_access::{current_connectors, throw_server_error};
use crate::conversion::plain;
use crate::entities::connector::JsConnector;
use crate::entities::connector_account_options::{
    AccountResourceListOptions, ConnectorItemFilters,
};
use crate::entities::resource::JsResource;
use crate::entities::workspace::JsWorkspace;
use crate::sandbox::ScriptDeadline;

/// An account the user added at a connected service, from
/// `connectors.accounts()` or `connectors.account(id)`. `connector()` is the
/// service it is an account of, saying what each kind of item holds, and
/// `createdAt` is in Unix seconds. An orphaned account, whose extension was
/// uninstalled, keeps its `resources()` but can't be browsed. Items already imported from an
/// account are `resources()`; `organizationUnits()`, `items()` and `fetch()`
/// read the live service and store nothing. Use them only after
/// `workspace.resources()` and `workspace.search()` have not found what you
/// need.
#[js_entity(ConnectorAccount, js = "ConnectorAccount")]
impl JsConnectorAccount {
    /// The service this is an account of.
    pub fn connector<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<JsConnector> {
        let connector = self.read(
            &ctx,
            GetAccountConnectorCommand {
                account_id: self.inner.id,
            },
        )?;
        Ok(JsConnector::new(connector, self.db.clone()))
    }

    /// The account's identifier at the service; null for an orphaned
    /// account.
    #[qjs(get, rename = "identifier")]
    pub fn identifier(&self) -> Option<String> {
        match &self.inner.host {
            AccountHost::Extension { identifier, .. } => Some(identifier.clone()),
            AccountHost::Orphaned => None,
        }
    }

    /// The parts the service groups items into (team spaces, projects,
    /// calendars), to narrow `items({ organizationUnitId })`. Reads the live
    /// service.
    #[qjs(rename = "organizationUnits")]
    #[ts(returns = "OrganizationUnit[]")]
    pub fn organization_units<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        let units = current_connectors(&ctx)?
            .organization_units(self.inner.id, ScriptDeadline::remaining_until(&ctx)?)
            .map_err(|e| throw_server_error(&ctx, e))?;
        plain(&ctx, &units)
    }

    /// Items the service offers for import, as previews (`identifier`,
    /// `title`, `kindIdentifier`, `lastUpdated`, `metadata`), within one organization
    /// unit or matching `search` when given. Reads the live service and
    /// stores nothing; `resources()` is what is already imported.
    #[ts(returns = "ConnectorResourcePreview[]")]
    pub fn items<'js>(
        &self,
        ctx: Ctx<'js>,
        filters: Opt<ConnectorItemFilters>,
    ) -> rquickjs::Result<Value<'js>> {
        let filters = filters.0.unwrap_or_default();
        let previews = current_connectors(&ctx)?
            .list_resources(
                self.inner.id,
                ConnectorResourceFilters {
                    organization_unit_id: filters.organization_unit_id,
                    search: filters.search,
                },
                ScriptDeadline::remaining_until(&ctx)?,
            )
            .map_err(|e| throw_server_error(&ctx, e))?;
        plain(&ctx, &previews)
    }

    /// The whole item `identifier` names, contents included, as the service
    /// has it now. Nothing is stored: use it to read an item before
    /// suggesting an import, or to answer a one-off question. Throws if the
    /// service has no such item.
    #[ts(returns = "ConnectorResource")]
    pub fn fetch<'js>(&self, ctx: Ctx<'js>, identifier: String) -> rquickjs::Result<Value<'js>> {
        let fetched = current_connectors(&ctx)?
            .fetch_resource(
                self.inner.id,
                identifier,
                ScriptDeadline::remaining_until(&ctx)?,
            )
            .map_err(|e| throw_server_error(&ctx, e))?;
        plain(&ctx, &fetched)
    }

    /// The resources already imported from this account, into `workspace` or
    /// into every workspace.
    pub fn resources<'js>(
        &self,
        ctx: Ctx<'js>,
        options: Opt<AccountResourceListOptions>,
    ) -> rquickjs::Result<Vec<JsResource>> {
        let options = options.0.unwrap_or_default();
        let resources = self.read(
            &ctx,
            ListAccountResourcesCommand {
                account_id: self.inner.id,
                workspace_id: options.workspace.map(|workspace| workspace.inner().id),
            },
        )?;
        Ok(resources
            .into_iter()
            .map(|resource| JsResource::new(resource, self.db.clone()))
            .collect())
    }

    /// Imports the item `identifier` names into `workspace` as a resource,
    /// replacing the copy already imported there from this account, and
    /// returns it. The daemon then keeps it fresh. A change the user decides
    /// on: in a conversation, only once the user has agreed to this item and
    /// workspace; in a turn a cue started this throws, so ask the user first.
    pub fn import<'js>(
        &self,
        ctx: Ctx<'js>,
        identifier: String,
        workspace: Class<'js, JsWorkspace>,
    ) -> rquickjs::Result<JsResource> {
        let fetched = current_connectors(&ctx)?
            .fetch_resource(
                self.inner.id,
                identifier,
                ScriptDeadline::remaining_until(&ctx)?,
            )
            .map_err(|e| throw_server_error(&ctx, e))?;
        let resource = self.write(
            &ctx,
            ImportResourceCommand {
                workspace_id: workspace.borrow().inner().id,
                account_id: self.inner.id,
                resource: fetched,
            },
        )?;
        Ok(JsResource::new(resource, self.db.clone()))
    }
}
