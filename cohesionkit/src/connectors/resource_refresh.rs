//! Refreshing imported resources from their services: on a timer from daemon
//! start, and on request for one workspace. Each account lists its items
//! first and fetches only those whose `lastUpdated` moved, or that it didn't
//! list. An orphaned account's resources are skipped.

use std::collections::HashMap;
use std::time::Duration;

use cohesion_core::log::Module;
use cohesion_core::model::{AccountHost, ConnectorAccount, ConnectorResourceFilters, Resource};
use cohesion_core::protocol::{ResourceRefreshSummary, ServerError};
use cohesion_core::{log_error, log_info};
use cohesion_db::{
    ListAccountResourcesCommand, ListConnectorAccountRowsCommand, UpdateResourceCommand,
};

use crate::cohesion_context::CohesionContext;
use crate::connectors::connector_calls::connect;

/// How long after start the first pass runs.
const FIRST_PASS: Duration = Duration::from_secs(60);
/// How long between passes.
const INTERVAL: Duration = Duration::from_secs(30 * 60);
impl CohesionContext {
    /// Refreshes every resource a minute after start, then every 30 minutes,
    /// until the daemon shuts down; a pass under way then runs to its end.
    pub(crate) fn start_resource_refresh(&self) {
        let ctx = self.clone();
        self.work
            .every("resource refresh", FIRST_PASS, INTERVAL, move || {
                let ctx = ctx.clone();
                async move {
                    refresh_resources(&ctx, None).await;
                }
            });
    }
}

/// Refreshes every resource, or those in `workspace_id`, rewriting the ones
/// that changed. A failure is logged and counted, never ending the pass.
pub(crate) async fn refresh_resources(
    ctx: &CohesionContext,
    workspace_id: Option<i64>,
) -> ResourceRefreshSummary {
    match ctx.database.send(ListConnectorAccountRowsCommand {}).await {
        Ok(accounts) => refresh_accounts(ctx, accounts, workspace_id).await,
        Err(e) => {
            log_error!(
                Module::Extensions,
                "Listing connector accounts failed: {e:?}"
            );
            ResourceRefreshSummary::default()
        }
    }
}

/// Refreshes the resources imported from `accounts`, in `workspace_id` or
/// every workspace.
async fn refresh_accounts(
    ctx: &CohesionContext,
    accounts: Vec<ConnectorAccount>,
    workspace_id: Option<i64>,
) -> ResourceRefreshSummary {
    let mut summary = ResourceRefreshSummary::default();
    for account in accounts {
        if account.host == AccountHost::Orphaned {
            continue;
        }
        let resources = match ctx
            .database
            .send(ListAccountResourcesCommand {
                account_id: account.id,
                workspace_id,
            })
            .await
        {
            Ok(resources) => resources,
            Err(e) => {
                log_error!(
                    Module::Extensions,
                    "Listing the resources of connector account {} failed: {e:?}",
                    account.id
                );
                continue;
            }
        };
        if resources.is_empty() {
            continue;
        }
        refresh_account(ctx, &account, resources, &mut summary).await;
    }
    log_info!(
        Module::Extensions,
        "Resource refresh checked {}, updated {}, failed {}.",
        summary.checked,
        summary.updated,
        summary.failed
    );
    summary
}

/// Lists `account`'s items, then fetches each of `resources`, imported from
/// it, whose item changed since or wasn't listed. Each call to an extension
/// checks its grants afresh, so one uninstalled or revoked mid-pass is not
/// called again.
async fn refresh_account(
    ctx: &CohesionContext,
    account: &ConnectorAccount,
    resources: Vec<Resource>,
    summary: &mut ResourceRefreshSummary,
) {
    let failed = |e: ServerError| {
        log_error!(
            Module::Extensions,
            "Refreshing connector account {} ({}) failed: {}",
            account.id,
            account.description,
            e.message
        );
    };
    let connection = match connect(&ctx.extensions, &ctx.database, account).await {
        Ok(connection) => connection,
        Err(e) => {
            failed(e);
            summary.failed += resources.len() as i64;
            return;
        }
    };
    let listed: HashMap<String, Option<i64>> = match connection
        .list(&ctx.extensions, ConnectorResourceFilters::default())
        .await
    {
        Ok(previews) => previews
            .into_iter()
            .map(|preview| (preview.identifier, preview.last_updated))
            .collect(),
        Err(e) => {
            failed(e);
            summary.failed += resources.len() as i64;
            return;
        }
    };
    for resource in resources {
        if listed.get(&resource.identifier) == Some(&Some(resource.last_updated)) {
            summary.checked += 1;
            continue;
        }
        let refreshed = async {
            let fresh = connection
                .fetch(&ctx.extensions, resource.identifier.clone())
                .await?;
            Ok::<_, ServerError>(
                ctx.database
                    .send(UpdateResourceCommand {
                        workspace_id: resource.workspace_id,
                        resource_id: resource.id,
                        resource: fresh,
                    })
                    .await?,
            )
        }
        .await;
        match refreshed {
            Ok(updated) => {
                summary.checked += 1;
                if updated.is_some() {
                    summary.updated += 1;
                }
            }
            Err(e) => {
                summary.failed += 1;
                failed(e);
            }
        }
    }
}
