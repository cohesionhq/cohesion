//! What an engine's sandboxes reach outside the interpreter through,
//! attached to each when it is built.

use std::sync::Arc;

use crate::actor_access::ActorHost;
use crate::service_access::ServiceHost;

/// The hosts every sandbox of one engine but the isolated worker's is
/// served by; cheap to clone, and shared by all the engine's clones.
#[derive(Clone, Default)]
pub(crate) struct Hosts {
    pub(crate) services: Arc<ServiceHost>,
    pub(crate) actors: Arc<ActorHost>,
}
