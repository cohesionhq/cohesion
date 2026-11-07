//! A hold on one persistent scope's worker.

use super::script_engine::run_on;
use super::script_worker::ScriptWorker;
use crate::sandbox::Sandbox;
use crate::script_error::ScriptError;

/// A running scope, as [`ScriptEngine::scope`](super::ScriptEngine::scope)
/// found it or [`spawn_scope`](super::ScriptEngine::spawn_scope) started it.
/// Shutting the scope down stops new lookups at once, but a handle keeps its
/// worker alive, so a job sent through one still runs there; the scope's
/// timers stop once no handle is left.
#[derive(Clone)]
pub struct ScopeHandle {
    pub(super) worker: ScriptWorker,
}

impl ScopeHandle {
    /// Runs `job` in the scope's sandbox, after the jobs queued before it.
    pub async fn run<T: Send + 'static>(
        &self,
        job: impl FnOnce(&Sandbox) -> Result<T, ScriptError> + Send + 'static,
    ) -> Result<T, ScriptError> {
        run_on(&self.worker, job).await
    }
}
