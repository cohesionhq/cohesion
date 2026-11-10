//! Reading a feed, in three steps. [`read_feed`] fetches it, following a
//! page, once, to the feed it advertises ([`discover`]). [`parse_feed`]
//! reads it, by its format, into a [`ParsedFeed`]: [`rss`] for RSS 2.0,
//! [`atom`] for Atom, [`json`] for JSON Feed, each from typed schemas of
//! just the elements used. Each new item's text is then read ([`item`]):
//! the feed's content when it is the whole article, else the item's page,
//! both through the one pipeline every article's text takes.

mod atom;
mod discover;
mod item;
mod json;
mod rss;
mod text;

use cohesion_core::model::{ArticleFetchFailure, FeedCapture, FeedFullPagePolicy, Published};
use std::collections::HashSet;

use crate::article_snapshot::{ArticleFetchAccess, Url, is_web_address};
use crate::fetch::fetch;

pub(crate) use discover::discover_feed;

/// Reads the feed at `url` and each of its items but those whose link is
/// in `known`, which are left out unread, fetching an item's page when
/// `policy` says to or the feed holds only an excerpt of it.
/// An item whose page can't be fetched keeps the feed's text when there is
/// any. When `url` answers with a web page, the feed it links to is read
/// instead; the capture's `url` is the address actually read.
pub fn read_feed(
    access: ArticleFetchAccess,
    url: &Url,
    policy: FeedFullPagePolicy,
    known: &HashSet<String>,
) -> Result<FeedCapture, ArticleFetchFailure> {
    let mut fetched = fetch(url, |media_type| {
        is_html(media_type) || is_feed_media_type(media_type)
    })?;
    if is_html(&fetched.media_type) {
        let discovered = discover_feed(&fetched.body, &fetched.final_url)
            .ok_or(ArticleFetchFailure::NotAFeed)?;
        fetched = fetch(&discovered, is_feed_media_type)?;
    }
    let feed =
        parse_feed(&fetched.body, &fetched.final_url).ok_or(ArticleFetchFailure::NotAFeed)?;
    Ok(FeedCapture {
        url: fetched.final_url.to_string(),
        title: feed.title,
        items: feed
            .items
            .into_iter()
            .filter(|item| !known.contains(item.url.as_str()))
            .map(|item| item::capture(access, item, policy))
            .collect(),
    })
}

/// Whether `media_type` is a web page, which may advertise a feed.
fn is_html(media_type: &str) -> bool {
    matches!(media_type, "text/html" | "application/xhtml+xml")
}

/// Whether `media_type` can be a feed: XML, JSON or plain text.
fn is_feed_media_type(media_type: &str) -> bool {
    media_type.ends_with("xml") || media_type.ends_with("json") || media_type == "text/plain"
}

/// A feed as it reads, before any page is fetched.
#[derive(Debug)]
pub(crate) struct ParsedFeed {
    pub title: Option<String>,
    pub items: Vec<FeedItem>,
}

/// One item of a feed, its links made absolute.
#[derive(Debug)]
pub(crate) struct FeedItem {
    pub url: Url,
    pub title: Option<String>,
    pub author: Option<String>,
    pub published: Option<Published>,
    /// Its HTML in the feed: the full content when the feed has it, else
    /// its summary.
    pub html: Option<String>,
    /// What the HTML's relative links are relative to: the item's own base
    /// when the feed gives one (Atom's `xml:base`), else its link.
    pub base: Url,
}

/// `text`, read from `base`, as an RSS 2.0, Atom or JSON Feed; `None` when
/// it is none of them. Items without a link are left out, and so are those
/// whose link isn't a web address (`mailto:`, `javascript:`): an article's
/// URL is fetched, and opened by clients, as one.
pub(crate) fn parse_feed(text: &str, base: &Url) -> Option<ParsedFeed> {
    let mut feed = if text.trim_start().starts_with('{') {
        json::parse(text, base)
    } else {
        match text::root_element(text)?.as_str() {
            "rss" => rss::parse(text, base),
            "feed" => atom::parse(text, base),
            _ => None,
        }
    }?;
    feed.items.retain(|item| is_web_address(&item.url));
    Some(feed)
}

#[cfg(test)]
mod tests;
