//! The facade cohesiond builds on: everything from cohesion-core at the root,
//! persistence under `db`, and the [`CohesionContext`] that ties them
//! together with the script engine, the agents and the extension manager,
//! the one path to an extension's scoped worker, which installs, restarts
//! and calls extensions and asks registries daily for updates to them.
//! `routes` holds how the context carries out each kind of request; `hooks`
//! runs extension hooks off database events, and `table_formulas`
//! computes table formulas' outputs off the events that outdate them; `connectors` calls `oauth` and
//! `connector` extensions, refreshes imported resources and gives agent scripts
//! their connector access; `feeds` reads followed feeds, on request and
//! hourly; `periodic_cleanup` hourly removes idle model threads, old
//! security activity and the clients that stopped connecting; [`Security`] records refused pairings and when and
//! from where each client last connected; every transport dispatches a [`ClientRequest`], so a route
//! can see which client asked; [`CapabilityClients`] keeps the sessions of
//! the clients serving actors and the calls the daemon makes into them, and
//! keeps the database rows for what each client offers in step with it,
//! through `CapabilityStore` (`capability_store.rs`). The agents get the
//! credentials of the accounts endpoints sign in with through
//! `EndpointRelay` (`endpoint_relay.rs`). [`ActorSessions`] hands
//! sessions to the actors clients offer and keeps an activity for each while it
//! goes on, and scripts reach it through `ScriptActors` (`script_actors.rs`). [`web_bundle`] is the web UI
//! the daemon was built with, which it hands clients, and [`web_bundle_hash`] identifies it.
//! [`PendingImport`] holds an import a client finished until cohesiond
//! restarts onto it.

pub use cohesion_core::*;

/// SQLite persistence: the container, the command channel, and every command.
pub mod db {
    pub use cohesion_db::*;
}
mod actor_sessions;
mod calendars;
mod capability_clients;
mod capability_store;
mod client_request;
mod cohesion_context;
mod connectors;
mod endpoint_relay;
mod extension_icon;
mod extension_manager;
mod feeds;
mod hooks;
mod hostname;
mod pending_import;
mod periodic_cleanup;
mod presence;
mod routes;
mod script_actors;
mod security;
mod table_formulas;
mod web_bundle;

pub use actor_sessions::{ActorSessions, Reported};
pub use capability_clients::{CapabilityClients, ClientCallError, ClientSessionHandle, OfferError};
pub use client_request::ClientRequest;
pub use cohesion_context::CohesionContext;
pub use pending_import::PendingImport;
pub use presence::{Presence, PresenceHandle};
pub use security::Security;
pub use web_bundle::{web_bundle, web_bundle_hash};
