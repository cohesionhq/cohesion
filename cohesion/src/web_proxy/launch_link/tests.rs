use std::time::{Duration, Instant};

use super::{LINK_LIFETIME, LaunchLink};

#[test]
fn a_nonce_is_redeemed_once() {
    let link = LaunchLink::new("secret".into());
    let now = Instant::now();
    let nonce = link.mint(now);
    assert_eq!(link.redeem("wrong", now), None);
    assert_eq!(link.redeem(&nonce, now), Some("secret"));
    assert_eq!(link.redeem(&nonce, now), None);
}

#[test]
fn a_nonce_expires() {
    let link = LaunchLink::new("secret".into());
    let now = Instant::now();
    let nonce = link.mint(now);
    assert_eq!(link.redeem(&nonce, now + LINK_LIFETIME), None);
    // Spent by the late attempt, too.
    assert_eq!(link.redeem(&nonce, now + Duration::from_secs(1)), None);
}

#[test]
fn minting_again_replaces_the_nonce() {
    let link = LaunchLink::new("secret".into());
    let now = Instant::now();
    let first = link.mint(now);
    let second = link.mint(now);
    assert_eq!(link.redeem(&first, now), None);
    assert_eq!(link.redeem(&second, now), Some("secret"));
}
