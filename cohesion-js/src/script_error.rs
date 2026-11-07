use std::fmt;

use cohesion_db::CommandError;

/// Why a script did not produce an output.
#[derive(Debug)]
pub enum ScriptError {
    /// The script threw. `message` is what a caller sees; `stack`, when
    /// the script threw an `Error`, says where, for the logs.
    Exception {
        message: String,
        stack: Option<String>,
    },
    /// The script engine stopped before replying.
    Cancelled,
    /// Its [`ScriptStop`](crate::ScriptStop) stopped the script.
    Stopped,
    /// A job panicked on its worker; carries the panic message. The worker
    /// survives, so only this job is lost.
    Panicked(String),
    /// The engine itself failed (not the script).
    InternalError(String),
    /// Loading data for the script failed.
    LoadError(CommandError),
    /// The data returned was not in the expected format.
    InvalidFormat(String),
    /// The script has no global function with this name.
    MissingFunction(String),
    /// Could not apply the trace.
    TracingError,
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exception {
                message,
                stack: None,
            } => write!(f, "script threw: {message}"),
            Self::Exception {
                message,
                stack: Some(stack),
            } => write!(f, "script threw: {message}\n{stack}"),
            Self::Cancelled => f.write_str("script worker exited unexpectedly"),
            Self::Stopped => f.write_str("the user stopped the script"),
            Self::Panicked(message) => write!(f, "script job panicked: {message}"),
            Self::InternalError(message) => write!(f, "script engine error: {message}"),
            Self::LoadError(error) => write!(f, "could not load data: {error:?}"),
            Self::InvalidFormat(error) => {
                write!(f, "did not get data in the expected format: {error:?}")
            }
            Self::MissingFunction(name) => {
                write!(f, "script does not define a function named `{name}`")
            }
            Self::TracingError => {
                write!(f, "cannot initiate tracing")
            }
        }
    }
}
impl std::error::Error for ScriptError {}

impl From<CommandError> for ScriptError {
    fn from(value: CommandError) -> Self {
        Self::LoadError(value)
    }
}
