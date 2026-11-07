//! Key management shared by clients and the server: keys, fingerprints, the
//! self-signed certificates each side presents over TLS, the handshake a
//! client identifies itself with on the Unix socket, and the PKCE pairs an
//! extension's sign-in is sent.

mod certificate;
mod host;
mod key;
mod local_handshake;
mod pkce;
mod setup;

pub use certificate::{Certificate, CertificateError, key_fingerprint};
pub use host::{Host, InvalidHost};
pub use key::{Authentication, KeyError, fingerprint};
pub use local_handshake::{LocalHandshakeReply, LocalIdentity};
pub use pkce::Pkce;
pub use setup::{DEFAULT_PORT, ServerAddress, SetupCode, SetupCodeError};
