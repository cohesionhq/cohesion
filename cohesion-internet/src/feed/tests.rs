use super::item::{FeedText, holds};
use super::{Url, discover_feed, parse_feed};
use crate::pipeline::{Html, read};

/// `html`, a feed's content for the item at `url`, as Coherence writes it.
fn content(html: &str, url: &Url) -> String {
    read(Html::Content(html), url, 1 << 16).text
}

/// `html`, a feed's text for the item at `url`, as Coherence reads it.
fn feed_text(html: &str, url: &Url) -> FeedText {
    super::item::feed_text(html, url, url)
}

fn base() -> Url {
    Url::parse("https://bakery.example/feed.xml").unwrap()
}

const RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:content="http://purl.org/rss/1.0/modules/content/"
     xmlns:dc="http://purl.org/dc/elements/1.1/">
  <channel>
    <title> The Bakery </title>
    <link>https://bakery.example/</link>
    <item>
      <title>Rye, slowly</title>
      <link>/posts/rye</link>
      <description>A short summary.</description>
      <content:encoded><![CDATA[<p>Rye <a href="/flour">ferments</a> faster.</p>]]></content:encoded>
      <dc:creator>Ada Baker</dc:creator>
      <pubDate>Fri, 01 Mar 2024 12:00:00 GMT</pubDate>
    </item>
    <item>
      <title>Untitled link</title>
      <description>&lt;p&gt;Only a summary.&lt;/p&gt;</description>
    </item>
    <item>
      <link>https://bakery.example/posts/ovens</link>
      <description>Ovens.</description>
    </item>
  </channel>
</rss>"#;

const ATOM: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Notes</title>
  <entry>
    <title type="html">Proofing &amp;amp; shaping</title>
    <link rel="replies" href="https://notes.example/a/comments"/>
    <link href="https://notes.example/a"/>
    <author><name>Grace</name></author>
    <updated>2024-03-02T09:00:00Z</updated>
    <published>2024-03-01T12:00:00Z</published>
    <summary>Short.</summary>
    <content type="html">&lt;p&gt;Long text.&lt;/p&gt;</content>
  </entry>
</feed>"#;

#[test]
fn reads_an_rss_feed_with_its_extensions() {
    let feed = parse_feed(RSS, &base()).expect("RSS parses");
    assert_eq!(feed.title.as_deref(), Some("The Bakery"));
    // The item without a link is left out.
    assert_eq!(feed.items.len(), 2);
    let rye = &feed.items[0];
    assert_eq!(rye.url.as_str(), "https://bakery.example/posts/rye");
    assert_eq!(rye.title.as_deref(), Some("Rye, slowly"));
    assert_eq!(rye.author.as_deref(), Some("Ada Baker"));
    assert_eq!(
        rye.published.map(|published| published.at),
        Some(1_709_294_400)
    );
    // The full content wins over the summary.
    assert!(rye.html.as_deref().unwrap().contains("ferments"));
    let ovens = &feed.items[1];
    assert_eq!(ovens.title, None);
    assert_eq!(ovens.published.map(|published| published.at), None);
}

#[test]
fn reads_an_atom_feed() {
    let feed = parse_feed(ATOM, &base()).expect("Atom parses");
    assert_eq!(feed.title.as_deref(), Some("Notes"));
    let entry = &feed.items[0];
    // The alternate link, not the replies one.
    assert_eq!(entry.url.as_str(), "https://notes.example/a");
    assert_eq!(entry.title.as_deref(), Some("Proofing & shaping"));
    assert_eq!(entry.author.as_deref(), Some("Grace"));
    // Published, not updated.
    assert_eq!(
        entry.published.map(|published| published.at),
        Some(1_709_294_400)
    );
    assert_eq!(entry.html.as_deref(), Some("<p>Long text.</p>"));
}

#[test]
fn atom_content_keeps_its_xhtml_and_its_base() {
    let atom = r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <entry>
    <link href="https://notes.example/a"/>
    <summary type="html">&lt;p&gt;Short.&lt;/p&gt;</summary>
    <content type="xhtml" xml:base="https://notes.example/linked/"><div xmlns="http://www.w3.org/1999/xhtml"><p>Long <b>text</b>, <a href="rye">linked</a>.</p></div></content>
  </entry>
  <entry>
    <link href="https://notes.example/b"/>
    <content type="html" xml:base="https://elsewhere.example/">&lt;a href="ovens"&gt;Ovens&lt;/a&gt;</content>
  </entry>
</feed>"#;
    let feed = parse_feed(atom, &base()).expect("Atom parses");
    let first = &feed.items[0];
    assert!(first.html.as_deref().unwrap().contains("<b>text</b>"));
    assert_eq!(
        content(first.html.as_deref().unwrap(), &first.base),
        "Long **text**, [linked](https://notes.example/linked/rye)."
    );
    let second = &feed.items[1];
    assert_eq!(second.base.as_str(), "https://elsewhere.example/");
}

#[test]
fn a_page_is_not_a_feed() {
    assert!(parse_feed("<!doctype html><html><body>Hi</body></html>", &base()).is_none());
    assert!(parse_feed("not xml at all", &base()).is_none());
}

#[test]
fn feed_html_becomes_markdown_with_absolute_links() {
    let url = Url::parse("https://bakery.example/posts/rye").unwrap();
    let text = content(r#"<p>Rye <a href="/flour">ferments</a> faster.</p>"#, &url);
    assert_eq!(text, "Rye [ferments](https://bakery.example/flour) faster.");
}

#[test]
fn reads_a_json_feed_1_1() {
    let json = r#"{
      "version": "https://jsonfeed.org/version/1.1",
      "title": " Field notes ",
      "items": [
        {"id": "1", "url": "/posts/one", "title": "One",
         "content_html": "<p>Full.</p>", "content_text": "Plain.",
         "authors": [{"name": "Ada"}, {"name": "Grace"}],
         "date_published": "2024-03-01T12:00:00Z", "date_modified": "2024-03-02T09:00:00Z"},
        {"id": "2", "url": "https://elsewhere.example/two",
         "content_text": "A < B & C.\n\nSecond paragraph.", "date_modified": "2024-03-02T09:00:00Z"},
        {"id": "3", "external_url": "https://elsewhere.example/three", "summary": "No url."},
        {"id": "4", "url": "/posts/four", "summary": "Only a summary."}
      ]
    }"#;
    let feed = parse_feed(json, &base()).expect("JSON Feed parses");
    assert_eq!(feed.title.as_deref(), Some("Field notes"));
    // The item without a `url` is left out.
    assert_eq!(feed.items.len(), 3);
    let one = &feed.items[0];
    assert_eq!(one.url.as_str(), "https://bakery.example/posts/one");
    assert_eq!(one.author.as_deref(), Some("Ada, Grace"));
    assert_eq!(
        one.published.map(|published| published.at),
        Some(1_709_294_400)
    );
    assert_eq!(one.html.as_deref(), Some("<p>Full.</p>"));
    let two = &feed.items[1];
    assert_eq!(
        two.published.map(|published| published.at),
        Some(1_709_370_000)
    );
    let html = two.html.as_deref().unwrap();
    assert!(html.contains("<p>A &lt; B &amp; C.</p>"));
    assert!(html.contains("<p>Second paragraph.</p>"));
    assert_eq!(feed.items[2].html.as_deref(), Some("Only a summary."));
}

#[test]
fn a_json_feed_naming_an_author_both_ways_has_one_author() {
    let json = r#"{
      "version": "https://jsonfeed.org/version/1.1",
      "items": [{
         "url": "https://bakery.example/rye",
         "authors": [{"name": "Ada"}],
         "author": {"name": "Ada"}
      }]
    }"#;
    let feed = parse_feed(json, &base()).expect("JSON Feed parses");
    assert_eq!(feed.items[0].author.as_deref(), Some("Ada"));
}

#[test]
fn reads_a_json_feed_1_0_author() {
    let json = r#"{
      "version": "https://jsonfeed.org/version/1",
      "title": "Old feed",
      "items": [{"id": "1", "url": "https://old.example/a", "content_html": "<p>Hi</p>",
                 "author": {"name": "Linus"}}]
    }"#;
    let feed = parse_feed(json, &base()).expect("JSON Feed parses");
    assert_eq!(feed.items[0].author.as_deref(), Some("Linus"));
}

#[test]
fn discovers_the_first_feed_a_page_lists() {
    let page = Url::parse("https://blog.example/news/today").unwrap();
    let html = r#"<html><head>
      <link rel="alternate" type="application/json+oembed" href="/oembed">
      <link rel="alternate" title="API" type="application/json" href="/wp-json/posts/1">
      <link rel="stylesheet" type="application/rss+xml" href="/not-alternate">
      <link rel='alternate' type='Application/Atom+XML' href='feed/atom'>
      <link rel="alternate" type="application/rss+xml" href="/rss">
    </head><body></body></html>"#;
    // The first real feed, resolved against the page; the generic JSON
    // link is only a fallback.
    assert_eq!(
        discover_feed(html, &page).unwrap().as_str(),
        "https://blog.example/news/feed/atom"
    );
    let only_json = r#"<link rel="alternate" type="application/json" href="/feed.json">"#;
    assert_eq!(
        discover_feed(only_json, &page).unwrap().as_str(),
        "https://blog.example/feed.json"
    );
    assert!(discover_feed("<html><head></head></html>", &page).is_none());
}

#[test]
fn titles_and_names_read_as_plain_text() {
    // The Verge's Atom escapes its titles' entities inside CDATA; RSS feeds
    // escape them twice, or put markup in a title.
    let atom = r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title type="text">The   Verge</title>
  <author><name>Verge Staff</name></author>
  <entry>
    <title type="html"><![CDATA[CoreWeave&#8217;s debt is the industry&#8217;s problem]]></title>
    <link href="https://verge.example/a"/>
  </entry>
  <entry>
    <title>Two bylines</title>
    <link href="https://verge.example/b"/>
    <author><name>Ada</name></author>
    <author><name>Grace</name></author>
  </entry>
</feed>"#;
    let feed = parse_feed(atom, &base()).expect("Atom parses");
    assert_eq!(feed.title.as_deref(), Some("The Verge"));
    assert_eq!(
        feed.items[0].title.as_deref(),
        Some("CoreWeave’s debt is the industry’s problem")
    );
    // An entry without an author has the feed's.
    assert_eq!(feed.items[0].author.as_deref(), Some("Verge Staff"));
    assert_eq!(feed.items[1].author.as_deref(), Some("Ada, Grace"));

    let rss = r#"<?xml version="1.0"?><rss version="2.0"
        xmlns:dc="http://purl.org/dc/elements/1.1/"><channel>
      <title>Bread &amp;amp; Butter</title>
      <item>
        <title>&lt;em&gt;Rye&lt;/em&gt; &amp;#8212; slowly</title>
        <link>/rye</link>
        <dc:creator>Ada &amp;amp; Grace</dc:creator>
      </item>
    </channel></rss>"#;
    let feed = parse_feed(rss, &base()).expect("RSS parses");
    assert_eq!(feed.title.as_deref(), Some("Bread & Butter"));
    assert_eq!(feed.items[0].title.as_deref(), Some("Rye — slowly"));
    assert_eq!(feed.items[0].author.as_deref(), Some("Ada & Grace"));
}

/// About 200 words of an article's prose.
fn prose() -> String {
    "<p>The oven is the heart of any bakery, and ours runs from four in the morning. ".to_string()
        + &"Loaves go in as the sun comes up and come out crusted and singing. ".repeat(14)
        + "</p>"
}

#[test]
fn a_feeds_closing_link_back_to_the_item_is_dropped() {
    let url = Url::parse("https://bakery.example/posts/ovens/").unwrap();
    let html = prose()
        + r#"<br/>This article, "<a href="https://bakery.example/posts/ovens/">Ovens</a>" first
        appeared on <a href="https://bakery.example">The Bakery</a><br/><br/>
        <a href="https://forums.bakery.example/t/1">Discuss this article</a> in our forums"#;
    let text = feed_text(&html, &url);
    assert!(text.capture.text.contains("crusted and singing"));
    assert!(!text.capture.text.contains("first"), "footer kept");
    assert!(!text.capture.text.contains("Discuss"), "footer kept");
    assert!(!text.excerpt);
}

#[test]
fn a_feed_text_is_an_excerpt_only_when_it_says_so() {
    let url = Url::parse("https://bakery.example/posts/ovens").unwrap();
    let read_on = prose()
        + r#"<p><a href="https://bakery.example/posts/ovens">Read the full story at The Bakery.</a></p>"#;
    let text = feed_text(&read_on, &url);
    assert!(text.excerpt);
    assert!(!text.capture.text.contains("Read the full story"));

    let ellipsis = prose() + "<p>And then the baker said …</p>";
    assert!(feed_text(&ellipsis, &url).excerpt);
    assert!(!feed_text(&prose(), &url).excerpt);
    assert!(!feed_text("<p>Ovens are hot.</p>", &url).excerpt);
}

#[test]
fn a_links_tooltip_is_not_text() {
    let url = Url::parse("https://bakery.example/posts/ovens").unwrap();
    let html = r##"<p>Ovens are hot.<sup><a href="#fn1" title="Jump to footnote 1">1</a></sup></p>
        <p><a href="/linked/ovens" title="Permanent link to ‘Ovens’">Ovens</a></p>"##;
    let text = content(html, &url);
    assert!(
        text.contains("https://bakery.example/linked/ovens"),
        "{text}"
    );
    assert!(!text.contains("Permanent link"), "{text}");
    assert!(!text.contains("Jump to"), "{text}");
}

#[test]
fn a_page_must_hold_the_feeds_text_to_replace_it() {
    let feed = "When I switched LLM over to httpx2 I forgot this would break plugins \
        which used httpx without depending on it.";
    let page = format!("Release notes. {feed} More words follow here.");
    assert!(holds(&page, feed));
    // A parser that picked the sidebar of a short post.
    let sidebar = "This is a beat by Simon, posted on 18th October. Monthly briefing: \
        sponsor me for $10/month and get a curated email digest.";
    assert!(!holds(sidebar, feed));
}

#[test]
fn a_members_only_notice_is_not_text() {
    let url = Url::parse("https://bakery.example/club/issue-1/").unwrap();
    let html = r#"<p>This week: ovens.</p>
        <div class="club-notice-restricted plan-"><h2>This Story is for Club Members</h2>
        <p><a href="https://bakery.example/plans" class="button">See Plans</a></p>
        <p>Already a member? <a href="https://bakery.example/?auth">Sign in</a></p></div>"#;
    let text = content(html, &url);
    assert!(text.contains("This week: ovens."), "{text}");
    assert!(!text.contains("Club Members"), "{text}");
    assert!(!text.contains("Sign in"), "{text}");
}

#[test]
fn a_glyph_linking_back_is_not_text() {
    let url = Url::parse("https://bakery.example/linked/ovens").unwrap();
    let html = r##"<h3>GTK <a href="https://gtk.example">↗</a></h3>
        <p>Ovens are hot.<sup><a href="#fn1">1</a></sup></p>
        <div><a href="https://bakery.example/linked/ovens">★</a></div>
        <ol><li id="fn1"><p>Very hot. <a href="#fnr1">↩︎</a></p></li></ol>"##;
    let text = content(html, &url);
    assert!(text.contains("[↗](https://gtk.example/)"), "{text}");
    assert!(text.contains("[1]("), "{text}");
    assert!(!text.contains('★'), "{text}");
    assert!(!text.contains('↩'), "{text}");
}

#[test]
fn an_embeds_placeholder_becomes_a_link_to_the_post() {
    let url = Url::parse("https://bakery.example/posts/ovens").unwrap();
    let html = r#"<p>Watch this.</p>
        <blockquote class="instagram-media" data-instgrm-permalink="https://www.instagram.com/reel/abc/?utm_source=ig_embed&amp;utm_campaign=loading">
        <div><a href="https://www.instagram.com/reel/abc/?utm_source=ig_embed"><div></div>
        <div>View this post on Instagram</div></a></div></blockquote>"#;
    let text = content(html, &url);
    assert!(!text.contains("View this post"), "{text}");
    assert!(
        text.contains("[https://www.instagram.com/reel/abc/](https://www.instagram.com/reel/abc/?utm_source=ig_embed&utm_campaign=loading)"),
        "{text}"
    );
}

#[test]
fn a_scripts_code_left_bare_is_not_text() {
    let url = Url::parse("https://bakery.example/posts/ovens").unwrap();
    let html = r#"<p>A deal on ovens.</p>
        <div id="deal-root"></div>
          window.DEAL_CONFIG = {
            price: "$99",
            link: "Shop at the bakery"
          };
        <p>Set it to <code>temp = { high: 250 };</code></p>
        <pre>let oven = { hot: true };</pre>"#;
    let text = content(html, &url);
    assert!(text.contains("A deal on ovens."), "{text}");
    assert!(!text.contains("DEAL_CONFIG"), "{text}");
    assert!(text.contains("temp = { high: 250 };"), "{text}");
    assert!(text.contains("let oven = { hot: true };"), "{text}");
}
