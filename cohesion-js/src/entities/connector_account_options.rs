//! Options objects that `ConnectorAccount` methods take.

use cohesion_macros::JsOptions;

use crate::entities::workspace::JsWorkspace;

/// Options for `ConnectorAccount.resources()`.
#[derive(JsOptions, Default)]
pub struct AccountResourceListOptions {
    /// Only the resources imported into this workspace; every workspace when
    /// left out.
    pub workspace: Option<JsWorkspace>,
}

/// Filters for `ConnectorAccount.items()`.
#[derive(JsOptions, Default)]
pub struct ConnectorItemFilters {
    /// Only items in this organization unit, by the `id` from
    /// `organizationUnits()`.
    pub organization_unit_id: Option<String>,
    /// Only items matching this text, as the service searches.
    pub search: Option<String>,
}
