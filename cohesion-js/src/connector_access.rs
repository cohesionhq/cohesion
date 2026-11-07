//! How agent scripts reach connectors' items, through the daemon above this
//! crate.

use std::sync::Arc;
use std::time::Instant;

use cohesion_core::model::{
    ConnectorResource, ConnectorResourceFilters, ConnectorResourcePreview, OrganizationUnit,
};
use cohesion_core::protocol::{ErrorCode, ServerError};
use rquickjs::{Ctx, Exception, JsLifetime};

/// Reaches the items of a connector account through its extension's import
/// functions, with the account's credential. Implemented above this crate,
/// where extensions are called, and blocking, since scripts run on their own
/// threads. Each call gives up with an error when the script's `deadline`
/// passes, so a slow service can't hold the script past it.
pub trait ConnectorAccess: Send + Sync {
    /// What `getOrganizationUnits` returns for account `account_id`.
    fn organization_units(
        &self,
        account_id: i64,
        deadline: Instant,
    ) -> Result<Vec<OrganizationUnit>, ServerError>;
    /// What `listResources(filters)` returns for account `account_id`.
    fn list_resources(
        &self,
        account_id: i64,
        filters: ConnectorResourceFilters,
        deadline: Instant,
    ) -> Result<Vec<ConnectorResourcePreview>, ServerError>;
    /// What `getResource(identifier)` returns for account `account_id`.
    fn fetch_resource(
        &self,
        account_id: i64,
        identifier: String,
        deadline: Instant,
    ) -> Result<ConnectorResource, ServerError>;
}

/// The connector access of the agent job that is running, stored as context
/// userdata by [`Sandbox::with_connectors`](crate::Sandbox::with_connectors)
/// for the job's duration.
#[derive(JsLifetime)]
pub(crate) struct CurrentConnectors(pub Arc<dyn ConnectorAccess>);

/// The running job's connector access; throws when the job has none (a cue,
/// or a sandbox built without it).
pub(crate) fn current_connectors(ctx: &Ctx<'_>) -> rquickjs::Result<Arc<dyn ConnectorAccess>> {
    ctx.userdata::<CurrentConnectors>()
        .map(|current| current.0.clone())
        .ok_or_else(|| {
            Exception::throw_message(
                ctx,
                "`connectors` is only available while an agent's script is running.",
            )
        })
}

/// What a failed call to a service throws: a `ReferenceError` when the item
/// or account was not found, otherwise an `Error` with the message the
/// daemon would show the user.
pub(crate) fn throw_server_error(ctx: &Ctx<'_>, error: ServerError) -> rquickjs::Error {
    match error.code {
        ErrorCode::NotFound => Exception::throw_reference(ctx, &error.message),
        _ => Exception::throw_message(ctx, &error.message),
    }
}
