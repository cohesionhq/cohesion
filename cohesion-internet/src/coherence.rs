//! Coherence, Cohesion's own parser. Its rules ([`rules`]) are decided over
//! a page, or a feed's content, once ([`rule`]); its finder ([`finder`])
//! picks out the elements holding the article, skipping what the rules
//! left out; its writer ([`markdown`]) makes those Markdown in one walk,
//! carrying out the rules' decisions as it goes. What the page says of
//! itself is read before any of that ([`metadata`]).

mod finder;
mod markdown;
mod metadata;
mod page;
mod rule;
mod rules;

use dom_query::Document;

use crate::article_snapshot::Url;
use page::Page;

pub(crate) use metadata::Metadata;

/// An article read from its page.
pub(crate) struct Extracted {
    pub metadata: Metadata,
    /// Its main content as Markdown; `""` when none was found.
    pub text: String,
}

/// The article in `document`, a whole page fetched from `url`: what the
/// page says of itself, and its main content as Markdown, links made
/// absolute.
pub(crate) fn read_page(document: Document, url: &Url) -> Extracted {
    let metadata = metadata::metadata(&document);
    let descriptions = metadata::descriptions(&document);
    let image = metadata::image(&document);
    let page = Page::new(&document);
    let mut decisions = rule::decide(&page, rules::RULES);
    let lead = finder::Lead {
        title: metadata.title.as_deref(),
        descriptions: &descriptions,
        image: image.as_deref(),
    };
    let roots = finder::article(&page, lead, &mut decisions);
    let mut text = markdown::markdown(&roots, &decisions, url);
    // A page its script writes holds nothing to find but what it says
    // of itself.
    if text.is_empty() {
        text = descriptions
            .first()
            .map(|description| markdown::paragraphs(description))
            .unwrap_or_default();
    }
    Extracted { metadata, text }
}

/// `content`, a feed's content for the item at `url` and all article, as
/// Markdown, links made absolute.
pub(crate) fn read_content(content: Document, url: &Url) -> String {
    let decisions = rule::decide(&Page::new(&content), rules::RULES);
    markdown::markdown(&[content.root()], &decisions, url)
}
