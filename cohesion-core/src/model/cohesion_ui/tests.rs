use serde_json::json;

use super::{Block, Component, MAX_BLOCKS, MAX_DEPTH, MAX_TEXT_CHARS};

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

fn text(text: &str) -> serde_json::Value {
    json!({ "kind": "text", "text": text })
}

#[test]
fn an_interface_may_hold_a_bounded_number_of_blocks() {
    let blocks = |n: usize| json!({ "blocks": vec![text("."); n] });
    assert!(Component::from_script(blocks(MAX_BLOCKS)).is_ok());
    assert!(Component::from_script(blocks(MAX_BLOCKS + 1)).is_err());

    // Blocks inside an overview count too.
    let overviews = |n: usize| json!({ "blocks": vec![json!({ "kind": "overview", "title": "T", "child": text(".") }); n] });
    assert!(Component::from_script(overviews(MAX_BLOCKS / 2)).is_ok());
    assert!(Component::from_script(overviews(MAX_BLOCKS / 2 + 1)).is_err());
}

#[test]
fn blocks_nest_only_so_deep() {
    let nested = |depth: usize| {
        let mut block = text(".");
        for _ in 1..depth {
            block = json!({ "kind": "overview", "title": "T", "child": block });
        }
        json!({ "blocks": [block] })
    };
    assert!(Component::from_script(nested(MAX_DEPTH)).is_ok());
    assert!(Component::from_script(nested(MAX_DEPTH + 1)).is_err());
    // Far deeper than any reader of the wire would follow.
    assert!(Component::from_script(nested(1_000)).is_err());
}

#[test]
fn text_is_bounded_wherever_it_sits() {
    let long = "é".repeat(MAX_TEXT_CHARS + 1);
    let fits = "é".repeat(MAX_TEXT_CHARS);
    assert!(Component::from_script(json!({ "blocks": [text(&fits)] })).is_ok());
    for block in [
        text(&long),
        json!({ "kind": "heading", "level": 1, "text": long }),
        json!({ "kind": "checkbox", "checked": true, "text": long }),
        json!({ "kind": "overview", "title": long, "child": text(".") }),
        json!({ "kind": "link", "text": long, "destination": "https://example.com" }),
        json!({ "kind": "link", "text": "x", "destination": format!("https://{long}") }),
    ] {
        assert!(Component::from_script(json!({ "blocks": [block] })).is_err());
    }
}
