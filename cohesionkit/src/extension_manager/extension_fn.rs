//! A global function an extension defines for the daemon to call, as a type:
//! its name, the permission it needs, its arguments and what it returns, so
//! each call names them once and can't pair a function with the wrong grant
//! or result.

use cohesion_core::model::ExtensionPermissionKind;
use cohesion_js::{Sandbox, ScriptArgument, ScriptError};
use serde::de::DeserializeOwned;

/// A global an extension defines, called through
/// [`ExtensionManager::call`](super::ExtensionManager::call).
pub(crate) trait ExtensionFn: Sized {
    /// The global's name in the script.
    const NAME: &'static str;
    /// The permission the extension must have enabled, with any it
    /// [needs](ExtensionPermissionKind::needs).
    const REQUIRES: ExtensionPermissionKind;
    /// What the global returns.
    type Output: DeserializeOwned + Send + 'static;

    /// The arguments the global takes, in order.
    fn arguments(self) -> Vec<ScriptArgument>;

    /// Calls the global on `sandbox`, for a job already running on the
    /// extension's scope or reading a script before it is installed.
    fn call_in(self, sandbox: &Sandbox) -> Result<Self::Output, ScriptError> {
        sandbox.call_returning_with_arguments(Self::NAME, &self.arguments())
    }
}
