//! How scripts reach services: the engine's [`ServiceAccess`], running the
//! manager's service call on the runtime from the script's own thread, for
//! no longer than the script has left.

use std::sync::{Arc, Weak};

use cohesion_core::protocol::ServerError;
use cohesion_js::{ServiceAccess, ServiceCall};
use tokio::runtime::Handle;
use tokio::time::{Instant, timeout_at};

use super::service_call::ServiceCaller;
use super::{ExtensionManager, Inner};

/// Calls service extensions for scripts, which run on their own threads,
/// by blocking on the runtime. Holds the manager weakly: the manager's
/// engine holds this.
pub(crate) struct ScriptServices {
    extensions: Weak<Inner>,
    runtime: Handle,
}

impl ScriptServices {
    /// Must be called inside the tokio runtime, which runs the calls.
    pub(crate) fn new(extensions: &ExtensionManager) -> Self {
        Self {
            extensions: Arc::downgrade(&extensions.inner),
            runtime: Handle::current(),
        }
    }
}

impl ServiceAccess for ScriptServices {
    fn call(&self, call: ServiceCall) -> Result<serde_json::Value, ServerError> {
        let inner = self
            .extensions
            .upgrade()
            .ok_or_else(ServerError::internal)?;
        let extensions = ExtensionManager { inner };
        let called = extensions.call_service(
            call.extension_id,
            ServiceCaller::Script {
                read_only: call.read_only,
            },
            call.name,
            call.args,
        );
        // Dropping the call stops the wait, not the service's job.
        self.runtime
            .block_on(async { timeout_at(Instant::from_std(call.deadline), called).await })
            .unwrap_or_else(|_| {
                Err(ServerError::upstream(
                    "The service didn't answer before this script's time ran out.",
                ))
            })
    }
}
