//! Requests about articles: the items feeds bring in, with a snapshot of
//! their text. An article is shared by every workspace, so these take only its
//! id or a listing's filter.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

use crate::model::{Article, ArticleFetchFailure, ArticleFilter, ArticleSummary};

/// Lists one page of the live articles `filter` covers, most recently
/// published first, without their contents.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct ListArticlesRequest {
    pub filter: ArticleFilter,
    /// How many of the covered articles to skip; none when absent.
    #[serde(default)]
    pub offset: u64,
    /// The most articles to list: 50 when absent, and never over 500.
    pub limit: Option<u64>,
}

/// One page of a listing: its articles, and how many the whole listing has.
#[derive(Serialize, Deserialize, Clone, TsWire)]
pub struct ArticlePage {
    pub articles: Vec<ArticleSummary>,
    /// Every article the listing covers, on any page.
    pub total: i64,
}

/// Marks live article `article_id` read or unread.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetArticleReadRequest {
    pub article_id: i64,
    pub read: bool,
}

/// Marks every unread live article `filter` covers as read: "mark all as
/// read" for a view.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct MarkArticlesReadRequest {
    pub filter: ArticleFilter,
}

/// What marking a view read changed.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ArticlesMarkedRead {
    /// How many articles were unread and now are read.
    pub marked: i64,
}

/// Fetches live article `article_id`, whole. Fetching it doesn't mark it read.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct GetArticleRequest {
    pub article_id: i64,
}

/// Fetches live article `article_id`'s page again and replaces its snapshot.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ResnapshotArticleRequest {
    pub article_id: i64,
}

/// Retitles live article `article_id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RenameArticleRequest {
    pub article_id: i64,
    pub title: String,
}

/// An article after its page was fetched again.
#[derive(Serialize, Deserialize, Clone, TsWire)]
pub struct RefetchedArticle {
    pub article: Article,
    /// Why this fetch failed, when it did. An article whose page was fetched
    /// before keeps that snapshot; one never fetched records the failure.
    pub failure: Option<ArticleFetchFailure>,
}
