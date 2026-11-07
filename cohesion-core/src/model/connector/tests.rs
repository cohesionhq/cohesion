use super::Credential;

const NOW: i64 = 1_700_000_000;

fn expiring(expires_at: Option<i64>) -> Credential {
    Credential {
        authorization: "token".to_string(),
        refresh: None,
        expires_at,
    }
}

#[test]
fn credentials_without_an_expiry_are_never_refreshed() {
    assert!(!expiring(None).needs_refresh(NOW));
}

#[test]
fn credentials_are_refreshed_within_the_margin() {
    assert!(!expiring(Some(NOW + 61)).needs_refresh(NOW));
    assert!(expiring(Some(NOW + 60)).needs_refresh(NOW));
    assert!(expiring(Some(NOW)).needs_refresh(NOW));
    assert!(expiring(Some(NOW - 3600)).needs_refresh(NOW));
}
