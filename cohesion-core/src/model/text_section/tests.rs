use super::*;

fn markdown(text: &str) -> TextPiece {
    TextPiece::Markdown(text.to_string())
}

fn task(label: &str, ws: i64, id: i64) -> TextPiece {
    TextPiece::Embed {
        link: ItemLink::Task {
            workspace_id: Some(ws),
            id,
        },
        href: format!("cohesion://task/{ws}/{id}"),
        label: label.to_string(),
    }
}

// The web UI's `textPieces` (web/src/lib/format/itemLinks.test.ts) splits these
// same texts the same way.

#[test]
fn a_link_alone_on_its_line_is_an_embed_between_markdown() {
    assert_eq!(
        text_pieces("# Plan\n\nFirst:\n\n[Fix login](cohesion://task/3/42)\n\nThen the rest."),
        vec![
            markdown("# Plan\n\nFirst:"),
            task("Fix login", 3, 42),
            markdown("Then the rest.")
        ]
    );
}

#[test]
fn a_link_in_a_sentence_or_list_stays_markdown() {
    let text = "See [Fix login](cohesion://task/3/42) today.\n- [Fix login](cohesion://task/3/42)";
    assert_eq!(text_pieces(text), vec![markdown(text)]);
}

#[test]
fn a_link_in_code_stays_markdown() {
    for text in [
        "```\n[Fix login](cohesion://task/3/42)\n```",
        "    [Fix login](cohesion://task/3/42)",
        "\t[Fix login](cohesion://task/3/42)",
    ] {
        assert_eq!(text_pieces(text), vec![markdown(text)], "{text}");
    }
}

#[test]
fn a_link_to_anything_but_an_item_stays_markdown() {
    for text in [
        "[Example](https://example.com)",
        "[Contact](cohesion://contact/3)",
        "![Chart](cohesion://task/3/42)",
    ] {
        assert_eq!(text_pieces(text), vec![markdown(text)], "{text}");
    }
}

#[test]
fn a_sentence_whose_last_link_ends_the_line_stays_markdown() {
    let text = "[Spec](https://example.com) blocks [Fix login](cohesion://task/5/42)";
    assert_eq!(text_pieces(text), vec![markdown(text)]);
}

#[test]
fn two_links_on_one_line_stay_markdown() {
    for text in [
        "[A](cohesion://task/3/1) [B](cohesion://task/3/2)",
        "[A](cohesion://task/3/1)[B](cohesion://task/3/2)",
    ] {
        assert_eq!(text_pieces(text), vec![markdown(text)], "{text}");
    }
}

#[test]
fn an_escaped_bracket_stays_in_the_label() {
    assert_eq!(
        text_pieces("[Fix \\] login \\[2\\]](cohesion://task/3/42)"),
        vec![task("Fix ] login [2]", 3, 42)]
    );
    // The same label unescaped ends at its first `]`, and what follows is text.
    let text = "[Fix ] login](cohesion://task/3/42)";
    assert_eq!(text_pieces(text), vec![markdown(text)]);
}

#[test]
fn a_bare_link_stays_markdown() {
    for text in [
        "cohesion://task/3/42",
        "<cohesion://task/3/42>",
        "(cohesion://task/3/42)",
    ] {
        assert_eq!(text_pieces(text), vec![markdown(text)], "{text}");
    }
}

#[test]
fn embeds_side_by_side_need_no_markdown_between() {
    assert_eq!(
        text_pieces("  [A](cohesion://task/3/1)  \n[B](cohesion://task/3/2)\n\n"),
        vec![task("A", 3, 1), task("B", 3, 2)]
    );
}

#[test]
fn nothing_splits_into_nothing() {
    assert_eq!(text_pieces(""), vec![]);
    assert_eq!(text_pieces("\n \n"), vec![]);
}

/// The cases the web UI runs too (`item_link/link_cases.json`): the lines
/// that are embeds here are the ones that must be there.
#[test]
fn the_shared_text_cases_hold() {
    let shared: serde_json::Value =
        serde_json::from_str(include_str!("../item_link/link_cases.json")).unwrap();
    for case in shared["texts"].as_array().unwrap() {
        let text = case["text"].as_str().unwrap();
        let pieces: Vec<serde_json::Value> = text_pieces(text)
            .into_iter()
            .map(|piece| match piece {
                TextPiece::Markdown(text) => serde_json::json!({ "markdown": text }),
                TextPiece::Embed { href, label, .. } => {
                    serde_json::json!({ "embed": { "label": label, "href": href } })
                }
            })
            .collect();
        assert_eq!(serde_json::Value::Array(pieces), case["pieces"], "{text:?}");
    }
}
