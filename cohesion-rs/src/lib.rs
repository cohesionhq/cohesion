//! What a client needs to talk to a Cohesion daemon, and only that: the
//! keys and setup code it pairs with, the TLS key fingerprint it checks the
//! daemon by, files only its user may read, and the protocol's envelope.
//!
//! MIT-licensed, unlike the rest of the workspace, so a client under any
//! licence can link it (`docs/licensing.md`). It depends on no other
//! workspace crate; `cohesion-core` depends on it and re-exports what it
//! shares from its own paths.

pub mod auth;
pub mod private_files;
pub mod protocol;

pub use auth::{
    Authentication, Certificate, CertificateError, DEFAULT_PORT, Host, InvalidHost, KeyError,
    LocalHandshakeReply, LocalIdentity, ServerAddress, SetupCode, SetupCodeError, fingerprint,
    key_fingerprint,
};
