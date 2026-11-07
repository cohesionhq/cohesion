//! Clearing out what has outlived its use, on a timer from daemon start:
//! the database's own cleanup and a vacuum if it freed enough, then the clients that stopped connecting,
//! which needs to know who is connected now.

use std::time::Duration;

use cohesion_core::log::Module;
use cohesion_core::{log_error, log_info};
use cohesion_db::{PeriodicCleanupCommand, RevokeInactiveClientsCommand};

use crate::cohesion_context::CohesionContext;

/// How long between passes.
const INTERVAL: Duration = Duration::from_secs(60 * 60);

impl CohesionContext {
    /// Runs [`PeriodicCleanupCommand`] and revokes the clients that stopped
    /// connecting at start, then every hour, until the daemon shuts down.
    pub(crate) fn start_periodic_cleanup(&self) {
        let ctx = self.clone();
        self.work
            .every("cleanup", Duration::ZERO, INTERVAL, move || {
                let ctx = ctx.clone();
                async move {
                    if let Err(e) = ctx.database.send(PeriodicCleanupCommand).await {
                        log_error!(Module::Database, "Cleanup failed: {e:?}");
                    }
                    match ctx.database.vacuum_if_reclaimable().await {
                        Ok(Some(free)) => log_info!(
                            Module::Database,
                            "Vacuumed the database, returning {} MB.",
                            free / (1024 * 1024)
                        ),
                        Ok(None) => {}
                        Err(e) => log_error!(Module::Database, "Vacuum failed: {e:?}"),
                    }
                    ctx.revoke_inactive_clients().await;
                }
            });
    }
}

impl CohesionContext {
    /// Revokes the clients the security settings say have stopped
    /// connecting, never one connected now, and ends any session one
    /// opened since.
    async fn revoke_inactive_clients(&self) {
        let online = self.presence.online_clients();
        match self
            .database
            .send(RevokeInactiveClientsCommand { online })
            .await
        {
            Ok(revoked) => {
                for client in revoked {
                    log_info!(
                        Module::Server,
                        "Revoked {} after it stopped connecting.",
                        client.client_name
                    );
                    self.presence.revoke(client.id);
                }
            }
            Err(e) => log_error!(Module::Database, "Revoking inactive clients failed: {e:?}"),
        }
    }
}
