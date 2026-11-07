//! Runs queued hooks in the extensions that have them enabled, then hands
//! them to the agents with cues on them.

use cohesion_core::{log::Module, log_error, log_info, log_warn};
use cohesion_db::{
    CommandError, DatabaseContext, GetExtensionCommand, GetExtensionQuery,
    GetExtensionsWithHookCommand,
};
use cohesion_intelligence::AgentCoordinator;
use tokio::sync::mpsc;

use super::hook_events::HookInstance;
use crate::extension_manager::ExtensionManager;

pub struct HookReceiver(pub(super) mpsc::UnboundedReceiver<HookInstance>);
impl HookReceiver {
    pub(crate) fn start(
        mut self,
        extensions: ExtensionManager,
        database: DatabaseContext,
        agents: AgentCoordinator,
    ) {
        tokio::spawn(async move {
            while let Some(event) = self.0.recv().await {
                if event.trace.is_too_deep() {
                    log_warn!(Module::Extensions, "Hook loop terminated.");
                    continue;
                }
                if let Err(e) = Self::dispatch_hook(&event, &database, &extensions).await {
                    log_error!(Module::Extensions, "Failed to execute hooks: {:?}.", e);
                }
                // Agents go after the extensions, so their cues see what the
                // extensions changed and can skip work an extension already did.
                // A hook aimed at one extension (its launch) is not an event agents watch.
                if event.extension_id.is_none() {
                    agents
                        .fire(event.hook.kind(), event.hook.arguments(), &event.trace)
                        .await;
                }
            }
        });
    }

    async fn dispatch_hook(
        event: &HookInstance,
        database: &DatabaseContext,
        extensions: &ExtensionManager,
    ) -> Result<(), CommandError> {
        let matched_extensions = match event.extension_id {
            // A launch queued by a restart runs only if the hook is still
            // on when its turn comes.
            Some(extension_id) => {
                if !extensions.claim_launch(extension_id).await? {
                    return Ok(());
                }
                vec![
                    database
                        .send(GetExtensionCommand {
                            query: GetExtensionQuery::Id(extension_id),
                        })
                        .await?,
                ]
            }
            None => {
                database
                    .send(GetExtensionsWithHookCommand {
                        hook: event.hook.kind(),
                    })
                    .await?
            }
        };

        // Each extension runs and logs on its own, so one failing doesn't
        // stop the rest. The manager starts each from the extension as it
        // is installed when its turn comes, not from these rows.
        let function_name = event.hook.function_name();
        for extension in matched_extensions {
            if event.trace.is_from_extension(&extension.script_identifier) {
                continue;
            }
            let identifier = extension.script_identifier;
            let hook = event.hook.clone();
            let trace = event.trace.clone();
            // The hook is checked again as it is called: the user may have
            // switched it off while earlier extensions ran.
            let result = extensions
                .hook(extension.id, event.hook.kind(), move |sandbox| {
                    sandbox.traced(trace, move |sandbox| hook.evaluate(sandbox))
                })
                .await;
            match result {
                Ok(None) => {}
                Ok(Some(())) => log_info!(
                    Module::Extensions,
                    "{identifier}: {function_name} succeeded."
                ),
                Err(e) => log_error!(
                    Module::Extensions,
                    "{identifier}: {function_name} failed: {e}"
                ),
            }
        }

        Ok(())
    }
}
