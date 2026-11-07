//! cohesiond's config directory: where the database, server key, config file
//! and socket live.

use std::{
    fmt,
    fs::File,
    io::{self, ErrorKind, Read},
    path::PathBuf,
};

use cohesionkit::{
    Authentication, KeyError,
    paths::{self, ConfigDirectory, ConfigDirectoryError},
    private_files::write_private,
};

/// Why cohesiond's config directory or server key could not be used.
#[derive(Debug)]
pub enum ApplicationStorageError {
    /// There is no key file yet; a new key should be made.
    NothingToRestore,
    /// The key file exists but could not be read.
    CannotOpenKeyFile(io::Error),
    /// The key file's contents are not a usable key.
    CannotUseKey(KeyError),
    /// A new key could not be generated.
    CannotMakeKey(KeyError),
    /// The key file could not be written.
    CannotWriteKey(io::Error),
    /// The config directory could not be located or created.
    Directory(ConfigDirectoryError),
}

impl fmt::Display for ApplicationStorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NothingToRestore => write!(f, "no server key to restore"),
            Self::CannotOpenKeyFile(err) => write!(f, "cannot read the server key file: {err}"),
            Self::CannotUseKey(err) => {
                write!(f, "the server key file is unusable: {}", key_error(err))
            }
            Self::CannotMakeKey(err) => {
                write!(f, "cannot make a server key: {}", key_error(err))
            }
            Self::CannotWriteKey(err) => write!(f, "cannot write the server key file: {err}"),
            Self::Directory(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for ApplicationStorageError {}

/// Describes a `KeyError`, which has no `Display` of its own.
fn key_error(err: &KeyError) -> &'static str {
    match err {
        KeyError::GenerationFailed => "key generation failed",
        KeyError::UnacceptableKey => "not a P-256 PKCS#8 key",
    }
}

/// cohesiond's config directory, the files it keeps there, and its socket.
pub struct StoragePath {
    directory: ConfigDirectory,
    socket: PathBuf,
}
impl StoragePath {
    fn key_file(&self) -> PathBuf {
        self.directory.file("key.pem")
    }
    pub fn config_file(&self) -> PathBuf {
        self.directory.file("config.json")
    }
    /// `path` taken relative to the config directory, unless it is absolute.
    pub fn relative(&self, path: PathBuf) -> PathBuf {
        self.directory.root().join(path)
    }
    pub fn database_file(&self) -> PathBuf {
        self.directory.file("cohesion.db")
    }
    pub fn socket_file(&self) -> PathBuf {
        self.socket.clone()
    }

    pub fn resolve(p: &str) -> Result<Self, ApplicationStorageError> {
        let directory = ConfigDirectory::resolve(p).map_err(ApplicationStorageError::Directory)?;
        let socket = paths::socket_path_in(directory.root());
        Ok(Self { directory, socket })
    }
    pub fn default() -> Result<Self, ApplicationStorageError> {
        let directory = ConfigDirectory::default_for("cohesiond")
            .map_err(ApplicationStorageError::Directory)?;
        let socket = paths::default_socket_path().ok_or(ApplicationStorageError::Directory(
            ConfigDirectoryError::NoPlatformDirectory,
        ))?;
        Ok(Self { directory, socket })
    }
}

pub struct ApplicationData {
    pub server_key: Authentication,
}

impl ApplicationData {
    pub fn make(p: &StoragePath) -> Result<ApplicationData, ApplicationStorageError> {
        let restore = Self::restore(p);
        match restore {
            Ok(restored) => Ok(restored),
            Err(ApplicationStorageError::NothingToRestore) => Self::new(p),
            _ => restore,
        }
    }

    fn new(p: &StoragePath) -> Result<ApplicationData, ApplicationStorageError> {
        let s = ApplicationData {
            server_key: Authentication::generate()
                .map_err(ApplicationStorageError::CannotMakeKey)?,
        };
        s.save(p)?;
        Ok(s)
    }

    fn restore(p: &StoragePath) -> Result<ApplicationData, ApplicationStorageError> {
        let key_path = p.key_file();
        // if the key file doesn't exist, we can't restore.
        let mut key_file = File::open(key_path).map_err(|err| match err.kind() {
            ErrorKind::NotFound => ApplicationStorageError::NothingToRestore,
            _ => ApplicationStorageError::CannotOpenKeyFile(err),
        })?;
        let mut key_contents = Vec::<u8>::new();
        key_file
            .read_to_end(&mut key_contents)
            .map_err(ApplicationStorageError::CannotOpenKeyFile)?;

        let auth =
            Authentication::make(&key_contents).map_err(ApplicationStorageError::CannotUseKey)?;

        Ok(Self { server_key: auth })
    }

    fn save(&self, p: &StoragePath) -> Result<(), ApplicationStorageError> {
        let key_bytes = self.server_key.bytes();
        write_private(&p.key_file(), &key_bytes).map_err(ApplicationStorageError::CannotWriteKey)
    }
}
