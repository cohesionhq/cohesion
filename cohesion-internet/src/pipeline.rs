//! The one way an article's HTML becomes its text, whether it is an item's
//! page a feed links to, a page fetched again, or the content a feed
//! carries: Coherence ([`crate::coherence`]) reads it as Markdown, and the
//! text is kept to a limit.

use dom_query::Document;

use crate::article_snapshot::Url;
use crate::coherence::{self, Metadata};

/// The HTML an article is read from.
pub(crate) enum Html<'a> {
    /// A whole page, which Coherence finds the article in.
    Page(&'a str),
    /// A feed's content: all article, its metadata in the feed.
    Content(&'a str),
}

/// An article as its HTML reads.
#[derive(Debug)]
pub(crate) struct Article {
    pub metadata: Metadata,
    /// Its main content as Markdown; `""` when there is none.
    pub text: String,
    /// Whether `text` was cut to the limit.
    pub truncated: bool,
}

/// Reads `html`, from `url`, as an article, keeping at most `limit` bytes
/// of its text, cut on a character boundary.
pub(crate) fn read(html: Html, url: &Url, limit: usize) -> Article {
    let (metadata, mut text) = match html {
        Html::Page(html) => {
            let extracted = coherence::read_page(Document::from(html), url);
            (extracted.metadata, extracted.text)
        }
        Html::Content(html) => (
            Metadata::default(),
            coherence::read_content(Document::from(html), url),
        ),
    };
    let truncated = truncate(&mut text, limit);
    Article {
        metadata,
        text,
        truncated,
    }
}

/// Cuts `text` to at most `limit` bytes on a character boundary; whether it
/// cut anything.
pub(crate) fn truncate(text: &mut String, limit: usize) -> bool {
    if text.len() <= limit {
        return false;
    }
    let end = text.floor_char_boundary(limit);
    text.truncate(end);
    true
}

#[cfg(test)]
mod tests;
