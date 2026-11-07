//! Reading one item of a feed: its text from the feed, or from its page
//! when the feed carries only an excerpt or its policy says so.

use std::collections::HashSet;

use cohesion_core::model::{ArticleCapture, FeedFullPagePolicy, FeedItemCapture};

use super::FeedItem;
use crate::article_snapshot::{ArticleFetchAccess, TEXT_LIMIT, Url, snapshot};
use crate::pipeline::{Html, read};

/// `item` with its text: from the feed, or its page when `policy` says so
/// or the feed's text is an excerpt. A page that
/// doesn't hold the feed's text, which the parser misread, gives way to the
/// feed's.
pub(super) fn capture(
    access: ArticleFetchAccess,
    item: FeedItem,
    policy: FeedFullPagePolicy,
) -> FeedItemCapture {
    let from_feed = item
        .html
        .as_deref()
        .map(|html| feed_text(html, &item.url, &item.base));
    let excerpt = from_feed.as_ref().is_none_or(|text| text.excerpt);
    let fetched = match (policy, excerpt, from_feed) {
        (FeedFullPagePolicy::WhenExcerpt, false, Some(from_feed)) => Ok(from_feed.capture),
        (_, _, from_feed) => match (snapshot(access, &item.url), from_feed) {
            (Ok(page), Some(from_feed)) if !holds(&page.text, &from_feed.capture.text) => {
                Ok(from_feed.capture)
            }
            (Ok(page), _) => Ok(page),
            (Err(failure), from_feed) => from_feed.map(|text| text.capture).ok_or(failure),
        },
    };
    FeedItemCapture {
        url: item.url.to_string(),
        title: item.title,
        author: item.author,
        published: item.published,
        fetched,
    }
}

/// An item's text as its feed has it.
pub(super) struct FeedText {
    pub capture: ArticleCapture,
    /// Whether it reads as an excerpt of the item rather than all of it.
    pub excerpt: bool,
}

/// The most text, in bytes, of a closing line that links back to its item.
const FOOTER_LIMIT: usize = 400;

/// `html`, the text a feed gives the item at `url`, as Markdown, its links
/// made absolute against `base`. A closing line that links back to the
/// item, such as WordPress's "The post … appeared first on …" or "Read the
/// full story", is dropped. The text is an excerpt only when it says so: it
/// is cut off with an ellipsis, or closed with a link to read or continue
/// reading the rest. A short text is a short post, not a teaser: link posts
/// and release notes are whole.
pub(super) fn feed_text(html: &str, url: &Url, base: &Url) -> FeedText {
    let article = read(Html::Content(html), base, TEXT_LIMIT);
    let mut text = article.text;
    let footer = footer_start(&text, url).map(|start| text.split_off(start));
    text.truncate(text.trim_end().len());
    let read_on = footer.as_deref().is_some_and(|footer| {
        footer
            .split(|c: char| !c.is_alphanumeric())
            .any(|word| ["read", "continue", "continued"].contains(&word.to_lowercase().as_str()))
    });
    let end = text.trim_end_matches(['[', ']', '\\', ' ']);
    let excerpt = read_on || end.ends_with('…') || end.ends_with("...");
    FeedText {
        capture: ArticleCapture {
            final_url: url.to_string(),
            page_title: None,
            author: None,
            published: None,
            text,
            truncated: article.truncated,
        },
        excerpt,
    }
}

/// Where the closing line of `markdown` that links back to `url` starts,
/// when there is one, it holds less than [`FOOTER_LIMIT`] with what follows,
/// and text comes before it.
fn footer_start(markdown: &str, url: &Url) -> Option<usize> {
    let link = markdown.rmatch_indices("](").find_map(|(at, _)| {
        let destination = markdown[at + 2..].split([')', ' ']).next()?;
        let destination = Url::parse(destination).ok()?;
        let same = destination.host_str() == url.host_str()
            && destination.path().trim_end_matches('/') == url.path().trim_end_matches('/')
            && destination.query() == url.query();
        same.then_some(at)
    })?;
    let start = markdown[..link]
        .rfind('\n')
        .map_or(0, |newline| newline + 1);
    (start > 0 && markdown.len() - start < FOOTER_LIMIT).then_some(start)
}

/// Whether `page`'s text holds at least half of the words of `feed`'s, as
/// an article's page holds the excerpt its feed gives. Words shorter than
/// four letters are left out, as any text holds those.
pub(super) fn holds(page: &str, feed: &str) -> bool {
    let words = |text: &str| {
        text.split(|c: char| !c.is_alphanumeric())
            .filter(|word| word.chars().count() >= 4)
            .map(str::to_lowercase)
            .collect::<HashSet<_>>()
    };
    let feed = words(feed);
    let page = words(page);
    feed.intersection(&page).count() * 2 >= feed.len()
}
