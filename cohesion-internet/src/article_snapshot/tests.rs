use super::article_url;

#[test]
fn only_web_addresses_can_be_saved() {
    assert!(article_url(" https://example.com/a ").is_some());
    assert!(article_url("http://example.com").is_some());
    for rejected in ["file:///etc/passwd", "example.com", "javascript:alert(1)"] {
        assert!(article_url(rejected).is_none(), "{rejected} accepted");
    }
}
