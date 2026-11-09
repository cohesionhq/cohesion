use serde_json::json;

use super::{Block, Component};

#[test]
fn every_kind_of_block_reads_and_nests() {
    let component = Component::from_script(json!({
        "blocks": [
            { "kind": "heading", "level": 1, "text": "Launch" },
            { "kind": "text", "text": "Ready." },
            { "kind": "checkbox", "checked": true },
            { "kind": "link", "text": "Open", "destination": "https://example.com" },
            { "kind": "overview", "title": "Done", "child": { "kind": "checkbox", "checked": false } },
        ]
    }))
    .expect("a valid interface");
    assert_eq!(component.blocks.len(), 5);
    assert!(
        matches!(&component.blocks[4], Block::Overview(o) if matches!(*o.child, Block::Checkbox(_)))
    );
}

#[test]
fn malformed_interfaces_are_refused() {
    for value in [
        json!(null),
        json!({ "blocks": [{ "kind": "table" }] }),
        json!({ "blocks": [{ "kind": "text" }] }),
        json!({ "blocks": [{ "kind": "heading", "level": 4, "text": "Too small" }] }),
        json!({ "blocks": [{ "kind": "heading", "level": 0, "text": "Too large" }] }),
    ] {
        assert!(Component::from_script(value).is_err());
    }
}

#[test]
fn links_open_only_web_and_mail_addresses() {
    let link = |destination: &str| {
        json!({ "blocks": [{ "kind": "overview", "title": "Link", "child":
            { "kind": "link", "text": "Open", "destination": destination } }] })
    };
    assert!(Component::from_script(link("HTTPS://example.com")).is_ok());
    assert!(Component::from_script(link("mailto:ada@example.com")).is_ok());
    assert!(Component::from_script(link("javascript:alert(1)")).is_err());
    assert!(Component::from_script(link("cohesion://task/1/2")).is_err());
}
