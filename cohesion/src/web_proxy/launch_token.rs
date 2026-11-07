//! The secret the web UI presents to `cohesion open`, saved in the config directory so
//! a browser given it once (through a launch link) keeps working across launches.

use std::{fs, io::ErrorKind};

use cohesion_core::private_files::write_private;
use rustls::crypto::aws_lc_rs::default_provider;

use crate::client_storage::{ApplicationStorageError, StoragePath};

const TOKEN_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
const TOKEN_LENGTH: usize = 32;

/// The saved token, or a fresh one (saved for next time) when there is none or `reset`.
pub fn saved(storage: &StoragePath, reset: bool) -> Result<String, ApplicationStorageError> {
    let path = storage.web_token_file();
    if !reset {
        match fs::read_to_string(&path) {
            Ok(token) => return Ok(token.trim().to_string()),
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => return Err(ApplicationStorageError::CannotReadWebToken(err)),
        }
    }
    let token = fresh();
    // Only this user may read it: it lets a browser act as this client.
    write_private(&path, token.as_bytes()).map_err(ApplicationStorageError::CannotWriteWebToken)?;
    Ok(token)
}

/// A fresh random secret: the token the UI must present, so other local software can't use
/// this client, or a launch link's nonce.
pub fn fresh() -> String {
    let mut bytes = [0u8; TOKEN_LENGTH];
    default_provider()
        .secure_random
        .fill(&mut bytes)
        .expect("the system random source is available");
    bytes
        .iter()
        .map(|b| TOKEN_ALPHABET[*b as usize % TOKEN_ALPHABET.len()] as char)
        .collect()
}

#[cfg(test)]
mod tests;
