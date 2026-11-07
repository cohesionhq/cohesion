//! Requests about feeds: RSS and Atom feeds followed once for every
//! workspace, whose items join the feed corpus.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

use crate::model::{FeedCorpusHit, FeedFullPagePolicy};

/// Lists the live feeds in their order, each with how many of its items
/// are unread.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListFeedsRequest;

/// Reads the feed at `url` (http or https) and follows it, last in the
/// order, adding its items to the feed corpus.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct AddFeedRequest {
    pub url: String,
    /// `"whenExcerpt"` when absent.
    #[serde(default)]
    pub full_page_policy: FeedFullPagePolicy,
}

/// Reads live feed `feed_id` now, adding the items it hasn't seen.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RefreshFeedRequest {
    pub feed_id: i64,
}

/// Reads every live feed now.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RefreshFeedsRequest;

/// Retitles live feed `feed_id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RenameFeedRequest {
    pub feed_id: i64,
    pub title: String,
}

/// Puts the feeds in the order of `feed_ids`, first to last.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ReorderFeedsRequest {
    /// Every live feed, in its new order.
    pub feed_ids: Vec<i64>,
}

/// Sets when live feed `feed_id`'s items are read from their pages. Items
/// already added keep their text.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetFeedFullPagePolicyRequest {
    pub feed_id: i64,
    pub full_page_policy: FeedFullPagePolicy,
}

/// Stops following feed `feed_id` and removes its items.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DeleteFeedRequest {
    pub feed_id: i64,
}

/// Removes the items of feeds `feed_ids` published before `before` (Unix
/// seconds). A removed item isn't added again when its feed is next read.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct PruneFeedsRequest {
    pub feed_ids: Vec<i64>,
    pub before: i64,
}

/// What cleaning up feeds removed.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct FeedCleanup {
    /// How many items were removed.
    pub removed: i64,
}

/// One page of the full-text matches among the articles that came from
/// feeds, best first. `query` is FTS5 syntax.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct SearchFeedCorpusRequest {
    pub query: String,
    /// Only articles from these feeds; every feed when absent.
    pub feed_ids: Option<Vec<i64>>,
    /// How many of the matches to skip; none when absent.
    #[serde(default)]
    pub offset: u64,
    /// The most matches to list: 50 when absent, and never over 500.
    pub limit: Option<u64>,
}

/// One page of a corpus search: its matches, and how many the whole search
/// has.
#[derive(Serialize, Deserialize, Clone, TsWire)]
pub struct FeedCorpusPage {
    pub hits: Vec<FeedCorpusHit>,
    /// Every match, on any page.
    pub total: i64,
}
