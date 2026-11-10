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
