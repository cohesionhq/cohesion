use super::{Article, Html};
use crate::article_snapshot::Url;

fn url() -> Url {
    Url::parse("https://example.com/news/today").expect("the URL parses")
}

/// `html` read by Coherence as if fetched from a page under
/// `https://example.com/`.
fn read(html: &str, limit: usize) -> Article {
    super::read(Html::Page(html), &url(), limit)
}

/// `html` read by Coherence as a feed's content for a page under
/// `https://example.com/`.
fn content(html: &str) -> String {
    super::read(Html::Content(html), &url(), 1 << 16).text
}

const PAGE: &str = r#"<!doctype html>
<html>
<head>
  <title> Bread &amp; butter
    notes </title>
  <style>body { color: red; }</style>
  <script>var tracking = "pixel";</script>
</head>
<body>
  <h1>Sourdough</h1>
  <p>Feed the starter the night before.</p>
  <script>document.write("injected");</script>
  <p>Bake at 250°C.</p>
</body>
</html>"#;

/// An article page wrapped in the chrome a site puts around one.
const ARTICLE_PAGE: &str = r#"<!doctype html>
<html>
<head><title>Vibe coding is turning the gear world upside down | The Gazette</title></head>
<body>
  <nav><a href="/">Skip to main content</a> <a href="/tech">Tech</a> <a href="/login">Login</a></nav>
  <article>
    <h1>Vibe coding is turning the gear world upside down</h1>
    <p>If you spend any time on <a href="/r/synth">synth forums</a> you have noticed
    the explosion of custom firmware for hardware synthesizers, samplers and
    grooveboxes. What unites almost all of them is that they were made possible by
    increasingly capable code-writing tools that let hobbyists finish in days what
    once took a specialist months.</p>
    <p>These firmwares were exceedingly rare before, and required both deep
    expertise and an incredible investment of time. It could take weeks to add a
    feature like velocity sensitivity to a budget synth. Now hackers turn cheap
    synths into full grooveboxes over a weekend, and share the result the same
    night.</p>
  </article>
  <footer>© 2026 The Gazette · <a href="/privacy">Privacy policy</a></footer>
</body>
</html>"#;

#[test]
fn a_streamed_boundary_is_read_though_its_script_has_not_moved_it() {
    let extracted = read(
        r#"<html><head><meta name="description" content="A site about tech."></head><body>
        <main><template id="B:0"></template></main>
        <div hidden="">A hidden note that stays hidden, however long it may be.</div>
        <div hidden id="S:0"><article>
          <p>The review a streaming page sends after its shell, for a script to move into place.</p>
        </article></div>
        <script>$RC("B:0","S:0")</script>
        </body></html>"#,
        1024,
    );
    assert_eq!(
        extracted.text,
        "The review a streaming page sends after its shell, for a script to move into place."
    );
}

#[test]
fn extracting_keeps_the_content_as_markdown_and_the_title() {
    let extracted = read(PAGE, 1024);
    assert_eq!(
        extracted.metadata.title.as_deref(),
        Some("Bread & butter notes")
    );
    assert!(extracted.text.contains("## Sourdough"));
    assert!(extracted.text.contains("Feed the starter the night before"));
    assert!(extracted.text.contains("Bake at 250°C"));
    for hidden in ["color: red", "tracking", "injected"] {
        assert!(!extracted.text.contains(hidden), "{hidden} leaked");
    }
    assert!(!extracted.truncated);
}

#[test]
fn an_article_page_keeps_its_article_and_drops_the_chrome() {
    let extracted = read(ARTICLE_PAGE, 1 << 16);
    assert_eq!(
        extracted.metadata.title.as_deref(),
        Some("Vibe coding is turning the gear world upside down")
    );
    assert!(extracted.text.contains("explosion of custom firmware"));
    assert!(extracted.text.contains("share the result the same"));
    assert!(
        extracted
            .text
            .contains("[synth forums](https://example.com/r/synth)"),
        "link not absolute Markdown"
    );
    for chrome in ["Skip to main content", "Login", "Privacy policy", "© 2026"] {
        assert!(!extracted.text.contains(chrome), "{chrome} kept");
    }
}

#[test]
fn an_article_page_drops_its_share_widget() {
    let html = ARTICLE_PAGE.replace(
        "</article>",
        r#"<div class="nr-article-share"><div>Share article</div>
        <ul><li><button>Share via Facebook</button></li></ul></div></article>"#,
    );
    let extracted = read(&html, 1 << 16);
    assert!(extracted.text.contains("explosion of custom firmware"));
    assert!(
        !extracted.text.contains("Share article"),
        "share widget kept"
    );
}

#[test]
fn a_figure_keeps_its_image_whatever_its_markup_is_named() {
    let html = ARTICLE_PAGE.replace(
        "</article>",
        r#"<figure class="image-sharesheet-gallery"><div class="image-sharesheet">
        <div class="image-resource"><img src="/synth.jpg" alt="A modded groovebox."></div>
        </div></figure></article>"#,
    );
    let extracted = read(&html, 1 << 16);
    assert!(
        extracted.text.contains("](https://example.com/synth.jpg)"),
        "image dropped"
    );
}

#[test]
fn a_gallery_drops_its_tab_strip() {
    let html = ARTICLE_PAGE.replace(
        "</article>",
        r##"<div><ul role="tablist"><li><a href="#slide-1">First slide</a></li>
        <li><a href="#slide-2">Second slide</a></li></ul></div></article>"##,
    );
    let extracted = read(&html, 1 << 16);
    assert!(extracted.text.contains("explosion of custom firmware"));
    assert!(!extracted.text.contains("slide"), "tab strip kept");
}

#[test]
fn controls_and_chrome_go_by_their_role() {
    let html = ARTICLE_PAGE
        .replace(
            "<article>",
            r#"<article><div role="banner">Gazette masthead</div>
            <div role="search">Search the Gazette</div>"#,
        )
        .replace(
            "</article>",
            r#"<p>Filed under <span role="button">Follow Terrence</span> synths.</p>
            <div role="contentinfo">Gazette staff directory</div></article>"#,
        );
    let extracted = read(&html, 1 << 16);
    assert!(extracted.text.contains("explosion of custom firmware"));
    assert!(extracted.text.contains("Filed under"));
    for chrome in [
        "masthead",
        "Search the",
        "Follow Terrence",
        "staff directory",
    ] {
        assert!(!extracted.text.contains(chrome), "{chrome} kept");
    }
}

#[test]
fn a_page_with_no_text_still_has_a_title() {
    let extracted = read(
        "<html><head><title>T</title></head><body></body></html>",
        1024,
    );
    assert_eq!(extracted.metadata.title.as_deref(), Some("T"));
    assert_eq!(extracted.text, "");
}

#[test]
fn lists_read_as_markdown_lists() {
    let html = "<article><p>What the dough needs, in order of weight.</p>\
        <ul><li>Flour</li><li>Water</li></ul></article>";
    let extracted = read(html, 1024);
    assert!(extracted.text.contains("- Flour"));
    assert!(extracted.text.contains("- Water"));
}

#[test]
fn extracting_cuts_long_text_on_a_character_boundary() {
    // Each "é" is two bytes, so an odd limit falls inside one.
    let html = format!("<p>{}</p>", "é".repeat(100));
    let extracted = read(&html, 51);
    assert!(extracted.truncated);
    assert_eq!(extracted.text, "é".repeat(25));
    assert_eq!(extracted.metadata.title, None);
}

/// An article followed by the membership pitch a site appends, marked up
/// as MacStories does, beside prose that merely mentions subscriptions.
const PITCH_PAGE: &str = r#"<!doctype html>
<html><head><title>Apple announces an event</title></head>
<body>
  <article class="post-content subscription-notes">
    <h1>Apple announces an event</h1>
    <p>Apple today announced a special event for October 28, called Pro and Tell,
    streaming from Cupertino for anyone who wants to watch the keynote live.</p>
    <p>The company says the event will focus on its professional hardware and the
    software that runs on it, with details promised for those who tune in.</p>
    <p>Invitations went out this morning, and the stream begins at ten in the
    morning Pacific time on the company's website and its television app.</p>
    <footer class="post-footer"><a href="/tag/apple">Apple</a></footer>
  </article>
  <div class='club-after-content'>
    <div class='club-call-to-action'>
      <img class="club-logo" src="/logo-shape-gold.svg" alt="Club MacStories">
      <a class="club-button" href="/club">Join Now</a>
    </div>
    <div class='club-description'>
      <h3 class='club-title'>Access Extra Content and Perks</h3>
      <p>Founded in 2015, Club MacStories has delivered exclusive content every
      week for over a decade.</p>
    </div>
  </div>
</body></html>"#;

#[test]
fn a_promotional_block_is_dropped_and_the_article_kept() {
    let text = read(PITCH_PAGE, 4096).text;
    for kept in [
        "Pro and Tell",
        "professional hardware",
        "Invitations went out",
    ] {
        assert!(text.contains(kept), "{kept} was dropped");
    }
    for dropped in ["Club MacStories", "Access Extra Content", "Join Now"] {
        assert!(!text.contains(dropped), "{dropped} leaked");
    }
}

#[test]
fn a_promotional_block_is_dropped_from_feed_html() {
    let text = content(
        r#"<p>The article.</p><div class="newsletter-signup"><p>Subscribe to our list</p></div>
        <section class="related-posts"><a href="/x">Another post</a></section>"#,
    );
    assert!(text.contains("The article"));
    assert!(!text.contains("Subscribe to our list"));
    assert!(!text.contains("Another post"));
}

#[test]
fn an_author_box_and_its_avatar_are_dropped() {
    // As MacStories marks up the box that follows each article.
    let html = ARTICLE_PAGE.replace(
        "</article>",
        r#"<div class="post-author-info"><div class="author-avatar">
        <img alt="" src="https://secure.gravatar.com/avatar/a5?s=40&amp;d=mm" class="avatar" height="40" width="40"></div>
        <div class="author-info"><a href="/author/terrence/" class="author-name">Terrence Example</a>
        <div class="author-bio">Terrence is the Gazette's synth editor, has written about
        hardware since joining in 2015, and co-hosts two podcasts: one on the weird web,
        one on the games we take with us.</div></div></div>
        </article>"#,
    );
    let text = read(&html, 1 << 16).text;
    assert!(text.contains("share the result the same"));
    for dropped in ["gravatar", "Terrence Example", "synth editor"] {
        assert!(!text.contains(dropped), "{dropped} kept");
    }
}

#[test]
fn icons_pixels_and_avatars_are_dropped_and_pictures_kept() {
    let html = ARTICLE_PAGE.replace(
        "</article>",
        r#"<p><img alt="TECH" src="/resources/categories/tech.webp" width="100" height="100">
        <img src="https://pixel.example/t.gif" width="1" height="1">
        <img alt="Ada" src="https://avatars.example.com/u/1.png">
        <img alt="A modded groovebox." src="/synth.jpg" width="1200" height="800"></p></article>"#,
    );
    let text = read(&html, 1 << 16).text;
    assert!(text.contains("![A modded groovebox.](https://example.com/synth.jpg)"));
    for dropped in ["tech.webp", "pixel.example", "avatars.example"] {
        assert!(!text.contains(dropped), "{dropped} kept");
    }
}

#[test]
fn a_linked_image_keeps_its_image() {
    let html = ARTICLE_PAGE.replace(
        "</article>",
        r#"<p><a href="/synth-full.jpg"><img alt="" src="/synth.jpg"></a></p></article>"#,
    );
    let text = read(&html, 1 << 16).text;
    assert!(
        text.contains("![](https://example.com/synth.jpg)"),
        "image dropped"
    );
}

#[test]
fn alt_text_written_as_html_reads_as_text() {
    let html = ARTICLE_PAGE.replace(
        "</article>",
        r#"<p><img alt="&lt;em&gt;The [rear] view&lt;/em&gt;" src="/rear.jpg"></p></article>"#,
    );
    let text = read(&html, 1 << 16).text;
    assert!(text.contains(r"![The \[rear\] view](https://example.com/rear.jpg)"));
}

#[test]
fn a_byline_is_left_out_of_the_text() {
    let text = content(
        r#"<p><time>Monday October 19, 2026</time> by
        <a href="/author/terrence" rel="author">Terrence Example</a></p>
        <div class="post-dateline">Filed October 19</div>
        <p>The article, which links <a href="/terrence">Terrence</a> by name.</p>"#,
    );
    assert!(text.contains("The article, which links"));
    assert!(!text.contains("October 19"), "byline kept");
}

#[test]
fn a_byline_reads_as_the_names_alone() {
    let author = |line: &str| {
        let meta = ARTICLE_PAGE.replace(
            "</head>",
            &format!(r#"<meta name="author" content="{line}"></head>"#),
        );
        read(&meta, 1 << 16).metadata.author
    };
    assert_eq!(
        author("By Terrence Example").as_deref(),
        Some("Terrence Example")
    );
    assert_eq!(
        author(
            "Written by Terrence Example in Synths on 19 October 2026 at 05:38 PM EDT. 4 Comments"
        ),
        None
    );
}

#[test]
fn the_date_comes_from_json_ld_in_a_graph() {
    let html = ARTICLE_PAGE.replace(
        "</head>",
        r#"<script type="application/ld+json">{"@context":"https://schema.org","@graph":[
        {"@type":"Article","headline":"Vibe coding","datePublished":"2026-10-19T13:34:42+00:00"},
        {"@type":"WebSite","name":"The Gazette"}]}</script></head>"#,
    );
    assert_eq!(
        read(&html, 1 << 16)
            .metadata
            .published
            .map(|published| published.at),
        Some(1_792_416_882)
    );
}

#[test]
fn a_section_titled_as_other_posts_or_a_pitch_is_dropped() {
    let html = ARTICLE_PAGE.replace(
        "</article>",
        r#"<h2>Related work</h2><p>Earlier firmware projects paved the way for this one.</p>
        <h2 id="newsletter">Newsletter</h2>
        <p>Enter your email address to follow this blog.</p>
        <h2>The next step</h2><p>Hackers now port the firmware to other boxes.</p>
        </article><div data-track="popular-stories"><h2>Popular Stories</h2>
        <div><h3><a href="/a">Another story</a></h3><p>Its excerpt.</p></div></div>"#,
    );
    let text = read(&html, 1 << 16).text;
    for kept in [
        "Related work",
        "Earlier firmware",
        "The next step",
        "port the firmware",
    ] {
        assert!(text.contains(kept), "{kept} dropped");
    }
    for dropped in [
        "Newsletter",
        "Enter your email",
        "Popular Stories",
        "Another story",
    ] {
        assert!(!text.contains(dropped), "{dropped} kept");
    }
}

#[test]
fn a_gallery_image_named_as_media_is_kept() {
    let html = ARTICLE_PAGE.replace(
        "</article>",
        r#"<div class="gallery"><div class="duet--media--content-warning"><div class="image">
        <img alt="The rear." src="/rear.jpg"></div></div><p>The rear of the unit.</p></div></article>"#,
    );
    let text = read(&html, 1 << 16).text;
    assert!(
        text.contains("](https://example.com/rear.jpg)"),
        "image dropped"
    );
}

#[test]
fn feed_html_drops_its_tags_and_keeps_bold_tight() {
    let text = content(
        r#"<p><strong> Curated by Felix </strong></p><p>The article.</p>
        <p>Tags: <a href="/tags/a">datasette</a>, <a href="/tags/b">llm</a></p>
        <div class="linkback">Related Forum: <a href="/forum">iPhone</a></div>"#,
    );
    assert!(text.contains("**Curated by Felix**"));
    assert!(text.contains("The article."));
    for dropped in ["Tags", "datasette", "Related Forum"] {
        assert!(!text.contains(dropped), "{dropped} kept");
    }
}

#[test]
fn a_page_whose_script_writes_it_reads_as_its_description() {
    let html = r#"<html><head><title>Ada on Posts</title>
        <meta property="og:description" content="Ovens are hot.&#10;&#10;Bread is good.">
        </head><body><div id="root"></div></body></html>"#;
    assert_eq!(read(html, 1024).text, "Ovens are hot.\n\nBread is good.");
}

#[test]
fn coherence_reads_metadata_from_meta_tags() {
    let html = ARTICLE_PAGE.replace(
        "</head>",
        r#"<meta property="og:title" content="Vibe coding">
        <meta property="og:site_name" content="The Gazette">
        <meta property="article:published_time" content="2026-10-19T13:34:42+00:00">
        </head>"#,
    );
    let article = read(&html, 1 << 16);
    assert_eq!(article.metadata.title.as_deref(), Some("Vibe coding"));
    assert_eq!(
        article.metadata.published.map(|published| published.at),
        Some(1_792_416_882)
    );
}

#[test]
fn the_title_without_the_site_name() {
    let html = r#"<html><head><title>Sourdough | The Gazette</title>
        <meta property="og:site_name" content="The Gazette"></head>
        <body><p>Feed the starter the night before, and bake it in the morning.</p></body></html>"#;
    assert_eq!(
        read(html, 1024).metadata.title.as_deref(),
        Some("Sourdough")
    );
}

#[test]
fn entries_that_score_alike_are_read_together() {
    let entry = |n: u32| {
        format!(
            r#"<div class="entry"><h2>Release {n}</h2><div class="prose"><p>Release {n} brings
            faster sync, a quieter fan curve, and fixes for the export flow, the import flow,
            and the settings page, which now remembers where you left it.</p></div></div>"#
        )
    };
    let html = format!(
        r#"<html><body><nav><a href="/">Home</a></nav><main><div class="list">{}</div></main>
        <aside><p>Elsewhere, a short note about something else entirely, with a link.</p></aside>
        </body></html>"#,
        (1..=5).map(entry).collect::<String>()
    );
    let text = read(&html, 1 << 16).text;
    for n in 1..=5 {
        assert!(
            text.contains(&format!("## Release {n}")),
            "release {n} missing: {text}"
        );
    }
}

#[test]
fn a_video_facade_becomes_a_link_to_the_video() {
    let html = ARTICLE_PAGE.replace(
        "</article>",
        r#"<div class="yt-facade" data-yt-id="1XxNXkOHKZw"><img src="https://i.ytimg.com/vi/1XxNXkOHKZw/maxresdefault.jpg" alt="maxresdefault">
        <a href="https://www.youtube.com/channel/UC1">Apple Music</a><a href="https://www.youtube.com/watch?v=1XxNXkOHKZw">Watch on YouTube</a></div></article>"#,
    );
    let text = read(&html, 1 << 16).text;
    assert!(
        text.contains("[https://www.youtube.com/watch?v=1XxNXkOHKZw](https://www.youtube.com/watch?v=1XxNXkOHKZw)"),
        "{text}"
    );
    for dropped in ["maxresdefault", "Watch on YouTube", "channel"] {
        assert!(!text.contains(dropped), "{dropped} kept: {text}");
    }
}

#[test]
fn the_lead_image_and_subtitle_shown_apart_from_the_article_open_it() {
    let html = ARTICLE_PAGE
        .replace(
            "</head>",
            r#"<meta property="og:image" content="https://cdn.example/lead.jpg?w=1200">
            <meta property="og:description" content="Hobbyists finish in days what took months.">
            </head>"#,
        )
        .replace(
            "<article>",
            r#"<div class="lede"><p class="dek">Hobbyists finish in days what took months.</p>
            <img src="https://cdn.example/lead.jpg?w=2400" alt="A groovebox."></div><article>"#,
        );
    let text = read(&html, 1 << 16).text;
    assert!(
        text.starts_with(
            "Hobbyists finish in days what took months.\n\n![A groovebox.](https://cdn.example/lead.jpg?w=2400)"
        ),
        "{text}"
    );
    assert!(text.contains("explosion of custom firmware"));
}

#[test]
fn a_comment_is_left_out_however_long() {
    let quoted = "<p>A reader quotes the whole article, at length, again and again.</p>".repeat(80);
    let html = ARTICLE_PAGE.replace(
        "</body>",
        &format!(r#"<div class="comment--2g6i"><div class="body">{quoted}</div></div></body>"#),
    );
    let text = read(&html, 1 << 16).text;
    assert!(text.contains("explosion of custom firmware"), "{text}");
    assert!(!text.contains("A reader quotes"));
}

#[test]
fn a_column_of_headlines_is_not_the_article() {
    let card = |n: u32| {
        format!(
            r#"<div class="card"><div class="body">Another story about synthesizers number {n}</div></div>"#
        )
    };
    let html = ARTICLE_PAGE.replace(
        "</body>",
        &format!(
            "<div>{}</div></body>",
            (1..=12).map(card).collect::<String>()
        ),
    );
    let text = read(&html, 1 << 16).text;
    assert!(text.contains("explosion of custom firmware"), "{text}");
    assert!(!text.contains("Another story"), "{text}");
}

#[test]
fn a_name_stating_the_page_layout_keeps_the_article_it_holds() {
    let html = ARTICLE_PAGE
        .replace(
            "<article>",
            r#"<section class="gh-container has-sidebar xl:[--sidebar-wide:1]"><article>"#,
        )
        .replace("</article>", "</article></section>");
    let text = read(&html, 1 << 16).text;
    assert!(text.contains("explosion of custom firmware"), "{text}");
}

#[test]
fn an_author_link_takes_only_its_own_block() {
    let html = ARTICLE_PAGE
        .replace(
            "<article>",
            r#"<div class="page"><p>By <a rel="author" href="/terrence">Terrence</a></p><article>"#,
        )
        .replace("</article>", "</article></div>");
    let text = read(&html, 1 << 16).text;
    assert!(text.contains("explosion of custom firmware"), "{text}");
    assert!(!text.contains("Terrence"), "{text}");
}
