use super::fallback_article_title;

#[test]
fn a_fallback_title_is_the_last_path_component_else_the_host() {
    assert_eq!(
        fallback_article_title("https://example.com/2024/03/sourdough-notes/?utm=x#top"),
        "sourdough-notes"
    );
    assert_eq!(
        fallback_article_title("https://example.com/"),
        "example.com"
    );
    assert_eq!(fallback_article_title("https://example.com"), "example.com");
}
