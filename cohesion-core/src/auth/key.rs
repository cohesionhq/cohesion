use aws_lc_rs::digest::{SHA256, digest};
use aws_lc_rs::encoding::AsDer;
use aws_lc_rs::rand::SystemRandom;
use aws_lc_rs::signature::KeyPair;
use aws_lc_rs::signature::{ECDSA_P256_SHA256_ASN1_SIGNING, EcdsaKeyPair, EcdsaPublicKey};

#[derive(Debug)]
pub enum KeyError {
    GenerationFailed,
    UnacceptableKey,
}

pub struct Authentication {
    key_pair: EcdsaKeyPair,
}
impl Authentication {
    pub fn generate() -> Result<Self, KeyError> {
        let rng = SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng)
            .map_err(|_| KeyError::GenerationFailed)?;
        Self::make(pkcs8.as_ref())
    }
    pub fn make(bytes: &[u8]) -> Result<Self, KeyError> {
        let key_pair = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, bytes)
            .map_err(|_err| KeyError::UnacceptableKey)?;

        Ok(Self { key_pair })
    }

    pub fn public_key(&self) -> &EcdsaPublicKey {
        self.key_pair.public_key()
    }

    /// Fingerprint of this key's SubjectPublicKeyInfo; matches
    /// [`key_fingerprint`](super::certificate::key_fingerprint) of any certificate it signs.
    pub fn fingerprint(&self) -> String {
        let spki = self
            .public_key()
            .as_der()
            .expect("P-256 keys always encode");
        fingerprint(spki.as_ref())
    }

    pub fn bytes(&self) -> Vec<u8> {
        self.key_pair.to_pkcs8v1().unwrap().as_ref().to_vec()
    }
}

/// Lowercase hex SHA-256 of `bytes`.
pub fn fingerprint(bytes: &[u8]) -> String {
    let fp = digest(&SHA256, bytes);
    fp.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}
