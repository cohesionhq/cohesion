//! Requests about extensions.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

use crate::model::{
    Extension, ExtensionManifest, ExtensionPermissionKind, HookKind, RegistryResult,
};

#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RegisterExtensionRequest {
    pub script: String,
    pub allow_overwrite: bool,
}

/// Lists every installed extension, in the order its hooks run.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListExtensionsRequest;

/// An installed extension without its script.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ExtensionSummary {
    pub id: i64,
    pub name: String,
    pub version: i64,
    pub script_identifier: String,
    pub description: String,
    pub author: String,
    /// A `data:image/…` or `https://` URL.
    pub icon: Option<String>,
    /// Hook evaluation order: lower runs first, ties by `id`.
    pub priority: i64,
}
impl From<Extension> for ExtensionSummary {
    fn from(value: Extension) -> Self {
        Self {
            id: value.id,
            name: value.name,
            version: value.version,
            script_identifier: value.script_identifier,
            description: value.description,
            author: value.author,
            icon: value.icon,
            priority: value.priority,
        }
    }
}

/// Lists the permissions recorded for extension `extension_id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListExtensionPermissionsRequest {
    pub extension_id: i64,
}

/// Enables or disables `permission` for extension `extension_id`. The
/// extension's running sandbox is discarded, so the change applies at once.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetExtensionPermissionRequest {
    pub extension_id: i64,
    pub permission: ExtensionPermissionKind,
    pub enabled: bool,
}

/// Removes extension `extension_id`, stopping its sandbox and timers and
/// dropping its permission and hook grants.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct UninstallExtensionRequest {
    pub extension_id: i64,
}

/// Lists the hooks recorded for extension `extension_id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListExtensionHooksRequest {
    pub extension_id: i64,
}

/// Enables or disables `hook` for extension `extension_id`. The extension's
/// running sandbox is discarded, and enabling `launch` calls `onLaunch`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetExtensionHookRequest {
    pub extension_id: i64,
    pub hook: HookKind,
    pub enabled: bool,
}

/// Asks every extension with the `registry` permission granted for the
/// extensions it offers matching `query` (`""` lists what it features).
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SearchExtensionRegistriesRequest {
    pub query: String,
}

/// One registry's answer to a search.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RegistrySection {
    /// The registry extension's id.
    pub extension_id: i64,
    /// The registry extension's name.
    pub name: String,
    pub results: Vec<RegistryResult>,
    /// Why this registry gave no results: it failed to load, threw, or
    /// returned something other than `RegistryResult[]`.
    pub error: Option<String>,
}

/// Lists the updates the last check found for installed extensions.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListExtensionUpdatesRequest;

/// Asks every extension with the `registry` permission granted about the
/// installed extensions now, rather than waiting for the daily check.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct CheckExtensionUpdatesRequest;

/// A registry's entry for an installed extension at a higher version than
/// the installed one.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ExtensionUpdate {
    /// The installed extension's id.
    pub extension_id: i64,
    /// The id of the registry extension that offers it.
    pub registry_extension_id: i64,
    pub result: RegistryResult,
}

/// Downloads the extension script at `source_url` (`http` or `https`) and
/// reads its manifest, without installing it.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct PreviewExtensionRequest {
    pub source_url: String,
}

/// Reads the manifest of an extension `script` the client already has, such
/// as one from a file, without installing it.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct PreviewExtensionScriptRequest {
    pub script: String,
}

/// An extension script as downloaded or given, for review before installing it with
/// `registerExtension`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ExtensionPreview {
    pub script: String,
    /// The manifest the script itself declares.
    // Boxed to keep `ReplyData` small.
    pub manifest: Box<ExtensionManifest>,
    /// The installed extension with the same identifier, which installing
    /// this script replaces.
    pub installed: Option<ExtensionSummary>,
}

/// Calls the global `serviceRequest(name, args, caller)` of the extension
/// whose manifest declares `identifier`, which the extension needs the
/// `service` or `service:isolated` permission enabled for, and answers what
/// it returns. `caller` is `{ clientId }`, naming the client asking. An
/// extension that declares a `service` block answers only the names it
/// declares.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct CallExtensionRequest {
    /// The extension's manifest `identifier`, such as
    /// `com.cohesion.cloud.ios-companion`; 1 to 256 characters.
    pub identifier: String,
    /// Which request, as the extension names them; at most 128 characters.
    /// One of its declared functions when it declares a `service` block.
    pub name: String,
    /// Passed to the extension as the JS value this JSON parses to; `null`
    /// when left out. At most 64 KB as JSON.
    #[serde(default)]
    pub args: serde_json::Value,
}

/// What an extension's `serviceRequest` returned; `null` for `undefined`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ExtensionCallResult {
    pub result: serde_json::Value,
}
