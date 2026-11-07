use cohesion_macros::{Fields, Model, Projection, TextEnum, TsWire};
use serde::{Deserialize, Serialize};

/// A web page a feed brought in, with its content as Markdown as of when it
/// was last fetched. Articles belong to no workspace: they make up the feed
/// corpus every workspace shares.
#[derive(Clone, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "articles", backup = Included)]
pub struct Article {
    /// Unique, stable identifier.
    pub id: i64,
    /// Whether the article has been soft-deleted. Listings skip deleted articles,
    /// so this is normally `false`.
    pub deleted: bool,
    /// The feed it came from.
    pub feed_id: i64,
    /// Whether it has been read. A new article starts unread.
    pub read: bool,

    /// The item's link in the feed.
    pub url: String,
    /// Display name: the item's title in the feed, else its page's, else the
    /// last part of the URL's path (its host when the path is empty), until
    /// renamed.
    pub title: String,
    /// Who wrote it, when the feed or page says.
    pub author: Option<String>,
    /// When it was published, in Unix seconds: the date the feed or page
    /// gives, else when it was brought in.
    pub published_at: i64,
    /// Whether `published_at` is a moment or only a day.
    pub published_precision: PublishedPrecision,
    /// How the last fetch went.
    #[model(group)]
    pub snapshot: ArticleSnapshot,
    /// The page's main content as Markdown (a plain-text page as it was
    /// served) as of the snapshot; `""` when the page has never been
    /// fetched successfully. Full-text searchable, with the title and
    /// author, through the corpus search.
    pub contents: String,
    /// When the article was brought in, in Unix seconds.
    pub created_at: i64,
    /// When the article last changed, in Unix seconds.
    pub modified: i64,
}

/// Which articles a listing covers.
///
/// - `{ "feed": id }`: every article that came from that feed.
/// - `"feeds"`: every article that came from any feed.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, TsWire)]
#[serde(rename_all = "camelCase")]
pub enum ArticleFilter {
    Feed(i64),
    Feeds,
}

/// An article as a listing shows it: everything but its contents.
#[derive(Clone, Serialize, Deserialize, Projection, TsWire)]
#[projection(of = Article)]
pub struct ArticleSummary {
    pub id: i64,
    pub deleted: bool,
    /// The feed it came from.
    pub feed_id: i64,
    /// Whether it has been read.
    pub read: bool,
    pub url: String,
    pub title: String,
    pub author: Option<String>,
    /// In Unix seconds.
    pub published_at: i64,
    pub published_precision: PublishedPrecision,
    #[projection(group)]
    pub snapshot: ArticleSnapshot,
    /// In Unix seconds.
    pub created_at: i64,
    /// In Unix seconds.
    pub modified: i64,
}

/// How precisely an article's publication is known.
///
/// - `"time"`: the moment it was published, or brought in when neither its
///   feed nor its page gives a date.
/// - `"day"`: only the day: `publishedAt` is that day's midnight UTC, to be
///   shown as that date in every time zone.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum PublishedPrecision {
    #[default]
    Time,
    Day,
}

/// When a page or feed says something was published.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Published {
    /// In Unix seconds; midnight UTC of the day for a `Day`.
    pub at: i64,
    pub precision: PublishedPrecision,
}

impl Published {
    /// What an article whose page and feed give no date is published as:
    /// the moment `now` it was brought in.
    pub fn brought_in_at(now: i64) -> Self {
        Self {
            at: now,
            precision: PublishedPrecision::Time,
        }
    }
}

/// How an article's page was last fetched.
#[derive(Clone, Debug, Serialize, Deserialize, TsWire)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum ArticleSnapshot {
    /// The page was fetched and its text is the article's `contents`.
    Captured {
        /// The address the page was read from, after redirects.
        final_url: String,
        /// The page's own `<title>`, if it had one.
        page_title: Option<String>,
        /// When the page was fetched, in Unix seconds.
        fetched_at: i64,
        /// Whether the text was cut short because the page was too long.
        truncated: bool,
    },
    /// The page has never been fetched successfully.
    Failed {
        /// Why the last attempt failed.
        failure: ArticleFetchFailure,
        /// When it was attempted, in Unix seconds.
        attempted_at: i64,
    },
}

/// Why a page couldn't be fetched.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ArticleFetchFailure {
    /// No response arrived: the address didn't resolve, the connection
    /// failed or the server took too long.
    Unreachable { message: String },
    /// The server answered with an error status.
    Status { code: i64 },
    /// The page isn't HTML or plain text.
    UnsupportedContent { content_type: String },
    /// The page is larger than the daemon reads.
    TooLarge,
    /// A feed's address answered with something other than an RSS or Atom
    /// feed.
    NotAFeed,
}

/// A page as it was fetched, before it is stored as an article's snapshot.
#[derive(Clone, Debug)]
pub struct ArticleCapture {
    /// The address the page was read from, after redirects.
    pub final_url: String,
    /// The page's own `<title>`, if it had one.
    pub page_title: Option<String>,
    /// Who the page says wrote it, if it says.
    pub author: Option<String>,
    /// When the page says it was published, if it says.
    pub published: Option<Published>,
    /// The page's main content as Markdown, or a plain-text page as it was
    /// served.
    pub text: String,
    /// Whether `text` was cut short because the page was too long.
    pub truncated: bool,
}

/// What an article from `url` is titled when neither its feed nor its page
/// gives a title:
/// the last non-empty part of its path, else its host, else `url` itself.
pub fn fallback_article_title(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let rest = rest.split(['?', '#']).next().unwrap_or_default();
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let component = path.split('/').rfind(|part| !part.is_empty());
    match component {
        Some(component) => component.to_string(),
        None if !host.is_empty() => host.to_string(),
        None => url.to_string(),
    }
}

#[cfg(test)]
mod tests;
