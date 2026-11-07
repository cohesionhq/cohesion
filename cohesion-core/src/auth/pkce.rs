use aws_lc_rs::digest::{SHA256, digest};
use aws_lc_rs::rand::{SecureRandom, SystemRandom};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

use crate::model::PkceChallenge;

/// One sign-in's PKCE (RFC 7636) pair: a random verifier the daemon keeps,
/// and the challenge derived from it that the sign-in page is sent.
pub struct Pkce {
    /// 32 random bytes, base64url-encoded without padding: 43 characters.
    pub verifier: String,
    pub challenge: PkceChallenge,
}

impl Pkce {
    /// A new pair, with an `S256` challenge.
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        SystemRandom::new()
            .fill(&mut bytes)
            .expect("the system random source never fails");
        let verifier = URL_SAFE_NO_PAD.encode(bytes);
        Self {
            challenge: PkceChallenge {
                challenge: s256(&verifier),
                method: "S256".to_string(),
            },
            verifier,
        }
    }
}

/// The `S256` challenge of `verifier`.
fn s256(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(digest(&SHA256, verifier.as_bytes()))
}

#[cfg(test)]
mod tests;
