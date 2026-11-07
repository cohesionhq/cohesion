//! How agent scripts reach connectors: the engine's [`ConnectorAccess`],
//! running the daemon's async connector path on the runtime from the
//! script's own thread.

use std::future::Future;
use std::time::Instant;

use cohesion_core::model::{
    ConnectorResource, ConnectorResourceFilters, ConnectorResourcePreview, OrganizationUnit,
};
use cohesion_core::protocol::ServerError;
use cohesion_db::DatabaseContext;
use cohesion_js::ConnectorAccess;
use tokio::runtime::Handle;
use tokio::time::timeout_at;

use crate::connectors::connector_calls::{
    account_items, account_organization_units, fetch_resource,
};
use crate::extension_manager::ExtensionManager;

/// Calls connector extensions' import functions for agent scripts, which
/// run on their own threads, by blocking on the runtime.
pub(crate) struct AgentConnectors {
    database: DatabaseContext,
    extensions: ExtensionManager,
    runtime: Handle,
}

impl AgentConnectors {
    /// Must be called inside the tokio runtime, which runs the calls.
    pub(crate) fn new(database: DatabaseContext, extensions: ExtensionManager) -> Self {
        Self {
            database,
            extensions,
            runtime: Handle::current(),
        }
    }
}

/// Blocks the calling thread, which is outside the runtime, on `call` on
/// `runtime`, giving up when `deadline` passes, so a slow service can't hold
/// the script past it. Dropping the call stops the wait, not the service's
/// job. The timeout is built inside the runtime's context: built as the
/// argument to `block_on`, it looks for a runtime on this thread and panics.
pub(super) fn block_until<T>(
    runtime: &Handle,
    deadline: Instant,
    call: impl Future<Output = Result<T, ServerError>>,
) -> Result<T, ServerError> {
    runtime
        .block_on(async { timeout_at(tokio::time::Instant::from_std(deadline), call).await })
        .unwrap_or_else(|_| {
            Err(ServerError::upstream(
                "The service didn't answer before this script's time ran out.",
            ))
        })
}

impl ConnectorAccess for AgentConnectors {
    fn organization_units(
        &self,
        account_id: i64,
        deadline: Instant,
    ) -> Result<Vec<OrganizationUnit>, ServerError> {
        block_until(
            &self.runtime,
            deadline,
            account_organization_units(&self.extensions, &self.database, account_id),
        )
    }

    fn list_resources(
        &self,
        account_id: i64,
        filters: ConnectorResourceFilters,
        deadline: Instant,
    ) -> Result<Vec<ConnectorResourcePreview>, ServerError> {
        block_until(
            &self.runtime,
            deadline,
            account_items(&self.extensions, &self.database, account_id, filters),
        )
    }

    fn fetch_resource(
        &self,
        account_id: i64,
        identifier: String,
        deadline: Instant,
    ) -> Result<ConnectorResource, ServerError> {
        block_until(
            &self.runtime,
            deadline,
            fetch_resource(&self.extensions, &self.database, account_id, identifier),
        )
    }
}
