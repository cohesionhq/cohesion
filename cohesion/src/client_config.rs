use std::{fs, io::ErrorKind, path::PathBuf};

use cohesion_core::private_files::write_private;
use cohesion_core::{Host, LocalIdentity};
use serde::{Deserialize, Serialize};

use crate::client_storage::{ApplicationStorageError, StoragePath};

#[derive(Serialize, Deserialize)]
pub struct Configuration {
    #[serde(flatten)]
    pub server: Server,
    /// The workspace selected with `cohesion switch`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<i64>,
}

/// How this client reaches cohesiond.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum Server {
    /// A cohesiond on this machine, trusted through the socket's file
    /// permissions; `identity` says which client this is.
    Local {
        socket: PathBuf,
        identity: LocalIdentity,
    },
    /// A cohesiond over mutual TLS, registered with a setup code.
    Remote {
        host: Host,
        /// The HTTPS port, from the setup code.
        port: u16,
        /// Key fingerprint of cohesiond's certificate, from the setup code.
        server_fingerprint: String,
    },
}

impl Configuration {
    pub fn load(p: &StoragePath) -> Result<Configuration, ApplicationStorageError> {
        let contents = fs::read(p.config_file()).map_err(|err| match err.kind() {
            ErrorKind::NotFound => ApplicationStorageError::NotConfigured,
            _ => ApplicationStorageError::CannotReadConfig(err),
        })?;
        serde_json::from_slice(&contents).map_err(ApplicationStorageError::InvalidConfig)
    }

    pub fn save(&self, p: &StoragePath) -> Result<(), ApplicationStorageError> {
        let contents =
            serde_json::to_vec_pretty(self).map_err(ApplicationStorageError::InvalidConfig)?;
        write_private(&p.config_file(), &contents)
            .map_err(ApplicationStorageError::CannotWriteConfig)
    }
}
