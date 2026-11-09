//! Requests about connectors: the services `oauth` extensions reach, the
//! accounts added through them, and the resources `connector` extensions import
//! from those accounts.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

use crate::model::{AccountHost, Connector, ConnectorAccount, ResourceSort};

/// Lists every service an `oauth`-granted extension reaches: those the user
/// can add an account at.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListConnectorServicesRequest;

/// One service an `oauth` extension reaches, as its `serviceDescription()`
/// describes it.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ConnectorService {
    pub extension_id: i64,
    /// The extension's name.
    pub name: String,
    /// The service's name; the extension's name when `error` is set.
    pub title: String,
    pub unit: String,
    /// The extension's icon, a `data:image/…` or `https://` URL.
    pub icon: Option<String>,
    /// Why the extension couldn't describe its service.
    pub error: Option<String>,
    /// What to do on the sign-in page, when the service doesn't end its
    /// sign-in by showing a code.
    pub sign_in_hint: Option<String>,
}

/// Asks `oauth` extension `extension_id` where the user signs in.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct BeginConnectorAuthenticationRequest {
    pub extension_id: i64,
}

/// The sign-in page to open for the user.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ConnectorAuthentication {
    pub url: String,
}

/// Exchanges the `code` the sign-in page gave the user for a credential and
/// adds (or re-authenticates) the account it belongs to.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct CompleteConnectorAuthenticationRequest {
    pub extension_id: i64,
    pub code: String,
}

/// Lists every connector account, by description and then connector title.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListConnectorAccountsRequest;

/// Removes connector account `account_id` and every resource imported from
/// it.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DeleteConnectorAccountRequest {
    pub account_id: i64,
}

/// Moves every resource of connector account `from_account_id` to account
/// `to_account_id` of the same connector, in every workspace. Where both
/// hold the same item, the target's copy stays.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct MigrateResourcesRequest {
    pub from_account_id: i64,
    pub to_account_id: i64,
}

/// A connector account without its credential, safe to send to clients.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RedactedConnectorAccount {
    pub id: i64,
    /// What serves the account.
    pub host: AccountHost,
    /// The connector the account is one of: its name, icon, unit and kinds.
    pub connector: Connector,
    /// What describes the account for people: its name or email.
    pub description: String,
    /// When refreshing the account's credential last failed, in Unix
    /// seconds; absent while it works. The account needs signing in again.
    pub credential_failed_at: Option<i64>,
    /// When the account was added, in Unix seconds.
    pub created_at: i64,
    /// Whether the account's items can be browsed and imported: its
    /// extension is granted `connector`. An orphaned account's can't.
    pub importable: bool,
}
impl RedactedConnectorAccount {
    /// `account`, one of `connector`, without its credential; `importable`
    /// says whether its items can be imported.
    pub fn new(account: ConnectorAccount, connector: Connector, importable: bool) -> Self {
        Self {
            id: account.id,
            host: account.host,
            connector,
            description: account.description,
            credential_failed_at: account.credential_failed_at,
            created_at: account.created_at,
            importable,
        }
    }
}

/// Lists the organization units of connector account `account_id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListConnectorOrganizationUnitsRequest {
    pub account_id: i64,
}

/// Lists the items of kind `kind_id` connector account `account_id` offers
/// for import into workspace `workspace_id`, optionally narrowed to an
/// organization unit or a search.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct SearchConnectorResourcesRequest {
    pub account_id: i64,
    pub workspace_id: i64,
    pub kind_id: i64,
    pub organization_unit_id: Option<String>,
    pub search: Option<String>,
}

/// One item a connector account offers for import.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ImportableResource {
    pub identifier: String,
    pub title: String,
    /// When the item last changed, in Unix seconds; absent when the service
    /// doesn't say.
    pub last_updated: Option<i64>,
    /// Whether the item is already a resource in the workspace.
    pub imported: bool,
}

/// Imports item `identifier` of connector account `account_id` into
/// workspace `workspace_id`, or refreshes it if it is already there.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ImportResourceRequest {
    pub account_id: i64,
    pub workspace_id: i64,
    pub identifier: String,
}

/// Lists the resources in workspace `workspace_id` in `sort`'s order,
/// optionally only those of `kind` or whose title contains `search`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct ListResourcesRequest {
    pub workspace_id: i64,
    pub kind: Option<String>,
    pub search: Option<String>,
    pub sort: ResourceSort,
    /// Reverses `sort`; resources without a value for it still come last.
    pub descending: bool,
}

/// Lists the kinds of resource shown in workspace `workspace_id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListWorkspaceResourceKindsRequest {
    pub workspace_id: i64,
}

/// Shows kind `kind_id` in workspace `workspace_id`, or hides it there.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetResourceKindShownRequest {
    pub workspace_id: i64,
    pub kind_id: i64,
    pub shown: bool,
}

/// Puts the kinds shown in workspace `workspace_id` in the order of
/// `kind_ids`, first to last.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ReorderWorkspaceResourceKindsRequest {
    pub workspace_id: i64,
    /// Every kind shown in the workspace, in its new order.
    pub kind_ids: Vec<i64>,
}

/// Fetches resource `resource_id` in workspace `workspace_id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct GetResourceRequest {
    pub workspace_id: i64,
    pub resource_id: i64,
}

/// Asks the connector of resource `resource_id` in workspace `workspace_id`
/// for the CohesionUI interface it shows the resource with.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RenderResourceRequest {
    pub workspace_id: i64,
    pub resource_id: i64,
}

/// Removes resource `resource_id` from workspace `workspace_id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DeleteResourceRequest {
    pub workspace_id: i64,
    pub resource_id: i64,
}

/// Refreshes every resource in workspace `workspace_id` from its service now.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RefreshResourcesRequest {
    pub workspace_id: i64,
}

/// What a refresh pass did.
#[derive(Serialize, Deserialize, Debug, Clone, Default, TsWire)]
pub struct ResourceRefreshSummary {
    /// Resources fetched from their services.
    pub checked: i64,
    /// Resources whose contents changed.
    pub updated: i64,
    /// Resources that couldn't be fetched.
    pub failed: i64,
}
