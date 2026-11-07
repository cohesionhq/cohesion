use std::time::Instant;

use super::{PAIRING_WINDOW, PairingWindow, Secrets};

#[test]
fn nothing_registers_until_pairing_begins() {
    let secrets = Secrets::new();
    assert!(!secrets.verify_setup_key(""));
    let key = secrets.begin_pairing();
    assert_eq!(key.len(), 32);
    assert!(secrets.verify_setup_key(&key));
    assert!(!secrets.verify_setup_key("wrong"));
}

#[test]
fn a_key_dies_when_pairing_ends_or_a_new_window_opens() {
    let secrets = Secrets::new();
    let first = secrets.begin_pairing();
    secrets.end_pairing();
    assert!(!secrets.verify_setup_key(&first));

    let second = secrets.begin_pairing();
    let third = secrets.begin_pairing();
    assert!(!secrets.verify_setup_key(&second));
    assert!(secrets.verify_setup_key(&third));
}

#[test]
fn a_key_expires_with_its_window() {
    let start = Instant::now();
    let mut window = PairingWindow::default();
    let key = window.open(start);
    assert!(window.admits(&key, start + PAIRING_WINDOW / 2));
    assert!(!window.admits(&key, start + PAIRING_WINDOW));
}
