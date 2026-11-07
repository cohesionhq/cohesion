use std::{fs, io::ErrorKind};

use cohesion_core::private_files::write_private;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};

use crate::client_storage::{ApplicationStorageError, StoragePath};

/// The certificate and key this client presents to cohesiond over TLS.
pub struct ClientIdentity {
    pub chain: Vec<CertificateDer<'static>>,
    pub private_key: PrivateKeyDer<'static>,
}

impl ClientIdentity {
    /// Writes `pem` (a certificate followed by its private key) as this client's identity.
    pub fn save_pem(p: &StoragePath, pem: &str) -> Result<(), ApplicationStorageError> {
        write_private(&p.certificate_file(), pem.as_bytes())
            .map_err(ApplicationStorageError::CannotWriteCertificate)
    }

    pub fn load(p: &StoragePath) -> Result<ClientIdentity, ApplicationStorageError> {
        let contents = fs::read(p.certificate_file()).map_err(|err| match err.kind() {
            ErrorKind::NotFound => ApplicationStorageError::NotConfigured,
            _ => ApplicationStorageError::CannotReadCertificate(err),
        })?;
        Self::from_pem(&contents)
    }

    /// Parses a certificate chain followed by its private key.
    pub fn from_pem(pem: &[u8]) -> Result<ClientIdentity, ApplicationStorageError> {
        let chain = CertificateDer::pem_slice_iter(pem)
            .collect::<Result<Vec<_>, _>>()
            .map_err(ApplicationStorageError::InvalidCertificate)?;
        if chain.is_empty() {
            return Err(ApplicationStorageError::NoCertificate);
        }
        let private_key = PrivateKeyDer::from_pem_slice(pem)
            .map_err(ApplicationStorageError::InvalidCertificate)?;

        Ok(Self { chain, private_key })
    }
}
