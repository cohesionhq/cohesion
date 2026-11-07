use super::accepted_icon;

fn accepted(icon: &str) -> bool {
    accepted_icon(Some(icon.to_string())).is_some()
}

#[test]
fn keeps_data_image_and_https_urls() {
    assert!(accepted("data:image/png;base64,iVBORw0KGgo="));
    assert!(accepted("https://example.com/icon.png"));
    assert_eq!(accepted_icon(None), None);
}

#[test]
fn drops_other_schemes_and_oversize_icons() {
    assert!(!accepted("http://example.com/icon.png"));
    assert!(!accepted("javascript:alert(1)"));
    assert!(!accepted("data:text/html,<script>alert(1)</script>"));
    let oversize = format!(
        "data:image/png;base64,{}",
        "A".repeat(super::MAX_ICON_BYTES)
    );
    assert!(!accepted(&oversize));
}
