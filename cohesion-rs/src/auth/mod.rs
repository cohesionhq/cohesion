//! Keys and pairing, shared by clients and the server: keys, fingerprints,
//! the self-signed certificates each side presents over TLS, the setup code
//! a client pairs with, and the handshake a client identifies itself with
//! on the Unix socket, and the name a client is given when its user gives
//! none.

mod certificate;
mod client_name;
mod host;
mod key;
mod local_handshake;
mod setup;

pub use certificate::{Certificate, CertificateError, key_fingerprint};
pub use client_name::generated_client_name;
pub use host::{Host, InvalidHost};
pub use key::{Authentication, KeyError, fingerprint};
pub use local_handshake::{LocalHandshakeReply, LocalIdentity};
pub use setup::{DEFAULT_PORT, ServerAddress, SetupCode, SetupCodeError};
