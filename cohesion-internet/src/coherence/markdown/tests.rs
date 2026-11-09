use dom_query::Document;

use super::markdown;
use crate::article_snapshot::Url;
use crate::coherence::rule::Decisions;

/// `html` written as Markdown, as if from a page under
/// `https://example.com/a/`, no rule having decided anything.
fn written(html: &str) -> String {
    let document = Document::from(html);
    let base = Url::parse("https://example.com/a/").expect("the URL parses");
    markdown(&[document.root()], &Decisions::default(), &base)
}

#[test]
fn prose_is_written_as_it_reads() {
    assert_eq!(
        written(r#"<p>Linux 7.4 (beta) is out! It says "hi" to {you}, 5 * 3 times.</p>"#),
        r#"Linux 7.4 (beta) is out! It says "hi" to {you}, 5 * 3 times."#
    );
}

#[test]
fn text_that_would_read_as_markup_is_escaped() {
    for (html, markdown) in [
        ("<p>1986. A good year.</p>", r"1986\. A good year."),
        ("<p># not a heading</p>", r"\# not a heading"),
        ("<p>- not an item</p>", r"\- not an item"),
        ("<p>a *b* c</p>", r"a \*b\* c"),
        ("<p>snake_case and _this_</p>", r"snake_case and \_this\_"),
        ("<p>[x](y) and [sic]</p>", r"[x\](y) and [sic]"),
        ("<p>&lt;div&gt; &amp;amp;</p>", r"\<div> \&amp;"),
        ("<p>`tick`</p>", r"\`tick\`"),
    ] {
        assert_eq!(written(html), markdown, "{html}");
    }
}

#[test]
fn emphasis_keeps_its_spaces_outside() {
    assert_eq!(
        written("<p>A<strong> Note </strong>here, <em> </em>and <b><strong>once</strong></b>.</p>"),
        "A **Note** here, and **once**."
    );
}

#[test]
fn lists_nest_and_number_from_their_start() {
    assert_eq!(
        written("<ul><li>One<ul><li>Inner</li></ul></li><li><p>Two</p></li></ul>"),
        "- One\n  - Inner\n- Two"
    );
    assert_eq!(
        written(r#"<ol start="3"><li>Three</li><li>Four</li></ol>"#),
        "3. Three\n4. Four"
    );
}

#[test]
fn code_keeps_its_text_and_language() {
    assert_eq!(
        written(
            r#"<pre><code class="language-rust">let x = 1;
let y = `a`;
</code></pre>"#
        ),
        "```rust\nlet x = 1;\nlet y = `a`;\n```"
    );
    assert_eq!(written("<p>Run <code>a`b</code>.</p>"), "Run ``a`b``.");
}

#[test]
fn breaks_break_lines_and_two_part_paragraphs() {
    assert_eq!(written("<p>a<br>b</p>"), "a\\\nb");
    assert_eq!(written("<p>a<br><br>b<br></p>"), "a\n\nb");
}

/// A hard break's backslash shows as text unless a line of text follows it
/// in the same paragraph, as when a page puts a no-break space after a
/// `<br>`.
#[test]
fn every_hard_break_has_text_after_it() {
    for html in [
        "<p>a.<br>&nbsp;</p>",
        "<p>a.<br><span>\u{a0}</span></p>",
        "<p>a.<br>&nbsp;<br>b</p>",
        "<p>&nbsp;<br>a.</p>",
        "<ul><li>a.<br>&nbsp;</li></ul>",
        "<blockquote>a.<br>\u{2003}<br>b</blockquote>",
    ] {
        let markdown = written(html);
        let lines: Vec<&str> = markdown.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if line.ends_with('\\') {
                let next = lines
                    .get(i + 1)
                    .map(|next| next.trim_start_matches(['>', ' ']));
                assert!(
                    next.is_some_and(|next| !next.trim().is_empty()),
                    "{html} => {markdown:?}"
                );
            }
        }
        assert!(markdown.contains("a."), "{html} => {markdown:?}");
    }
}

#[test]
fn links_and_images_are_absolute_and_tooltips_dropped() {
    assert_eq!(
        written(
            r#"<p><a href="x" title="Permanent link">X</a> <img src="/i.png" alt="An [odd] one" title="Tip"></p>"#
        ),
        r"[X](https://example.com/a/x) ![An \[odd\] one](https://example.com/i.png)"
    );
    assert_eq!(
        written(r#"<p><a href="javascript:void(0)">Menu</a> <a href="/y"> </a></p>"#),
        "Menu"
    );
}

#[test]
fn tables_quotes_and_rules() {
    assert_eq!(
        written(
            "<table><tr><th>Name</th><th>Size</th></tr><tr><td>a|b</td><td>2</td></tr></table>"
        ),
        "| Name | Size |\n| --- | --- |\n| a\\|b | 2 |"
    );
    assert_eq!(
        written("<blockquote><p>a</p><p>b</p></blockquote><hr>"),
        "> a\n>\n> b\n\n---"
    );
}

#[test]
fn what_no_reader_sees_is_left_out() {
    assert_eq!(
        written("<p>Kept<script>var x = 1;</script><button>Share</button>.</p><style>p {}</style>"),
        "Kept."
    );
}
