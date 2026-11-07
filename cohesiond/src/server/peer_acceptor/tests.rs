use std::time::Instant;

use super::UnknownPeers;

#[test]
fn an_unknown_fingerprint_is_remembered_until_it_expires() {
    let unknown = UnknownPeers::default();
    let start = Instant::now();
    unknown.remember("a".to_string(), unknown.epoch(), start);
    assert!(unknown.contains("a", start));
    assert!(!unknown.contains("b", start));
    assert!(!unknown.contains("a", start + UnknownPeers::TTL));
}

#[test]
fn a_registered_fingerprint_is_not_stuck_in_the_cache() {
    let unknown = UnknownPeers::default();
    let now = Instant::now();
    unknown.remember("a".to_string(), unknown.epoch(), now);
    unknown.forget("a");
    assert!(!unknown.contains("a", now));
}

#[test]
fn a_lookup_that_began_before_a_registration_is_not_remembered() {
    let unknown = UnknownPeers::default();
    let now = Instant::now();
    let epoch = unknown.epoch();
    unknown.forget("a");
    unknown.remember("a".to_string(), epoch, now);
    assert!(!unknown.contains("a", now));
}

#[test]
fn the_cache_stays_bounded() {
    let unknown = UnknownPeers::default();
    let now = Instant::now();
    for n in 0..UnknownPeers::CAPACITY + 10 {
        unknown.remember(n.to_string(), unknown.epoch(), now);
    }
    assert_eq!(unknown.state().seen.len(), UnknownPeers::CAPACITY);
}
