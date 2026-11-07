//! How scripts start actor sessions and talk to them, through the daemon above
//! this crate, which calls the actors' clients.

use std::sync::{Arc, OnceLock};
use std::time::Instant;

use cohesion_core::model::{ActorSession, ActorSessionMessage, ActorUserContent};
use cohesion_core::protocol::{ActorUsage, ServerError, StartActorSessionRequest};
use rquickjs::{Ctx, Exception, JsLifetime};

/// Starts, talks to, stops and resolves the sessions of the actors clients offer, as the
/// user would from a client. Implemented above this crate, where the
/// clients are called, and blocking, since scripts run on their own
/// threads. Each call stops waiting when the script's `deadline` passes,
/// without undoing what the daemon already did.
pub trait ActorAccess: Send + Sync {
    /// Starts the session `request` describes, answering it as recorded.
    fn start(
        &self,
        request: StartActorSessionRequest,
        deadline: Instant,
    ) -> Result<ActorSession, ServerError>;
    /// Says `content` in session `session_id`, answering the message recorded.
    fn send(
        &self,
        session_id: i64,
        content: ActorUserContent,
        deadline: Instant,
    ) -> Result<ActorSessionMessage, ServerError>;
    /// Stops what session `session_id` is doing, answering it as it stands.
    fn stop(&self, session_id: i64, deadline: Instant) -> Result<ActorSession, ServerError>;
    /// Resolves session `session_id` for the user, stopping it first if it is
    /// going on, answering it as it stands.
    fn resolve(&self, session_id: i64, deadline: Instant) -> Result<ActorSession, ServerError>;
    /// What client `client_id`'s actor `identifier` last reported of its
    /// usage while the client is connected; empty when it reported none.
    /// Reaches no client.
    fn usage(&self, client_id: i64, identifier: &str) -> ActorUsage;
}

/// What every sandbox of one engine reaches actors through, attached to
/// each when it is built; whether a script may use it is its mounts'
/// say. The access is set once, after the engine is built, since what
/// implements it is built after the engine.
#[derive(Default)]
pub(crate) struct ActorHost(pub(crate) OnceLock<Arc<dyn ActorAccess>>);

/// The actor host of the sandbox's engine, stored as context userdata for
/// the sandbox's life.
#[derive(JsLifetime)]
pub(crate) struct CurrentActors(pub Arc<ActorHost>);

/// The sandbox's actor access; throws when its engine has none.
pub(crate) fn current_actors(ctx: &Ctx<'_>) -> rquickjs::Result<Arc<dyn ActorAccess>> {
    ctx.userdata::<CurrentActors>()
        .and_then(|current| current.0.0.get().cloned())
        .ok_or_else(|| Exception::throw_message(ctx, "Actors can't be reached from this script."))
}
