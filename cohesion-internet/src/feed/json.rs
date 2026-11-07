//! JSON Feed, version 1.0 or 1.1. Version 1.1 lists an item's `authors`;
//! 1.0 has one `author`, which a 1.1 feed may repeat for older readers.

use serde::Deserialize;

use super::text::{joined_names, plain_text, present};
use super::{FeedItem, ParsedFeed};
use crate::article_snapshot::Url;
use crate::published_time;

/// `json`, read from `base`, as a feed.
pub(super) fn parse(json: &str, base: &Url) -> Option<ParsedFeed> {
    let feed: Feed = serde_json::from_str(json).ok()?;
    let items = feed
        .items
        .into_iter()
        .filter_map(|item| {
            let url = base.join(item.url?.trim()).ok()?;
            Some(FeedItem {
                title: plain_text(item.title),
                author: joined_names(
                    match item.authors.is_empty() {
                        true => item.author.into_iter().collect(),
                        false => item.authors,
                    }
                    .into_iter()
                    .map(|person| person.name),
                ),
                published: item
                    .date_published
                    .or(item.date_modified)
                    .as_deref()
                    .and_then(published_time),
                html: present(item.content_html)
                    .or_else(|| present(item.content_text).map(|text| paragraphs(&text)))
                    .or_else(|| present(item.summary)),
                base: url.clone(),
                url,
            })
        })
        .collect();
    Some(ParsedFeed {
        title: plain_text(feed.title),
        items,
    })
}

/// Plain `text` as HTML: its blank-line separated paragraphs escaped and
/// wrapped in `<p>`, line breaks kept.
fn paragraphs(text: &str) -> String {
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    escaped
        .replace("\r\n", "\n")
        .split("\n\n")
        .map(str::trim)
        .filter(|paragraph| !paragraph.is_empty())
        .map(|paragraph| format!("<p>{}</p>", paragraph.replace('\n', "<br>")))
        .collect()
}

#[derive(Deserialize)]
struct Feed {
    title: Option<String>,
    #[serde(default)]
    items: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    url: Option<String>,
    title: Option<String>,
    content_html: Option<String>,
    content_text: Option<String>,
    summary: Option<String>,
    #[serde(default)]
    authors: Vec<Person>,
    author: Option<Person>,
    date_published: Option<String>,
    date_modified: Option<String>,
}

#[derive(Deserialize)]
struct Person {
    name: Option<String>,
}
