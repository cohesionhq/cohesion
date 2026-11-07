use std::{
    fmt, io,
    path::{Path, PathBuf},
};

use cohesion_core::{
    KeyError,
    auth::CertificateError,
    paths::{ConfigDirectory, ConfigDirectoryError},
};
use rustls::pki_types::pem;

/// Why this client's config directory, configuration, or identity could not be used.
#[derive(Debug)]
pub enum ApplicationStorageError {
    /// There is no configuration or identity yet; `setup` has not run.
    NotConfigured,
    /// The configuration file exists but could not be read.
    CannotReadConfig(io::Error),
    /// The configuration is not valid JSON of the expected shape.
    InvalidConfig(serde_json::Error),
    /// The configuration file could not be written.
    CannotWriteConfig(io::Error),
    /// The identity file exists but could not be read.
    CannotReadCertificate(io::Error),
    /// The identity file holds no certificate.
    NoCertificate,
    /// The identity file's PEM could not be parsed.
    InvalidCertificate(pem::Error),
    /// The identity file could not be written.
    CannotWriteCertificate(io::Error),
    /// A key for a new identity could not be generated.
    CannotCreateKey(KeyError),
    /// A certificate for a new identity could not be signed.
    CannotCreateCertificate(CertificateError),
    /// The saved web UI token exists but could not be read.
    CannotReadWebToken(io::Error),
    /// The web UI token could not be saved.
    CannotWriteWebToken(io::Error),
    /// The config directory could not be located or created.
    Directory(ConfigDirectoryError),
}

impl fmt::Display for ApplicationStorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotConfigured => write!(f, "not configured"),
            Self::CannotReadConfig(err) => write!(f, "couldn't read config.json: {err}"),
            Self::InvalidConfig(err) => write!(f, "invalid config.json: {err}"),
            Self::CannotWriteConfig(err) => write!(f, "couldn't write config.json: {err}"),
            Self::CannotReadCertificate(err) => write!(f, "couldn't read client.pem: {err}"),
            Self::NoCertificate => write!(f, "client.pem holds no certificate"),
            Self::InvalidCertificate(err) => write!(f, "invalid client.pem: {err}"),
            Self::CannotWriteCertificate(err) => write!(f, "couldn't write client.pem: {err}"),
            Self::CannotCreateKey(err) => {
                let reason = match err {
                    KeyError::GenerationFailed => "key generation failed",
                    KeyError::UnacceptableKey => "the generated key was unusable",
                };
                write!(f, "couldn't create a client key: {reason}")
            }
            Self::CannotCreateCertificate(err) => {
                let reason = match err {
                    CertificateError::UnusableKey => "the key can't sign certificates",
                    CertificateError::GenerationFailed => "signing failed",
                };
                write!(f, "couldn't create a client certificate: {reason}")
            }
            Self::CannotReadWebToken(err) => write!(f, "couldn't read web-token: {err}"),
            Self::CannotWriteWebToken(err) => write!(f, "couldn't write web-token: {err}"),
            Self::Directory(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for ApplicationStorageError {}

/// This client's config directory and the files it keeps there.
pub struct StoragePath {
    directory: ConfigDirectory,
}
impl StoragePath {
    pub fn root(&self) -> &Path {
        self.directory.root()
    }
    pub fn config_file(&self) -> PathBuf {
        self.directory.file("config.json")
    }
    pub fn certificate_file(&self) -> PathBuf {
        self.directory.file("client.pem")
    }
    /// The token `cohesion open` asks the web UI for, kept so its URL survives restarts.
    pub fn web_token_file(&self) -> PathBuf {
        self.directory.file("web-token")
    }

    pub fn resolve(p: &str) -> Result<Self, ApplicationStorageError> {
        let directory = ConfigDirectory::resolve(p).map_err(ApplicationStorageError::Directory)?;
        Ok(Self { directory })
    }
    pub fn default() -> Result<Self, ApplicationStorageError> {
        let directory =
            ConfigDirectory::default_for("cohesion").map_err(ApplicationStorageError::Directory)?;
        Ok(Self { directory })
    }
}
