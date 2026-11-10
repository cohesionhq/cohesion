use super::{ArticleFetchAccess, article_url, snapshot_address};
use cohesion_core::model::ArticleFetchFailure;

#[test]
fn only_web_addresses_can_be_saved() {
    assert!(article_url(" https://example.com/a ").is_some());
    assert!(article_url("http://example.com").is_some());
    for rejected in ["file:///etc/passwd", "example.com", "javascript:alert(1)"] {
        assert!(article_url(rejected).is_none(), "{rejected} accepted");
    }
}

#[test]
fn a_stored_address_that_is_not_a_web_address_is_unreachable() {
    let fetched = snapshot_address(ArticleFetchAccess::granted(), "mailto:ada@bakery.example");
    assert!(matches!(
        fetched,
        Err(ArticleFetchFailure::Unreachable { .. })
    ));
}
