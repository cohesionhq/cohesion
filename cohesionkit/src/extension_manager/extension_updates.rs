//! Finding updates for installed extensions: every registry is asked, through
//! its `registryLookup`, for the installed identifiers at daemon start, then
//! daily, and on request. The answer is kept until the next check, less what
//! an uninstall removes. Each update raises `extension:update` the first time
//! a check finds its version.

use std::time::Duration;

use cohesion_core::invalidation::Invalidation;
use cohesion_core::log::Module;
use cohesion_core::protocol::{ExtensionUpdate, ServerError};
use cohesion_core::{log_error, log_info};
use cohesion_db::{AnnounceExtensionUpdateCommand, ListExtensionsCommand};

use super::registry_fns::RegistryLookupFn;
use super::{ExtensionError, ExtensionFn, ExtensionManager, script_fault};
use crate::cohesion_context::CohesionContext;
use crate::extension_icon::accepted_icon;
use crate::hooks::ExtensionUpdateHook;

/// How long between checks.
const INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

impl CohesionContext {
    /// Checks for extension updates now, then every day, until the daemon
    /// shuts down.
    pub(crate) fn start_update_checks(&self) {
        let ctx = self.clone();
        self.work
            .every("update check", Duration::ZERO, INTERVAL, move || {
                let ctx = ctx.clone();
                async move {
                    if let Err(e) = ctx.extensions.check_updates().await {
                        log_error!(Module::Extensions, "Update check failed: {}", e.message);
                    }
                }
            });
    }
}

impl ExtensionManager {
    /// Asks every registry for the installed extensions, keeping one update
    /// per extension a registry lists at a higher version: the first
    /// registry's, in priority order. A registry that fails is logged and
    /// skipped. An update at a version not yet announced for its extension
    /// raises `extension:update`.
    pub(crate) async fn check_updates(&self) -> Result<Vec<ExtensionUpdate>, ServerError> {
        let installed = self.inner.database.send(ListExtensionsCommand {}).await?;
        let identifiers: Vec<String> = installed
            .iter()
            .map(|e| e.script_identifier.clone())
            .collect();
        let answers = self
            .call_each(RegistryLookupFn::REQUIRES, "Registry lookup", |registry| {
                let manager = self.clone();
                let identifiers = identifiers.clone();
                async move {
                    let looked_up = manager
                        .call(registry.id, RegistryLookupFn { identifiers })
                        .await;
                    match looked_up {
                        Ok(results) => results.into_iter().map(|r| (registry.id, r)).collect(),
                        Err(ExtensionError::Script(e)) => {
                            let name = RegistryLookupFn::NAME;
                            let fault = script_fault(&registry.name, name, e, |message| {
                                format!("{name} didn't return RegistryResult[]: {message}")
                            });
                            if let Some(fault) = fault {
                                log_error!(Module::Extensions, "{}: {fault}", registry.name);
                            }
                            Vec::new()
                        }
                        Err(e) => {
                            log_error!(
                                Module::Extensions,
                                "{}: {} failed: {e}",
                                registry.name,
                                RegistryLookupFn::NAME
                            );
                            Vec::new()
                        }
                    }
                }
            })
            .await?;

        let mut updates: Vec<ExtensionUpdate> = Vec::new();
        for (registry_extension_id, mut result) in answers.into_iter().flatten() {
            let Some(extension) = installed
                .iter()
                .find(|e| e.script_identifier == result.manifest.identifier)
            else {
                continue;
            };
            if result.manifest.version <= extension.version
                || updates.iter().any(|u| u.extension_id == extension.id)
            {
                continue;
            }
            result.manifest.icon = accepted_icon(result.manifest.icon.take());
            updates.push(ExtensionUpdate {
                extension_id: extension.id,
                registry_extension_id,
                result,
            });
        }
        log_info!(
            Module::Extensions,
            "Update check found {} of {} extensions out of date.",
            updates.len(),
            installed.len()
        );
        *self.inner.updates.lock().unwrap() = updates.clone();
        self.inner.database.publish(Invalidation::ExtensionUpdates);
        for update in &updates {
            let manifest = &update.result.manifest;
            let announced = self
                .inner
                .database
                .send(AnnounceExtensionUpdateCommand {
                    extension_id: update.extension_id,
                    version: manifest.version,
                })
                .await?;
            if !announced {
                continue;
            }
            let extension = installed
                .iter()
                .find(|e| e.id == update.extension_id)
                .expect("an update is for an installed extension");
            self.inner.hooks.send(ExtensionUpdateHook {
                extension_id: extension.id,
                identifier: extension.script_identifier.clone(),
                name: extension.name.clone(),
                installed_version: extension.version,
                version: manifest.version,
                source_url: update.result.source_url.clone(),
            });
        }
        Ok(updates)
    }

    /// The last check's updates that still apply: the extension is
    /// installed, is still the one the update is for (an id freed by an
    /// uninstall can be reused), and is at a lower version than the
    /// registry's.
    pub(crate) async fn updates(&self) -> Result<Vec<ExtensionUpdate>, ServerError> {
        let installed = self.inner.database.send(ListExtensionsCommand {}).await?;
        let updates = self.inner.updates.lock().unwrap().clone();
        Ok(updates
            .into_iter()
            .filter(|u| {
                installed.iter().any(|e| {
                    e.id == u.extension_id
                        && e.script_identifier == u.result.manifest.identifier
                        && e.version < u.result.manifest.version
                })
            })
            .collect())
    }
}
