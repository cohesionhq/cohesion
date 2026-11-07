//! The server's TLS configuration: its certificate, and mandatory client certificates.

use std::sync::Arc;

use cohesionkit::Certificate;
use rustls::ServerConfig;

use super::client_cert_verifier::ClientVerifier;

pub fn server_config(cert: Certificate) -> ServerConfig {
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let mut config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("unsupported TLS protocol versions")
        .with_client_cert_verifier(Arc::new(ClientVerifier::new()))
        .with_single_cert(cert.chain, cert.private_key)
        .expect("invalid server certificate or key");
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    config
}
