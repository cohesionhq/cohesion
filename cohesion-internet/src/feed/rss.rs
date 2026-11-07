//! RSS 2.0, with the content and Dublin Core extensions feeds use for an
//! item's full text, author and date. Elements match by local name, so
//! `encoded` is `content:encoded` and `creator` `dc:creator` whatever
//! prefix the feed binds those namespaces to.

use serde::Deserialize;

use super::text::{plain_text, present};
use super::{FeedItem, ParsedFeed};
use crate::article_snapshot::Url;
use crate::published_time;

/// `xml`, an `<rss>` document read from `base`, as a feed.
pub(super) fn parse(xml: &str, base: &Url) -> Option<ParsedFeed> {
    let rss: Rss = quick_xml::de::from_str(xml).ok()?;
    let items = rss
        .channel
        .items
        .into_iter()
        .filter_map(|item| {
            let url = base.join(item.link?.trim()).ok()?;
            Some(FeedItem {
                title: plain_text(item.title),
                author: plain_text(item.creator.or(item.author)),
                published: item
                    .pub_date
                    .or(item.date)
                    .as_deref()
                    .and_then(published_time),
                html: present(item.content.or(item.description)),
                base: url.clone(),
                url,
            })
        })
        .collect();
    Some(ParsedFeed {
        title: plain_text(rss.channel.title),
        items,
    })
}

#[derive(Deserialize)]
struct Rss {
    channel: Channel,
}

#[derive(Deserialize)]
struct Channel {
    title: Option<String>,
    #[serde(default, rename = "item")]
    items: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    title: Option<String>,
    link: Option<String>,
    /// The item's summary, or all of it when the feed has no `encoded`.
    description: Option<String>,
    #[serde(rename = "encoded")]
    content: Option<String>,
    author: Option<String>,
    creator: Option<String>,
    #[serde(rename = "pubDate")]
    pub_date: Option<String>,
    date: Option<String>,
}
