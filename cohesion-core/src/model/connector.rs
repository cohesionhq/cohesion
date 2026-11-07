//! The connector contract: what the global functions of an `oauth` or
//! `connector` extension take and return.

use std::collections::BTreeMap;

use cohesion_macros::{Fields, Model, TsInterface, TsWire};
use serde::{Deserialize, Serialize};

use super::resource::{MetadataKey, ResourceContentKind};

/// What `serviceDescription()` returns: the service a connector reaches,
/// which every account of the connector shares.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire, TsInterface)]
pub struct ServiceDescription {
    /// The service's name, such as `"Notion"`.
    pub title: String,
    /// What the service calls an account, such as `"Workspace"`.
    pub unit: String,
    /// The kinds of item `getResource` returns, so scripts and the assistant
    /// know what a resource of each kind holds.
    #[serde(default)]
    pub kinds: Vec<ResourceKindDescription>,
    /// The metadata keys `getResource` and `listResources` fill.
    #[serde(rename = "metadataKeys", default)]
    pub metadata_keys: Vec<MetadataKey>,
    /// What to do on the sign-in page, shown in place of "paste the code it
    /// shows" when the service ends its sign-in some other way, such as on a
    /// page whose address holds the code: "Sign in, then paste the address
    /// of the page that won't load."
    #[serde(
        rename = "signInHint",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub sign_in_hint: Option<String>,
}

/// A connector as an extension's identifier, version, icon and
/// `serviceDescription()` make it: one definition, whichever extension
/// serves it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire)]
pub struct ConnectorDefinition {
    /// Identifies the connector to every extension serving it: its script
    /// identifier.
    pub identifier: String,
    /// A `data:image/…` or `https://` URL.
    #[serde(default)]
    pub icon: Option<String>,
    /// The definition's version: a host serving a higher one rewrites it
    /// for every account; a lower or equal one leaves it. An extension's is
    /// its manifest's.
    pub version: i64,
    #[serde(flatten)]
    pub description: ServiceDescription,
}

/// A connector's stored definition, which its accounts share.
#[derive(Clone, Debug, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "connectors", backup = Included)]
pub struct Connector {
    /// Unique, stable identifier.
    pub id: i64,
    /// Identifies the connector to its hosts, unique ignoring case.
    pub identifier: String,
    pub version: i64,
    /// The service's name, such as `"Notion"`.
    pub title: String,
    /// A `data:image/…` or `https://` URL.
    pub icon: Option<String>,
    /// What the service calls an account, such as `"Workspace"` or `"Device"`.
    pub unit: String,
    /// The kinds of item its resources can be, as its definition last
    /// declared them.
    #[model(json, expr = Connector::KINDS_SQL)]
    pub kinds: Vec<ResourceKind>,
    /// The metadata keys its resources may carry.
    #[model(json)]
    pub metadata_keys: Vec<MetadataKey>,
    /// When the connector was first served, in Unix seconds.
    pub created_at: i64,
}

impl Connector {
    /// Its kinds, by id, as JSON.
    pub const KINDS_SQL: &str = "(SELECT json_group_array(json_object('id', k.id, \
        'connector_id', k.connector_id, 'identifier', k.identifier, 'title', k.title, \
        'plural', k.plural, 'contents', k.contents)) \
        FROM (SELECT * FROM resource_kinds k WHERE k.connector_id = connectors.id ORDER BY k.id) k)";
}

/// One kind of item a connector imports, as `serviceDescription()` lists
/// them in `kinds`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire, TsInterface)]
pub struct ResourceKindDescription {
    /// The short key `getResource` and `listResources` return as
    /// `kindIdentifier`, such as `"page"`. An item of a kind the connector
    /// doesn't declare can't be imported.
    pub identifier: String,
    /// The kind's name for people, such as `"Page"`.
    pub title: String,
    /// The name for several, such as `"Pages"`, which titles their list.
    pub plural: String,
    /// One or two sentences saying what a resource's `contents` holds, for
    /// scripts and the assistant, such as `"Markdown of the page's blocks"`
    /// or `"JSON { rows: [{ item: string, amount: number }] }"`.
    pub contents: String,
}

/// A kind of item a connector imports, as its definition last declared it.
/// A definition that drops a kind removes it, and its resources with it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Model, Fields, TsWire, TsInterface)]
#[model(table = "resource_kinds", backup = Included)]
pub struct ResourceKind {
    /// Unique, stable identifier.
    pub id: i64,
    /// Id of the connector that declares it.
    pub connector_id: i64,
    /// The short key, such as `"page"`; a resource's `kind` is its
    /// connector's identifier and this.
    pub identifier: String,
    /// The kind's name for people, such as `"Page"`.
    pub title: String,
    /// The name for several, such as `"Pages"`.
    pub plural: String,
    /// What a resource's `contents` holds.
    pub contents: String,
}

/// What `authenticationURL(pkce)` is passed: the PKCE (RFC 7636) challenge
/// for this sign-in, to send as `code_challenge` and
/// `code_challenge_method` in the sign-in page's URL.
/// `completeAuthentication(code, verifier)` is then passed its verifier.
#[derive(Serialize, Deserialize, Clone, Debug, TsInterface)]
pub struct PkceChallenge {
    /// The verifier's SHA-256, base64url-encoded without padding.
    pub challenge: String,
    /// How `challenge` was derived: always `"S256"`.
    pub method: String,
}

/// An account's credential. `completeAuthentication(code)` and
/// `completeRefresh(credential)` return one; every function that reaches the
/// service is passed the stored one, and `Account.credential()` returns
/// it.
#[derive(Serialize, Deserialize, Clone, Debug, TsInterface)]
pub struct Credential {
    /// The access token sent to the service.
    pub authorization: String,
    /// A token to get a new access token with, if the service issues one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh: Option<String>,
    /// When `authorization` expires, in Unix **seconds**. Within 60 seconds
    /// of it the daemon calls `completeRefresh(credential)` and stores the
    /// result before the credential is used. Leave it out if it never
    /// expires.
    #[serde(rename = "expiresAt", default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
}

/// How close to expiry, in seconds, a credential is refreshed.
const REFRESH_MARGIN_SECONDS: i64 = 60;

impl Credential {
    /// Whether the credential expires within a minute of `now` (Unix
    /// seconds), so `completeRefresh` should replace it before it is used.
    /// One without an expiry never does.
    pub fn needs_refresh(&self, now: i64) -> bool {
        self.expires_at
            .is_some_and(|expires_at| expires_at - now <= REFRESH_MARGIN_SECONDS)
    }
}

/// What `getAccountDescription(credential)` returns: which account a
/// credential belongs to. `accountRemoved(account)` is passed one too,
/// naming the account that was removed.
#[derive(Serialize, Deserialize, Clone, Debug, TsInterface)]
pub struct AccountDescription {
    /// The account's stable identifier at the service. Adding an account
    /// with the same identifier again replaces its credential.
    pub identifier: String,
    /// Describes the account for people, such as its name or email.
    pub description: String,
}

/// A part of an account that resources are grouped into, such as a team space
/// or a project, as `getOrganizationUnits(credential)` returns them.
#[derive(Serialize, Deserialize, Clone, Debug, TsWire, TsInterface)]
pub struct OrganizationUnit {
    /// Identifies the unit to `listResources` as `organizationUnitId`.
    pub id: String,
    /// The unit's name, shown to people.
    pub display: String,
}

/// What the daemon passes `listResources(filters, credential)`: the user's
/// choices while browsing. Either may be left out.
#[derive(Serialize, Deserialize, Clone, Debug, Default, TsInterface)]
pub struct ConnectorResourceFilters {
    /// Only resources in this organization unit.
    #[serde(
        rename = "organizationUnitId",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub organization_unit_id: Option<String>,
    /// Text the user searched for; match it against titles at least.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
}

/// One item `listResources(filters, credential)` returns, enough to list it for
/// import.
#[derive(Serialize, Deserialize, Clone, Debug, TsInterface)]
pub struct ConnectorResourcePreview {
    /// The item's stable identifier at the service; `getResource` takes it.
    pub identifier: String,
    /// When the item last changed, in Unix **seconds**; `null` when the
    /// service has no modified date or the listing can't know it cheaply.
    /// A refresh fetches an item whose preview has none.
    #[serde(rename = "lastUpdated", default)]
    pub last_updated: Option<i64>,
    /// Display name.
    pub title: String,
    /// The item's kind, one of the `kinds` the connector describes, such as
    /// `"page"`, as `getResource` returns it.
    #[serde(rename = "kindIdentifier")]
    pub kind_identifier: String,
    /// What the item's metadata says, from the keys the connector declares,
    /// so a list can be filtered without fetching each item.
    #[serde(default)]
    pub metadata: BTreeMap<MetadataKey, String>,
}

/// What `getResource(identifier, credential)` returns: the whole item, as it is
/// imported and refreshed.
#[derive(Serialize, Deserialize, Clone, Debug, TsInterface)]
pub struct ConnectorResource {
    /// The item's stable identifier at the service.
    pub identifier: String,
    /// When the item last changed, in Unix **seconds**. A refresh rewrites
    /// the resource only when this or its contents change.
    #[serde(rename = "lastUpdated")]
    pub last_updated: i64,
    /// Display name.
    pub title: String,
    /// The item's contents: Markdown, plain text, or JSON text.
    pub contents: String,
    /// Whether `contents` is Markdown, plain text or JSON text.
    #[serde(rename = "contentKind")]
    pub content_kind: ResourceContentKind,
    /// A short, stable key for the item's kind, one of the `kinds` the
    /// connector describes, such as `"page"`. The resource's `kind` is this
    /// prefixed with the connector's identifier, as
    /// `"com.example.notion.page"`.
    #[serde(rename = "kindIdentifier")]
    pub kind_identifier: String,
    /// What the item's metadata says, from the keys the connector declares,
    /// such as `{ dateCreated: "2026-10-24T09:30:00Z", author: "Ada" }`. It
    /// replaces what the resource carried.
    #[serde(default)]
    pub metadata: BTreeMap<MetadataKey, String>,
}

#[cfg(test)]
mod tests;
