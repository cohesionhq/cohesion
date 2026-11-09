//! Key management shared by clients and the server: keys, fingerprints, the
//! self-signed certificates each side presents over TLS, the handshake a
//! client identifies itself with on the Unix socket, and the PKCE pairs an
//! extension's sign-in is sent. All but PKCE live in the MIT `cohesion-rs`,
//! which clients link, and are re-exported here.

mod pkce;

pub use cohesion_rs::auth::{
    Authentication, Certificate, CertificateError, DEFAULT_PORT, Host, InvalidHost, KeyError,
    LocalHandshakeReply, LocalIdentity, ServerAddress, SetupCode, SetupCodeError, fingerprint,
    key_fingerprint,
};
pub use pkce::Pkce;
