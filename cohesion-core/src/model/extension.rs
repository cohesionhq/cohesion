use std::collections::BTreeMap;

use cohesion_macros::{Model, TextEnum, TsInterface, TsWire};
use serde::{Deserialize, Serialize};

use crate::model::HookKind;

#[derive(Clone, Serialize, Deserialize, Model)]
#[model(table = "extensions", backup = Included)]
pub struct Extension {
    pub id: i64,

    pub name: String,
    /// The manifest's version; a registry entry above it is an update.
    pub version: i64,
    pub script: String,
    pub script_identifier: String,
    pub description: String,
    pub author: String,
    /// A `data:image/…` or `https://` URL, as the manifest gave it if the
    /// daemon accepted it.
    pub icon: Option<String>,
    /// Hook evaluation order: lower runs first, ties by `id`, so an
    /// extension added later runs after existing ones at the same priority.
    pub priority: i64,
    /// The last version a registry offered as an update that the daemon
    /// raised `extension:update` for, so each offered version is announced
    /// once.
    pub announced_version: Option<i64>,
}

#[derive(Clone, Serialize, Deserialize, Model, TsWire)]
#[model(table = "extension_permissions", backup = Included)]
pub struct ExtensionPermission {
    pub id: i64,
    pub extension_id: i64,
    pub permission: ExtensionPermissionKind,
    pub enabled: bool,
}

#[derive(Clone, Serialize, Deserialize, Model, TsWire)]
#[model(table = "extension_hooks", backup = Included)]
pub struct ExtensionHook {
    pub id: i64,
    pub extension_id: i64,
    pub hook: HookKind,
    pub enabled: bool,
}

/// The service an extension declares in its manifest: one row per
/// extension, its functions in `extension_service_functions`.
#[derive(Clone, Serialize, Deserialize, Model, TsWire)]
#[model(table = "extension_services", backup = Included)]
pub struct ExtensionService {
    pub id: i64,
    pub extension_id: i64,
    pub description: String,
}

/// One function an extension's service declares.
#[derive(Clone, Serialize, Deserialize, Model, TsWire)]
#[model(table = "extension_service_functions", backup = Included)]
pub struct ExtensionServiceFunction {
    pub id: i64,
    pub service_id: i64,
    /// What `serviceRequest` receives as its `name`.
    pub name: String,
    pub description: String,
    /// The TypeScript type of its one argument; `None` when it takes none.
    pub args: Option<String>,
    /// The TypeScript type of its result; `None` for `unknown`.
    pub returns: Option<String>,
    /// Whether it only reads, so callers with read-only access may call it.
    pub read_only: bool,
}

/// An installed service that agents, scripts and extensions granted
/// `services` may call: its extension holds `service` (not
/// `service:isolated`) enabled and declared a `service` block.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, TsWire)]
pub struct InstalledService {
    pub extension_id: i64,
    pub script_identifier: String,
    /// The extension's display name.
    pub name: String,
    pub description: String,
    /// By name.
    pub functions: Vec<InstalledServiceFunction>,
}

/// One function of an [`InstalledService`].
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, TsWire)]
pub struct InstalledServiceFunction {
    pub name: String,
    pub description: String,
    /// The TypeScript type of its one argument; `None` when it takes none.
    pub args: Option<String>,
    /// The TypeScript type of its result; `None` for `unknown`.
    pub returns: Option<String>,
    /// Whether it only reads, so callers with read-only access may call it.
    pub read_only: bool,
}

impl From<ExtensionServiceFunction> for InstalledServiceFunction {
    fn from(function: ExtensionServiceFunction) -> Self {
        Self {
            name: function.name,
            description: function.description,
            args: function.args,
            returns: function.returns,
            read_only: function.read_only,
        }
    }
}

/// What an extension's global `manifest()` function returns. The daemon
/// reads it at install time, in a sandbox with no globals but entity classes
/// and enums.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire, TsInterface)]
pub struct ExtensionManifest {
    /// Stable, unique key for the extension, such as
    /// `"com.example.done-stamp"`. Installing a file with an identifier that
    /// already exists replaces that extension.
    pub identifier: String,
    /// Display name.
    pub name: String,
    /// A whole number, raised with each release: a registry listing a
    /// higher one offers it as an update, and for an `oauth` extension it
    /// is also its connector definition's version.
    pub version: i64,
    /// What the extension does, in a sentence or two.
    pub description: Option<String>,
    /// Who wrote the extension.
    pub author: Option<String>,
    /// The extension's icon: a `data:image/…` URL embedded in the script, or
    /// an `https://` URL. Aim for about 100×100 px; a PNG or a small SVG is
    /// plenty. At most 64 KB. Anything else is dropped without an error, and
    /// the extension's initial is shown instead.
    pub icon: Option<String>,
    /// What the extension may do. An unknown permission rejects the install.
    pub permissions: Vec<ExtensionPermissionKind>,
    /// The events the daemon calls the extension for. An unknown hook
    /// rejects the install.
    pub hooks: Vec<HookKind>,
    /// The functions the extension serves at `serviceRequest`, for a
    /// manifest that lists `service` or `service:isolated`. Once declared,
    /// a call to any other name is refused.
    pub service: Option<ServiceManifest>,
}

/// The `service` block of an [`ExtensionManifest`]. Scripts reach the
/// service by the extension's identifier, as
/// `services.use("<identifier>")`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire, TsInterface)]
pub struct ServiceManifest {
    /// What the service offers, in a sentence or two.
    pub description: Option<String>,
    /// The functions, by name; each name is a JavaScript identifier.
    pub functions: BTreeMap<String, ServiceFunctionManifest>,
}

/// One function of a [`ServiceManifest`].
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire, TsInterface)]
pub struct ServiceFunctionManifest {
    /// What the function does, and what its arguments mean.
    pub description: Option<String>,
    /// The TypeScript type of the function's one argument, such as
    /// `"{ by?: number }"`. Leave it out for a function that takes none.
    pub args: Option<String>,
    /// The TypeScript type of what it returns, such as
    /// `"{ count: number }"`. Leave it out for `unknown`.
    pub returns: Option<String>,
    /// Whether the function only reads, changing nothing, so background
    /// agents may call it. Defaults to `false`.
    #[serde(alias = "readOnly")]
    pub read_only: Option<bool>,
}

/// A permission an extension's manifest lists. `cohesion:read` grants the
/// `cohesion` global read-only: any write throws. `cohesion:write` grants it
/// with writes, and makes the entities hooks receive writable too. `network`
/// grants the `network` global and `timer` the `timer` global. `registry`
/// grants no global: it lets the daemon call the extension's globals
/// `registryResults(query: string)`, which returns `RegistryResult[]`
/// (`{ manifest, sourceURL }`) listing extensions it offers, and
/// `registryLookup(identifiers: string[])`, which returns a `RegistryResult`
/// for each of those identifiers it offers and leaves the rest out; a
/// registry must define both. `oauth` lets the
/// daemon call the extension's sign-in functions (`serviceDescription`,
/// `authenticationURL`, `completeAuthentication`, `completeRefresh` and
/// `getAccountDescription`) so the user can add accounts at a service, and
/// the optional `accountRemoved(account: AccountDescription)` once one is
/// removed. It also grants the `accounts` global, which lists those
/// accounts with their credentials. `connector` lets the daemon call `getOrganizationUnits`,
/// `listResources` and `getResource` to import an account's items as resources; a
/// manifest that lists `connector` must list `oauth` too. `calendar:backend`
/// lets the daemon call `listCalendars`, `listEvents`, `createEvent`,
/// `updateEvent` and `deleteEvent` to sync each account's calendar store both
/// ways; it too needs `oauth`. `service` grants
/// no global: it makes the extension a service clients call, through the
/// `callExtension` command, at its global
/// `serviceRequest(name: string, args: unknown)`, receiving what it returns;
/// with a `service` block in the manifest, agents, scripts and extensions
/// granted `services` can call it too. `service:isolated` is `service` for
/// clients only: it never reaches `services`. A manifest lists one or the
/// other, never both. `services` grants the `services` global, through
/// which the extension calls other installed services. `usage` grants the
/// `usage` global: how many tokens Cohesion's models used and what they
/// cost. `intelligence` grants the `intelligence` global and lets the
/// extension change intelligence settings (endpoints and their models),
/// which `cohesion:write` does not. `notifications` grants the
/// `notifications` global, which sends the user a notification.
#[derive(Clone, Debug, TextEnum, PartialEq, TsWire)]
#[text_enum(rename_all = "custom")]
pub enum ExtensionPermissionKind {
    #[text_enum("cohesion:read")]
    GlobalReadOnly,
    #[text_enum("cohesion:write")]
    GlobalReadWrite,
    #[text_enum("network")]
    Network,
    #[text_enum("timer")]
    Timer,
    #[text_enum("registry")]
    Registry,
    #[text_enum("oauth")]
    OAuth,
    #[text_enum("connector")]
    Connector,
    #[text_enum("calendar:backend")]
    CalendarBackend,
    #[text_enum("service")]
    Service,
    #[text_enum("service:isolated")]
    ServiceIsolated,
    #[text_enum("services")]
    Services,
    #[text_enum("usage")]
    Usage,
    #[text_enum("intelligence")]
    Intelligence,
    #[text_enum("notifications")]
    Notifications,
    #[text_enum("actors")]
    Actors,
}

impl ExtensionPermissionKind {
    /// The permission this one only works beside: `connector` imports the
    /// items of, and `calendar:backend` syncs the calendars of, accounts
    /// that `oauth` signs in to. A manifest listing this
    /// one must list that one, and a call needing this one needs that one
    /// enabled too.
    pub fn needs(&self) -> Option<ExtensionPermissionKind> {
        match self {
            Self::Connector | Self::CalendarBackend => Some(Self::OAuth),
            Self::GlobalReadOnly
            | Self::GlobalReadWrite
            | Self::Network
            | Self::Timer
            | Self::Registry
            | Self::OAuth
            | Self::Service
            | Self::ServiceIsolated
            | Self::Services
            | Self::Usage
            | Self::Intelligence
            | Self::Notifications
            | Self::Actors => None,
        }
    }
}

/// One extension a registry offers, as its globals
/// `registryResults(query: string)` and
/// `registryLookup(identifiers: string[])` return them. The daemon installs from
/// the script at `sourceURL`, reading the manifest from that script; this
/// `manifest` is what the registry lists, shown before the user reviews it.
#[derive(Serialize, Deserialize, Clone, Debug, TsWire, TsInterface)]
pub struct RegistryResult {
    /// The extension's manifest as the registry lists it.
    pub manifest: ExtensionManifest,
    /// An `http` or `https` URL serving the extension's script.
    #[serde(alias = "sourceURL")]
    pub source_url: String,
}
