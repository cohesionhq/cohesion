//! Secrets the server holds and commands check against: the setup key a new
//! client must present, which is only valid during a pairing window.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// How long a setup key stays valid if nobody registers with it.
const PAIRING_WINDOW: Duration = Duration::from_secs(10 * 60);

/// Server-held secrets that commands may check against. Clones share the
/// same state, so a request handler can open a pairing window that the
/// database worker's registration command then closes.
#[derive(Clone, Default)]
pub struct Secrets {
    pairing: Arc<Mutex<PairingWindow>>,
}

impl Secrets {
    /// Secrets with no pairing window open: no one can register until
    /// [`begin_pairing`](Self::begin_pairing).
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens a pairing window and returns its setup key, replacing any
    /// window already open. The key works for one registration, for ten
    /// minutes.
    pub fn begin_pairing(&self) -> String {
        self.window().open(Instant::now())
    }

    /// Whether `key` is the open window's setup key, compared in constant time.
    pub fn verify_setup_key(&self, key: &str) -> bool {
        self.window().admits(key, Instant::now())
    }

    /// Closes the pairing window: its key has been used.
    pub fn end_pairing(&self) {
        self.window().close();
    }

    fn window(&self) -> MutexGuard<'_, PairingWindow> {
        self.pairing.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The setup key a new client may register with, and when it stops working.
/// Time is passed in, so tests need not sleep.
#[derive(Default)]
struct PairingWindow {
    open: Option<(String, Instant)>,
}

impl PairingWindow {
    /// A fresh key, valid until `now` plus the window.
    fn open(&mut self, now: Instant) -> String {
        let key = random_string(32);
        self.open = Some((key.clone(), now + PAIRING_WINDOW));
        key
    }

    /// Whether `key` is the window's key and `now` is before its deadline.
    fn admits(&self, key: &str, now: Instant) -> bool {
        self.open.as_ref().is_some_and(|(open_key, deadline)| {
            let same = aws_lc_rs::constant_time::verify_slices_are_equal(
                key.as_bytes(),
                open_key.as_bytes(),
            )
            .is_ok();
            same && now < *deadline
        })
    }

    fn close(&mut self) {
        self.open = None;
    }
}

/// Generates a cryptographically secure random alphanumeric string.
pub(crate) fn random_string(len: usize) -> String {
    use aws_lc_rs::rand::{SecureRandom, SystemRandom};

    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    // Largest multiple of ALPHABET.len() that fits in a byte; bytes at or above
    // this are rejected to avoid modulo bias.
    const LIMIT: u8 = (256 / ALPHABET.len() * ALPHABET.len()) as u8;

    let rng = SystemRandom::new();
    let mut out = String::with_capacity(len);
    let mut buf = [0u8; 64];
    while out.len() < len {
        rng.fill(&mut buf).expect("system RNG failed");
        for &b in buf.iter().filter(|&&b| b < LIMIT) {
            if out.len() == len {
                break;
            }
            out.push(ALPHABET[b as usize % ALPHABET.len()] as char);
        }
    }
    out
}

#[cfg(test)]
mod tests;
