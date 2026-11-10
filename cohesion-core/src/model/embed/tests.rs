use super::*;

#[test]
fn an_excerpt_is_the_first_paragraph_of_body_text() {
    let text = "# Launch plan\n\nShip the **beta** to [ten users](https://example.com)\nby Friday.\n\nThen the rest.";
    assert_eq!(excerpt(text), "Ship the beta to ten users by Friday.");
}

#[test]
fn an_excerpt_skips_code_rules_and_tables() {
    let text =
        "```js\nlet x = 1;\n```\n---\n| a | b |\n\n> - [ ] Call `Ana` about ~~it~~ the *plan*";
    assert_eq!(excerpt(text), "Call Ana about it the plan");
}

#[test]
fn an_excerpt_keeps_a_cohesion_links_text() {
    assert_eq!(
        excerpt("See ![chart](a.png) and [Fix login](cohesion://task/3/42)."),
        "See chart and Fix login."
    );
}

#[test]
fn an_excerpt_of_nothing_is_empty() {
    assert_eq!(excerpt(""), "");
    assert_eq!(excerpt("# Only a heading\n\n"), "");
}

#[test]
fn a_long_excerpt_is_cut_at_a_word_with_an_ellipsis() {
    let text = "word ".repeat(100);
    let cut = excerpt(&text);
    assert!(cut.ends_with("word…"), "{cut}");
    assert!(cut.chars().count() <= EXCERPT_CHARS + 1);
}

#[test]
fn a_numbered_list_item_loses_its_number() {
    assert_eq!(excerpt("1. First\n2) Second"), "First Second");
}

#[test]
fn an_excerpt_passes_over_a_paragraph_repeating_the_title() {
    assert_eq!(
        excerpt_after_title("**Touch and Glow**\n\nA week with it.", "Touch and Glow"),
        "A week with it."
    );
    assert_eq!(
        excerpt_after_title("Touch and Glow", "★ Touch and glow"),
        ""
    );
    assert_eq!(excerpt_after_title("Touch and Glow", ""), "Touch and Glow");
}
