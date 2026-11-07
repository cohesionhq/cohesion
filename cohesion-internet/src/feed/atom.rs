//! Atom. A text construct is plain text, escaped HTML, or XHTML markup
//! inline (`type="xhtml"`), which serde reads as no text, so that markup is
//! read from the document directly. `xml:base` on an entry or its content
//! is what the content's relative links are relative to.

use std::collections::VecDeque;

use quick_xml::Reader;
use quick_xml::events::Event;
use serde::Deserialize;

use super::text::{joined_names, plain_text, present};
use super::{FeedItem, ParsedFeed};
use crate::article_snapshot::Url;
use crate::published_time;

/// `xml`, a `<feed>` document read from `base`, as a feed.
pub(super) fn parse(xml: &str, base: &Url) -> Option<ParsedFeed> {
    let feed: Feed = quick_xml::de::from_str(xml).ok()?;
    let mut markup = Markup::read(xml);
    let base = rebased(base, feed.base.as_deref());
    // An entry without an author has the feed's.
    let feed_authors = joined_names(feed.authors.into_iter().map(|person| person.name));
    let items = feed
        .entries
        .into_iter()
        .filter_map(|entry| {
            let entry_base = rebased(&base, entry.base.as_deref());
            let content = entry
                .content
                .map(|text| markup.text(text, Element::Content));
            let summary = entry
                .summary
                .map(|text| markup.text(text, Element::Summary));
            let href = &entry
                .links
                .iter()
                .find(|link| matches!(link.rel.as_deref(), None | Some("alternate")))?
                .href;
            let url = entry_base.join(href.trim()).ok()?;
            let (html, content_base) = content
                .or(summary)
                .map(|(text, base)| (present(Some(text)), base))
                .unwrap_or_default();
            Some(FeedItem {
                title: plain_text(entry.title.map(|title| title.text)),
                author: joined_names(entry.authors.into_iter().map(|person| person.name))
                    .or_else(|| feed_authors.clone()),
                published: entry
                    .published
                    .or(entry.updated)
                    .as_deref()
                    .and_then(published_time),
                html,
                base: rebased(&entry_base, content_base.as_deref()),
                url,
            })
        })
        .collect();
    Some(ParsedFeed {
        title: plain_text(feed.title.map(|title| title.text)),
        items,
    })
}

/// `base` with `relative`, an `xml:base`, resolved against it.
fn rebased(base: &Url, relative: Option<&str>) -> Url {
    relative
        .and_then(|relative| base.join(relative.trim()).ok())
        .unwrap_or_else(|| base.clone())
}

/// The elements whose XHTML markup is read from the document.
#[derive(Clone, Copy)]
enum Element {
    Content,
    Summary,
}

/// The markup of each `content` and `summary` of type `xhtml`, in the order
/// the document has them.
struct Markup {
    content: VecDeque<String>,
    summary: VecDeque<String>,
}

impl Markup {
    fn read(xml: &str) -> Self {
        let mut markup = Self {
            content: VecDeque::new(),
            summary: VecDeque::new(),
        };
        let mut reader = Reader::from_str(xml);
        while let Ok(event) = reader.read_event() {
            let start = match event {
                Event::Start(start) => start,
                Event::Eof => break,
                _ => continue,
            };
            let xhtml = start
                .try_get_attribute("type")
                .ok()
                .flatten()
                .is_some_and(|kind| &*kind.value == "xhtml");
            let queue = match start.local_name().as_ref() {
                "content" if xhtml => &mut markup.content,
                "summary" if xhtml => &mut markup.summary,
                _ => continue,
            };
            let Ok(inner) = reader.read_text(start.name()) else {
                break;
            };
            queue.push_back(inner.into_inner().into_owned());
        }
        markup
    }

    /// `text`'s HTML and its `xml:base`: its markup when it is XHTML,
    /// taken in turn.
    fn text(&mut self, text: Text, element: Element) -> (String, Option<String>) {
        let html = match text.kind.as_deref() {
            Some("xhtml") => match element {
                Element::Content => self.content.pop_front(),
                Element::Summary => self.summary.pop_front(),
            }
            .unwrap_or_default(),
            _ => text.text,
        };
        (html, text.base)
    }
}

#[derive(Deserialize)]
struct Feed {
    #[serde(rename = "@xml:base")]
    base: Option<String>,
    title: Option<Text>,
    #[serde(default, rename = "author")]
    authors: Vec<Person>,
    #[serde(default, rename = "entry")]
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    #[serde(rename = "@xml:base")]
    base: Option<String>,
    title: Option<Text>,
    #[serde(default, rename = "link")]
    links: Vec<Link>,
    #[serde(default, rename = "author")]
    authors: Vec<Person>,
    published: Option<String>,
    updated: Option<String>,
    content: Option<Text>,
    summary: Option<Text>,
}

#[derive(Deserialize)]
struct Link {
    #[serde(rename = "@href")]
    href: String,
    #[serde(rename = "@rel")]
    rel: Option<String>,
}

#[derive(Deserialize)]
struct Person {
    name: Option<String>,
}

/// A text construct: plain text or escaped HTML, or XHTML whose markup
/// [`Markup`] reads.
#[derive(Deserialize)]
struct Text {
    #[serde(rename = "@type")]
    kind: Option<String>,
    #[serde(rename = "@xml:base")]
    base: Option<String>,
    #[serde(default, rename = "$text")]
    text: String,
}
