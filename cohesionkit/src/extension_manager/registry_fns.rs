//! The globals a `registry` extension defines: searching what it offers and
//! looking up the extensions it offers updates for.

use cohesion_core::model::{ExtensionPermissionKind, RegistryResult};
use cohesion_js::ScriptArgument;

use super::ExtensionFn;

/// `registryResults(query)`: the extensions the registry offers for `query`.
pub(crate) struct RegistryResultsFn {
    pub(crate) query: String,
}
impl ExtensionFn for RegistryResultsFn {
    const NAME: &'static str = "registryResults";
    const REQUIRES: ExtensionPermissionKind = ExtensionPermissionKind::Registry;
    type Output = Vec<RegistryResult>;
    fn arguments(self) -> Vec<ScriptArgument> {
        vec![ScriptArgument::String(self.query)]
    }
}

/// `registryLookup(identifiers)`: what the registry offers of `identifiers`,
/// leaving out the ones it doesn't.
pub(crate) struct RegistryLookupFn {
    pub(crate) identifiers: Vec<String>,
}
impl ExtensionFn for RegistryLookupFn {
    const NAME: &'static str = "registryLookup";
    const REQUIRES: ExtensionPermissionKind = ExtensionPermissionKind::Registry;
    type Output = Vec<RegistryResult>;
    fn arguments(self) -> Vec<ScriptArgument> {
        vec![ScriptArgument::Json(self.identifiers.into())]
    }
}
