//! Why a call to an extension failed, and what the user is told about it.

use std::fmt;

use cohesion_core::log::Module;
use cohesion_core::log_error;
use cohesion_core::model::ExtensionPermissionKind;
use cohesion_core::protocol::ServerError;
use cohesion_db::CommandError;
use cohesion_js::ScriptError;

/// A call to an extension that failed before or while it ran.
#[derive(Debug)]
pub(crate) enum ExtensionError {
    /// Reading the extension failed: `NotFound` when it isn't installed.
    Database(CommandError),
    /// The extension doesn't have this permission enabled.
    NotGranted(ExtensionPermissionKind),
    /// Starting the scope or running the call failed.
    Script(ScriptError),
}
impl From<CommandError> for ExtensionError {
    fn from(value: CommandError) -> Self {
        Self::Database(value)
    }
}
impl From<ScriptError> for ExtensionError {
    fn from(value: ScriptError) -> Self {
        Self::Script(value)
    }
}
impl fmt::Display for ExtensionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(e) => write!(f, "{e:?}"),
            Self::NotGranted(permission) => {
                write!(f, "the `{}` permission isn't enabled", permission.as_str())
            }
            Self::Script(e) => write!(f, "{e}"),
        }
    }
}
/// What a client is told. A script's failure is logged and internal here;
/// a caller that shows the script's fault matches `Script` first and words
/// it with [`script_fault`].
impl From<ExtensionError> for ServerError {
    fn from(value: ExtensionError) -> Self {
        match value {
            ExtensionError::Database(e) => e.into(),
            ExtensionError::NotGranted(permission) => ServerError::validation(
                None,
                format!(
                    "This extension doesn’t have the `{}` permission enabled.",
                    permission.as_str()
                ),
            ),
            ExtensionError::Script(e) => {
                log_error!(Module::Extensions, "An extension call failed: {e}");
                ServerError::internal()
            }
        }
    }
}

/// What the user is told about a failed call to `function` on the extension
/// `name`. What the script did wrong is the extension author's to see:
/// what it threw, a missing function, or a result of the wrong shape, which
/// `shape` words from the reason. Anything else is logged and is `None`.
pub(crate) fn script_fault(
    name: &str,
    function: &str,
    error: ScriptError,
    shape: impl FnOnce(String) -> String,
) -> Option<String> {
    match error {
        ScriptError::Exception { message, .. } => Some(message),
        ScriptError::InvalidFormat(message) => Some(shape(message)),
        e @ ScriptError::MissingFunction(_) => Some(e.to_string()),
        e => {
            log_error!(Module::Extensions, "{name}: {function} failed: {e}");
            None
        }
    }
}
