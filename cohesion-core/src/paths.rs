//! Filesystem locations shared by cohesiond and its local clients, so both
//! sides agree on where the daemon listens without extra configuration.

use crate::private_files::create_private_dir;
use std::{
    fs, io,
    path::{self, Path, PathBuf},
};

const SOCKET_NAME: &str = "cohesiond.sock";

/// Where cohesiond keeps its database and keys unless given `--config`.
pub fn default_daemon_directory() -> Option<PathBuf> {
    dirs::config_local_dir().map(|dir| dir.join("cohesiond"))
}

/// The socket of a cohesiond using the default directory: the per-user
/// runtime directory where the platform has one (Linux), otherwise beside the
/// daemon's data (macOS, whose temporary directory is periodically purged).
pub fn default_socket_path() -> Option<PathBuf> {
    dirs::runtime_dir()
        .or_else(default_daemon_directory)
        .map(|dir| dir.join(SOCKET_NAME))
}

/// The socket of a cohesiond given `--config`, kept inside that directory so
/// instances with different directories never share a socket.
pub fn socket_path_in(directory: &Path) -> PathBuf {
    directory.join(SOCKET_NAME)
}

/// A directory of configuration and data files, created on first use.
pub struct ConfigDirectory {
    root: PathBuf,
}

/// Why a [`ConfigDirectory`] could not be located or created.
#[derive(Debug)]
pub enum ConfigDirectoryError {
    /// This platform has no per-user config directory to default to.
    NoPlatformDirectory,
    CannotRead(io::Error),
    CannotCreate(io::Error),
}

impl std::fmt::Display for ConfigDirectoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoPlatformDirectory => write!(f, "couldn’t find a config directory"),
            Self::CannotRead(err) => write!(f, "cannot read config directory: {err}"),
            Self::CannotCreate(err) => write!(f, "cannot create config directory: {err}"),
        }
    }
}
impl std::error::Error for ConfigDirectoryError {}

impl ConfigDirectory {
    /// `--config` value: made absolute and created.
    pub fn resolve(path: &str) -> Result<Self, ConfigDirectoryError> {
        let root = path::absolute(path).map_err(ConfigDirectoryError::CannotRead)?;
        Self::create(root)
    }

    /// The platform's config dir joined with `name` ("cohesion" / "cohesiond").
    pub fn default_for(name: &str) -> Result<Self, ConfigDirectoryError> {
        let base = dirs::config_local_dir().ok_or(ConfigDirectoryError::NoPlatformDirectory)?;
        Self::create(base.join(name))
    }

    /// The directory itself.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The path of the file called `name` inside this directory.
    pub fn file(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn create(root: PathBuf) -> Result<Self, ConfigDirectoryError> {
        if !fs::exists(&root).map_err(ConfigDirectoryError::CannotRead)? {
            create_private_dir(&root).map_err(ConfigDirectoryError::CannotCreate)?;
        }
        Ok(Self { root })
    }
}
