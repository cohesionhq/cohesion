//! How scripts reach actors: the engine's [`ActorAccess`], running
//! [`ActorSessions`] on the runtime from the script's own thread, for no longer
//! than the script has left, and reading the usage [`CapabilityClients`]
//! hold.

use std::future::Future;
use std::time::Instant;

use cohesion_core::model::{ActorSession, ActorSessionMessage, ActorUserContent};
use cohesion_core::protocol::{ActorUsage, ServerError, StartActorSessionRequest};
use cohesion_js::ActorAccess;
use tokio::runtime::Handle;
use tokio::time::timeout_at;

use crate::actor_sessions::ActorSessions;
use crate::capability_clients::CapabilityClients;

/// Starts, talks to, stops and resolves actor sessions for scripts, which run on their
/// own threads, by blocking on the runtime.
pub(crate) struct ScriptActors {
    sessions: ActorSessions,
    clients: CapabilityClients,
    runtime: Handle,
}

impl ScriptActors {
    /// Must be called inside the tokio runtime, which runs the calls.
    pub(crate) fn new(sessions: ActorSessions, clients: CapabilityClients) -> Self {
        Self {
            sessions,
            clients,
            runtime: Handle::current(),
        }
    }

    /// Blocks the calling thread, which is outside the runtime, on `call`,
    /// giving up when `deadline` passes. The call runs as a session of its
    /// own, so a script that gives up leaves it to finish: dropped halfway,
    /// it could leave a session recorded that its actor never heard of.
    fn block_until<T: Send + 'static>(
        &self,
        deadline: Instant,
        call: impl Future<Output = Result<T, ServerError>> + Send + 'static,
    ) -> Result<T, ServerError> {
        let call = self.runtime.spawn(call);
        let waited = self
            .runtime
            .block_on(async { timeout_at(tokio::time::Instant::from_std(deadline), call).await });
        match waited {
            Ok(Ok(result)) => result,
            Ok(Err(_panicked)) => Err(ServerError::internal()),
            Err(_elapsed) => Err(ServerError::upstream(
                "The actor didn't answer before this script's time ran out.",
            )),
        }
    }
}

impl ActorAccess for ScriptActors {
    fn start(
        &self,
        request: StartActorSessionRequest,
        deadline: Instant,
    ) -> Result<ActorSession, ServerError> {
        let sessions = self.sessions.clone();
        self.block_until(deadline, async move { sessions.start(request).await })
    }

    fn send(
        &self,
        session_id: i64,
        content: ActorUserContent,
        deadline: Instant,
    ) -> Result<ActorSessionMessage, ServerError> {
        let sessions = self.sessions.clone();
        self.block_until(deadline, async move {
            sessions.send(session_id, content, Vec::new()).await
        })
    }

    fn stop(&self, session_id: i64, deadline: Instant) -> Result<ActorSession, ServerError> {
        let sessions = self.sessions.clone();
        self.block_until(deadline, async move { sessions.stop(session_id).await })
    }

    fn resolve(&self, session_id: i64, deadline: Instant) -> Result<ActorSession, ServerError> {
        let sessions = self.sessions.clone();
        self.block_until(deadline, async move { sessions.resolve(session_id).await })
    }

    fn usage(&self, client_id: i64, identifier: &str) -> ActorUsage {
        self.clients.actor_usage(client_id, identifier)
    }
}
