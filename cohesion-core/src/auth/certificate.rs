use base64::{Engine, engine::general_purpose::STANDARD};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

use super::key::{Authentication, fingerprint};

#[derive(Debug)]
pub enum CertificateError {
    UnusableKey,
    GenerationFailed,
}

/// A self-signed certificate for an [`Authentication`] key, ready to hand to rustls.
pub struct Certificate {
    pub chain: Vec<CertificateDer<'static>>,
    pub private_key: PrivateKeyDer<'static>,
}

impl Authentication {
    pub fn self_signed_certificate(&self, name: &str) -> Result<Certificate, CertificateError> {
        let cert = self.sign_self(name)?;
        Ok(Certificate {
            chain: vec![cert.der().clone()],
            private_key: PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.bytes())),
        })
    }

    /// The certificate followed by its PKCS#8 private key, as PEM.
    pub fn self_signed_certificate_pem(&self, name: &str) -> Result<String, CertificateError> {
        let cert = self.sign_self(name)?;
        Ok(pem("CERTIFICATE", cert.der()) + &pem("PRIVATE KEY", &self.bytes()))
    }

    fn sign_self(&self, name: &str) -> Result<rcgen::Certificate, CertificateError> {
        let key_pair = rcgen::KeyPair::try_from(self.bytes().as_slice())
            .map_err(|_| CertificateError::UnusableKey)?;
        let cert = rcgen::CertificateParams::new(vec![name.to_string()])
            .and_then(|params| params.self_signed(&key_pair))
            .map_err(|_| CertificateError::GenerationFailed)?;
        Ok(cert)
    }
}

fn pem(label: &str, der: &[u8]) -> String {
    let encoded = STANDARD.encode(der);
    let mut out = format!("-----BEGIN {label}-----\n");
    // Base64 is ASCII, so splitting on byte boundaries is safe.
    for line in encoded.as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(line).expect("base64 is ASCII"));
        out.push('\n');
    }
    out.push_str(&format!("-----END {label}-----\n"));
    out
}

/// Fingerprint of a certificate's public key (its SubjectPublicKeyInfo); how
/// both clients and cohesiond are identified.
///
/// Unlike a fingerprint of the whole certificate, this survives re-signing, as
/// cohesiond does on every start.
pub fn key_fingerprint(cert: &CertificateDer<'_>) -> Option<String> {
    let cert = webpki::EndEntityCert::try_from(cert).ok()?;
    Some(fingerprint(cert.subject_public_key_info().as_ref()))
}
