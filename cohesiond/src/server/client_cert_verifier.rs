//! The rustls verifier that demands, but does not judge, client certificates.

use rustls::{
    DigitallySignedStruct, DistinguishedName, Error, SignatureScheme,
    client::danger::HandshakeSignatureValid,
    crypto::{WebPkiSupportedAlgorithms, verify_tls12_signature, verify_tls13_signature},
    pki_types::{CertificateDer, UnixTime},
    server::danger::{ClientCertVerified, ClientCertVerifier},
};

/// Requires a client certificate and proof of its key, but accepts any certificate.
///
/// Whether the certificate belongs to a registered client is decided after the
/// handshake (see `PeerAcceptor`), so unregistered clients can still reach `/register`.
#[derive(Debug)]
pub struct ClientVerifier {
    algs: WebPkiSupportedAlgorithms,
}
impl ClientVerifier {
    pub fn new() -> Self {
        ClientVerifier {
            algs: rustls::crypto::aws_lc_rs::default_provider().signature_verification_algorithms,
        }
    }
}
impl ClientCertVerifier for ClientVerifier {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[] // no CA to advertise
    }

    fn verify_client_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, Error> {
        Ok(ClientCertVerified::assertion())
    }

    // These prove the client holds the private key. Never stub them out.
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls12_signature(message, cert, dss, &self.algs)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(message, cert, dss, &self.algs)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algs.supported_schemes()
    }
}
