//! Finding the feed a web page advertises.

use dom_query::Document;

use crate::article_snapshot::Url;

/// The feed `html`, a page fetched from `base`, advertises in a
/// `<link rel="alternate">`, made absolute. An RSS, Atom or JSON Feed link
/// wins over a bare `application/json` one, which sites also use for their
/// APIs; among equals the first listed wins.
pub(crate) fn discover_feed(html: &str, base: &Url) -> Option<Url> {
    let document = Document::from(html);
    document
        .select("link[rel][type][href]")
        .nodes()
        .iter()
        .filter(|node| {
            node.attr_or("rel", "")
                .split_whitespace()
                .any(|rel| rel.eq_ignore_ascii_case("alternate"))
        })
        .filter_map(|node| {
            let rank = match node
                .attr_or("type", "")
                .trim()
                .to_ascii_lowercase()
                .as_str()
            {
                "application/rss+xml" | "application/atom+xml" | "application/feed+json" => 0,
                "application/json" => 1,
                _ => return None,
            };
            Some((rank, base.join(node.attr_or("href", "").trim()).ok()?))
        })
        .min_by_key(|(rank, _)| *rank)
        .map(|(_, url)| url)
}
