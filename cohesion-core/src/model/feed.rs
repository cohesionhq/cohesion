use cohesion_macros::{Fields, Model, Projection, TextEnum, TsWire};
use serde::{Deserialize, Serialize};

use super::article::{Article, ArticleCapture, ArticleFetchFailure, Published, PublishedPrecision};

/// An RSS or Atom feed, followed once for every workspace. Each refresh
/// adds the items it hasn't seen as articles, which make up the feed corpus;
/// nothing a refresh no longer sees is removed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "feeds", backup = Included)]
pub struct Feed {
    /// Unique, stable identifier.
    pub id: i64,
    /// Whether the feed has been deleted. Listings skip deleted feeds, so
    /// this is normally `false`.
    pub deleted: bool,
    /// The feed's address; unique among the live feeds.
    pub url: String,
    /// Display name: the feed's own title when it was added, until renamed.
    pub title: String,
    /// Where the feed sits among the feeds; lower comes first.
    pub sort_order: i64,
    /// When an item's page is fetched rather than read from the feed.
    #[fields(skip)]
    pub full_page_policy: FeedFullPagePolicy,
    /// How the last refresh went.
    #[fields(skip)]
    #[model(group)]
    pub status: FeedStatus,
    /// When the feed was added, in Unix seconds.
    pub created_at: i64,
}

/// When an item's text comes from its page rather than from the feed.
///
/// - `"whenExcerpt"`: only when the feed carries an excerpt, not the whole
///   article (the default).
/// - `"always"`: every item's page is fetched.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum FeedFullPagePolicy {
    #[default]
    WhenExcerpt,
    Always,
}

/// How a feed was last refreshed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TsWire)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum FeedStatus {
    /// The feed was read and `added` new items joined the corpus.
    Refreshed {
        /// When it was read, in Unix seconds.
        at: i64,
        /// How many items were new.
        added: i64,
    },
    /// The feed couldn't be read; the items already added stay.
    Failed {
        /// Why it couldn't.
        failure: ArticleFetchFailure,
        /// When it was attempted, in Unix seconds.
        at: i64,
    },
}

/// A feed as the feed list shows it, with how many of its items are unread.
#[derive(Clone, Serialize, Deserialize, Projection, TsWire)]
#[projection(of = Feed)]
pub struct FeedSummary {
    pub id: i64,
    pub deleted: bool,
    pub url: String,
    pub title: String,
    pub sort_order: i64,
    pub full_page_policy: FeedFullPagePolicy,
    #[projection(group)]
    pub status: FeedStatus,
    /// In Unix seconds.
    pub created_at: i64,
    /// How many of its live items haven't been read.
    #[projection(
        expr = "(SELECT COUNT(*) FROM articles WHERE articles.feed_id = feeds.id AND articles.deleted = FALSE AND articles.read = FALSE)"
    )]
    pub unread: i64,
}

/// One item of a feed as it was read, with its text taken from the feed or
/// its page.
#[derive(Clone, Debug)]
pub struct FeedItemCapture {
    /// The item's link, which is the article's address.
    pub url: String,
    /// The item's title in the feed, if it has one.
    pub title: Option<String>,
    /// Who the feed says wrote it.
    pub author: Option<String>,
    /// When the feed says it was published.
    pub published: Option<Published>,
    /// Its text: from the feed, or from its page when the feed held only an
    /// excerpt or the feed's policy says so.
    pub fetched: Result<ArticleCapture, ArticleFetchFailure>,
}

/// A feed as it was read, before it is stored.
#[derive(Clone, Debug)]
pub struct FeedCapture {
    /// The address of the feed that was read, after autodiscovery from a
    /// page and redirects; what to store as the feed's address.
    pub url: String,
    /// The feed's own title, if it has one.
    pub title: Option<String>,
    /// Its items, in the order the feed lists them.
    pub items: Vec<FeedItemCapture>,
}

/// The link of an item read from a feed, removed or not. Read only to
/// tell a refresh which items it needn't read again.
#[derive(Clone, Projection)]
#[projection(of = Article)]
pub struct FeedItemUrl {
    pub url: String,
}

/// An article from a feed matching a corpus search, with the passage that
/// matched. Read only by the corpus search, which joins each match as `hit`.
#[derive(Clone, Serialize, Deserialize, Projection, TsWire)]
#[projection(of = Article)]
pub struct FeedCorpusHit {
    pub id: i64,
    pub feed_id: i64,
    pub read: bool,
    pub url: String,
    pub title: String,
    pub author: Option<String>,
    /// In Unix seconds.
    pub published_at: i64,
    pub published_precision: PublishedPrecision,
    /// A few words of the article around the match, in Markdown.
    #[projection(expr = "hit.excerpt")]
    pub excerpt: String,
}
