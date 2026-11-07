//! How scripts reach installed services' `serviceRequest`, through the
//! daemon above this crate, and what stops a service from waiting on itself.

use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use cohesion_core::protocol::ServerError;
use rquickjs::JsLifetime;

/// One call a script makes through `services.use`.
pub struct ServiceCall {
    /// The service's extension.
    pub extension_id: i64,
    /// The declared function, which `serviceRequest` receives as its `name`.
    pub name: String,
    /// Its one argument; `null` when it takes none or the script passed none.
    pub args: serde_json::Value,
    /// Whether the script's database access is read-only, as in a
    /// turn a cue started or an extension without `cohesion:write`, so it may
    /// only call functions declared `readOnly`.
    pub read_only: bool,
    /// When the script's own time runs out; the call gives up then, so a
    /// slow service can't hold the script past it.
    pub deadline: Instant,
}

/// Calls a service extension's `serviceRequest` for a script. Implemented
/// above this crate, where extensions are called, and blocking, since
/// scripts run on their own threads. The implementation checks the call
/// against what the service declares, its read-only rule included:
/// `services.use` offers every declared function, and a script can hold
/// on to one after the service changes.
pub trait ServiceAccess: Send + Sync {
    /// What the service's `serviceRequest(name, args)` returns.
    fn call(&self, call: ServiceCall) -> Result<serde_json::Value, ServerError>;
}

/// What every sandbox of one engine reaches services through, attached to
/// each when it is built. The access is set once, after the engine is
/// built, since what implements it needs the engine itself.
#[derive(Default)]
pub(crate) struct ServiceHost {
    pub(crate) access: OnceLock<Arc<dyn ServiceAccess>>,
    pub(crate) waits: ServiceWaits,
}

/// The service host of the sandbox's engine, stored as context userdata
/// for the sandbox's life.
#[derive(JsLifetime)]
pub(crate) struct CurrentServices(pub Arc<ServiceHost>);

/// Which extensions' scopes are blocked on a service call to which, by
/// script identifier. An extension's scope runs one job at a time, so a
/// call into a scope that is waiting, however indirectly, on the caller
/// would never return; such a call is refused instead. Kept acyclic by
/// that refusal.
#[derive(Default)]
pub(crate) struct ServiceWaits(Mutex<Vec<(String, String)>>);

impl ServiceWaits {
    /// Records that `caller`'s scope waits on `target`'s until the answer
    /// is dropped; `None` when `target` is `caller` or already waits on it.
    #[must_use = "the wait is erased when the answer is dropped"]
    pub(crate) fn wait(&self, caller: &str, target: &str) -> Option<ServiceWait<'_>> {
        let mut waits = self.0.lock().expect("mutex is not poisoned");
        let mut pending = vec![target];
        while let Some(at) = pending.pop() {
            if at == caller {
                return None;
            }
            pending.extend(
                waits
                    .iter()
                    .filter(|(waiter, _)| waiter == at)
                    .map(|(_, awaited)| awaited.as_str()),
            );
        }
        waits.push((caller.to_string(), target.to_string()));
        Some(ServiceWait {
            waits: self,
            edge: (caller.to_string(), target.to_string()),
        })
    }
}

/// One scope's wait on another, erased when dropped.
#[must_use = "the wait is erased when this is dropped"]
pub(crate) struct ServiceWait<'a> {
    waits: &'a ServiceWaits,
    edge: (String, String),
}

impl Drop for ServiceWait<'_> {
    fn drop(&mut self) {
        let mut waits = self.waits.0.lock().expect("mutex is not poisoned");
        if let Some(index) = waits.iter().position(|edge| *edge == self.edge) {
            waits.swap_remove(index);
        }
    }
}
