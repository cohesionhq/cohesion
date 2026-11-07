//! Fetching an article's page and reducing it to the Markdown of its main
//! content ([`crate::pipeline`]): the one path `Article.resnapshot`, the
//! daemon's `ResnapshotArticle` request and a feed's items' pages take. It blocks, so callers run it off the database thread (and
//! off an async runtime). Fetching takes an [`ArticleFetchAccess`], which a
//! script holds only when its sandbox mounts `ArticleFetch`, and no extension
//! permission mounts that.

use crate::coherence::Metadata;
use crate::fetch::fetch;
use crate::pipeline::{Article, Html, read, truncate};
use cohesion_core::model::{ArticleCapture, ArticleFetchFailure};

pub use reqwest::Url;

/// The most text kept from a page, in bytes; longer text is cut short.
pub(crate) const TEXT_LIMIT: usize = 1024 * 1024;

/// Permission to fetch an article's page. Whoever decides a caller may
/// fetch makes one with [`granted`](Self::granted): the daemon for a request
/// from a client the user drives, and a sandbox that mounts `ArticleFetch`
/// for its scripts.
#[derive(Clone, Copy)]
pub struct ArticleFetchAccess(());

impl ArticleFetchAccess {
    /// For a caller already allowed to fetch: a client's request, or a
    /// script whose sandbox mounts `ArticleFetch`.
    pub fn granted() -> Self {
        Self(())
    }
}

/// `text` as a URL an article can be read from: an absolute `http` or
/// `https` address, else `None`.
pub fn article_url(text: &str) -> Option<Url> {
    Url::parse(text.trim())
        .ok()
        .filter(|url| matches!(url.scheme(), "http" | "https"))
}

/// Fetches `url` and reads its text: HTML as the Markdown of the article
/// Coherence finds in it, plain text as it is. Anything else, an error
/// status, or a body over 5 MiB fails.
pub fn snapshot(_: ArticleFetchAccess, url: &Url) -> Result<ArticleCapture, ArticleFetchFailure> {
    let page = fetch(url, |media_type| {
        matches!(
            media_type,
            "text/html" | "application/xhtml+xml" | "text/plain"
        )
    })?;
    let article = if page.media_type == "text/plain" {
        let mut text = page.body;
        let truncated = truncate(&mut text, TEXT_LIMIT);
        Article {
            metadata: Metadata::default(),
            text,
            truncated,
        }
    } else {
        read(Html::Page(&page.body), &page.final_url, TEXT_LIMIT)
    };
    Ok(ArticleCapture {
        final_url: page.final_url.to_string(),
        page_title: article.metadata.title,
        author: article.metadata.author,
        published: article.metadata.published,
        text: article.text,
        truncated: article.truncated,
    })
}

#[cfg(test)]
mod tests;
