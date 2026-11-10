use std::{fmt, io, path::PathBuf};

use cohesion_core::SetupCodeError;
use cohesion_core::protocol::ServerError;

use crate::{client_storage::ApplicationStorageError, connection_error::ConnectionError};

/// Why a CLI command failed; printed to stderr by `main`.
pub enum CliError {
    NotConfigured(PathBuf),
    InvalidSetupCode(SetupCodeError),
    Storage(ApplicationStorageError),
    Setup(ConnectionError),
    Unreachable(String, ConnectionError),
    NoDefaultSocket,
    Server(ServerError),
    UnexpectedReply,
    NoWorkspace,
    /// Nothing of this kind (e.g. "workspace") has this id or name.
    UnknownName(&'static str, String),
    /// Several of this kind share this name; their ids.
    AmbiguousName(&'static str, String, Vec<i64>),
    Tempfile(io::Error),
    CannotLaunchEditor(io::Error),
    CannotReadFile(io::Error),
    CannotWriteFile(io::Error),
    FileExists(PathBuf),
    ExportExists(PathBuf),
    CannotWriteExport(io::Error),
    CannotReadExport(io::Error),
    CannotReadStdin(io::Error),
    CannotWriteStdout(io::Error),
    CannotReadTerminal(io::Error),
    NothingToAppend,
    EditorFailed,
    InvalidRequest(serde_json::Error),
    CannotServe(io::Error),
    InvalidWebBundle(io::Error),
    MissingDevToken,
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotConfigured(root) => write!(
                f,
                "No client configuration in {}. Run `cohesion setup <setup-code>`.",
                root.display()
            ),
            Self::InvalidSetupCode(err) => {
                let reason = match err {
                    SetupCodeError::NotBase64 => "not base64",
                    SetupCodeError::Malformed => "expected a key, an address and a fingerprint",
                    SetupCodeError::InvalidAddress => "the address isn't a valid host and port",
                };
                write!(f, "Invalid setup code: {reason}.")
            }
            Self::Storage(err) => write!(f, "Client configuration error: {err}"),
            Self::Setup(err) => write!(f, "Couldn't set up the connection: {err}"),
            Self::Unreachable(target, err) => {
                write!(f, "Couldn't reach cohesiond at {target}: {err}")
            }
            Self::NoDefaultSocket => write!(
                f,
                "Couldn't find cohesiond's default socket. Pass its path to --local."
            ),
            Self::Server(err) => write!(f, "cohesiond: {}", err.message),
            Self::UnexpectedReply => write!(f, "Unexpected reply from cohesiond."),
            Self::NoWorkspace => write!(
                f,
                "No workspace selected. Run `cohesion switch <workspace>` or pass --workspace."
            ),
            Self::UnknownName(kind, query) => write!(f, "No {kind} with id or name {query:?}."),
            Self::AmbiguousName(kind, query, ids) => {
                let ids: Vec<_> = ids.iter().map(i64::to_string).collect();
                write!(
                    f,
                    "Several {kind}s are named {query:?} (ids {}). Use an id instead.",
                    ids.join(", ")
                )
            }
            Self::Tempfile(err) => write!(f, "Can't use a temporary file: {err}"),
            Self::CannotLaunchEditor(err) => write!(f, "Couldn't open your editor: {err}"),
            Self::CannotReadFile(err) => write!(f, "Couldn't read the script file: {err}"),
            Self::CannotWriteFile(err) => write!(f, "Couldn't write the script file: {err}"),
            Self::ExportExists(path) => write!(
                f,
                "{} already exists. Choose a new file name.",
                path.display()
            ),
            Self::CannotWriteExport(err) => write!(f, "Couldn't write the export: {err}"),
            Self::CannotReadExport(err) => write!(f, "Couldn't read the export: {err}"),
            Self::FileExists(path) => write!(
                f,
                "{} already exists. Pass --force to replace it.",
                path.display()
            ),
            Self::CannotReadStdin(err) => write!(f, "Couldn't read stdin: {err}"),
            Self::CannotWriteStdout(err) => write!(f, "Couldn't write to stdout: {err}"),
            Self::CannotReadTerminal(err) => write!(f, "Couldn't read from the terminal: {err}"),
            Self::NothingToAppend => write!(f, "Nothing to append. Pass text or pipe it in."),
            Self::InvalidRequest(err) => write!(f, "Invalid request: {err}"),
            Self::EditorFailed => write!(f, "Your editor exited with an error."),
            Self::CannotServe(err) => write!(f, "Couldn't serve the web UI: {err}"),
            Self::InvalidWebBundle(err) => {
                write!(f, "cohesiond sent a web UI that couldn't be read: {err}")
            }
            Self::MissingDevToken => write!(
                f,
                "--dev needs the token the dev server will present in COHESION_DEV_TOKEN. Start it with scripts/web-dev.sh."
            ),
        }
    }
}
