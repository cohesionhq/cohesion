//! How a client identifies itself on cohesiond's Unix socket. Over TLS the
//! certificate does that during the TLS handshake; the socket has no such
//! step, so a client sends one [`LocalIdentity`] line first and the daemon
//! answers one [`LocalHandshakeReply`] line, before either side starts a
//! session. Only the socket transports on both sides read or write these.

use aws_lc_rs::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};

use super::key::fingerprint;

/// The first line a client sends on the socket.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LocalIdentity {
    /// The client's local key, made by [`LocalIdentity::generate`] and kept
    /// with its configuration, as a paired client keeps its certificate.
    pub key: String,
    /// The name a client new to the daemon is listed under; a known key
    /// keeps the name its client has.
    pub name: String,
}
impl LocalIdentity {
    /// A new identity named `name`, with a random key.
    pub fn generate(name: String) -> Self {
        let mut bytes = [0u8; 32];
        SystemRandom::new()
            .fill(&mut bytes)
            .expect("the system random source never fails");
        let key = bytes.iter().map(|b| format!("{b:02x}")).collect();
        Self { key, name }
    }

    /// What the daemon stores for the key: its SHA-256, as a certificate's
    /// key is stored by its fingerprint.
    pub fn fingerprint(&self) -> String {
        fingerprint(self.key.as_bytes())
    }
}

/// The daemon's answer to a [`LocalIdentity`]; a session follows only
/// `Accepted`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "outcome", rename_all = "camelCase")]
pub enum LocalHandshakeReply {
    Accepted,
    /// Why not, as people read it, such as a revoked key.
    Refused {
        reason: String,
    },
}
