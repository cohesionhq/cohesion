//! Connectors: calling an extension's sign-in (`oauth`) and import
//! (`connector`) functions on its scoped worker, fetching account credentials
//! through the database's credential manager, the PKCE verifier between a
//! sign-in's two calls, the loop that refreshes imported resources from their
//! services, and the access agent scripts get to all of it.

mod agent_connectors;
mod connector_calls;
mod credential_refresh;
mod pending_sign_ins;
mod resource_refresh;

pub(crate) use agent_connectors::AgentConnectors;
pub(crate) use connector_calls::{
    AuthenticationUrlFn, ServiceDescriptionFn, account_items, account_organization_units,
    account_removed, call_connector, complete_connector_authentication, fetch_resource,
};
pub(crate) use credential_refresh::fresh_credential;
pub(crate) use pending_sign_ins::PendingSignIns;
pub(crate) use resource_refresh::refresh_resources;

#[cfg(test)]
mod tests;
