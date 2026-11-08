//! How [`CohesionContext`](crate::CohesionContext) carries out each request
//! that isn't a plain database command. Files are by domain, not by `via`
//! kind: the kind only keeps the blanket `DbRequest` impl in
//! `database_routes` from overlapping the hand-written ones, so a domain's
//! `Local`, `Script` and `Intelligence` routes sit together.
//!
//! - `database_routes`: every `DbRequest`, through its command.
//! - `client_routes`: the daemon itself, pairing and clients, some of it
//!   `FromClient`, on [`ClientRequest`](crate::ClientRequest).
//! - `extension_routes`: installing, granting, registries, updates and
//!   calling a `service` extension.
//! - `connector_routes`: accounts, importing and refreshing resources on
//!   `oauth` and `connector` extensions.
//! - `article_routes` and `feed_routes`: re-fetching articles and following feeds,
//!   which fetch from the web.
//! - `agent_routes`: requests that touch a running agent.
//! - `actor_routes`: requests that reach an actor through its client.
//! - `catalog_routes`: refreshing the model catalog.
//! - `script_routes`: running scripts and their documentation.
//! - `mermaid_routes`: drawing a Mermaid diagram.
//! - `transfer_routes`: exporting the database, and importing one in its place.

mod actor_routes;
mod agent_routes;
mod article_routes;
mod catalog_routes;
mod client_routes;
mod connector_routes;
mod database_routes;
mod extension_routes;
mod feed_routes;
mod mermaid_routes;
mod script_routes;
mod transfer_routes;

#[cfg(test)]
mod tests;
