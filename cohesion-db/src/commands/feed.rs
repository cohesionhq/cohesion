use std::collections::HashSet;

use cohesion_core::FromRequest;
use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{
    Article, ArticleFetchFailure, ArticleSnapshot, Feed, FeedCapture, FeedCorpusHit,
    FeedFullPagePolicy, FeedItemCapture, FeedItemUrl, FeedStatus, FeedSummary, Published,
    fallback_article_title,
};
use cohesion_core::protocol::{
    DeleteFeedRequest, FeedCleanup, FeedCorpusPage, ListFeedsRequest, PruneFeedsRequest,
    RenameFeedRequest, ReorderFeedsRequest, SearchFeedCorpusRequest, SetFeedFullPagePolicyRequest,
};
use cohesion_core::value::{IntoValue, Value, read_column};
use cohesion_core::{Model, Projection};

use crate::changed::{Changed, Invalidates};
use crate::command_error::searched;
use crate::commands::article::captured;
use crate::commands::page::page_size;
use crate::commands::title::ensure_title;
use crate::{
    CommandContext, CommandError, DatabaseCommand, DatabaseCommandResult, DatabaseEventKind,
    Filtered, Found, Read, ReadableDatabase, ValidationError, Write,
};

/// `feed`, just written, having changed the feed list.
fn changed(feed: Feed) -> Changed<Feed> {
    feed.invalidating([Invalidation::Feeds])
}

/// `count` items added or removed, having changed the feed items and the
/// feed list's unread counts when there were any.
fn items_changed(count: i64) -> Changed<i64> {
    let keys = (count > 0).then_some([Invalidation::FeedArticles, Invalidation::Feeds]);
    count.invalidating(keys.into_iter().flatten())
}

/// `?, ?, ?` for `count` values.
fn placeholders(count: usize) -> String {
    vec!["?"; count].join(", ")
}

/// Adds the items of `feed` no article from it has had the URL of, deleted
/// ones included, as unread articles; how many were new.
/// Nothing already there is changed or removed.
fn add_items(
    ctx: &mut CommandContext<'_>,
    feed: &Feed,
    items: Vec<FeedItemCapture>,
) -> DatabaseCommandResult<Changed<Vec<i64>>> {
    let mut added = Vec::new();
    for item in items {
        let seen = ctx
            .db
            .select::<Article>()
            .filter("feed_id = ? AND url = ?")
            .bind(feed.id)
            .bind(item.url.clone())
            .exists()?;
        if seen {
            continue;
        }
        let capture = item.fetched.as_ref().ok();
        let page_title = capture.and_then(|capture| capture.page_title.clone());
        let page_author = capture.and_then(|capture| capture.author.clone());
        let page_published = capture.and_then(|capture| capture.published);
        let published = item
            .published
            .or(page_published)
            .unwrap_or_else(|| Published::brought_in_at(ctx.now));
        let insert = ctx
            .db
            .insert::<Article>()
            .set(Article::FEED_ID, feed.id)
            .set(
                Article::TITLE,
                item.title
                    .or(page_title)
                    .unwrap_or_else(|| fallback_article_title(&item.url)),
            )
            .set(Article::AUTHOR, item.author.or(page_author))
            .set(Article::PUBLISHED_AT, published.at)
            .set(Article::PUBLISHED_PRECISION, published.precision);
        let insert = match item.fetched {
            Ok(capture) => insert
                .set_group(captured(&capture, ctx.now))
                .set(Article::CONTENTS, capture.text),
            Err(failure) => insert.set_group(ArticleSnapshot::Failed {
                failure,
                attempted_at: ctx.now,
            }),
        };
        let article = insert.set(Article::URL, item.url).returning_one().found()?;
        added.push(article.id);
    }
    Ok(items_changed(added.len() as i64).map(|_| added))
}

/// Every live feed in its order, with its unread count.
#[derive(FromRequest)]
#[request(ListFeedsRequest)]
pub struct ListFeedSummariesCommand;
impl DatabaseCommand for ListFeedSummariesCommand {
    type Response = Vec<FeedSummary>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Vec<FeedSummary>> {
        Ok(ctx
            .db
            .select_live_as::<FeedSummary>()
            .order("sort_order, id")
            .all()?)
    }
}

/// Every live feed in its order, whole; for refreshing them all.
pub struct ListFeedsCommand;
impl DatabaseCommand for ListFeedsCommand {
    type Response = Vec<Feed>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Vec<Feed>> {
        Ok(ctx.db.select_live().order("sort_order, id").all()?)
    }
}

/// The links of every item read from feed `feed_id`, removed ones
/// included: the items a refresh would add nothing for.
pub struct FeedItemUrlsCommand {
    pub feed_id: i64,
}
impl DatabaseCommand for FeedItemUrlsCommand {
    type Response = HashSet<String>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<HashSet<String>> {
        Ok(ctx
            .db
            .select_as::<FeedItemUrl>()
            .filter("feed_id = ?")
            .bind(self.feed_id)
            .all()?
            .into_iter()
            .map(|item| item.url)
            .collect())
    }
}

/// A live feed by id.
pub struct GetFeedCommand {
    pub feed_id: i64,
}
impl DatabaseCommand for GetFeedCommand {
    type Response = Feed;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Feed> {
        ctx.db
            .select_live()
            .filter("id = ?")
            .bind(self.feed_id)
            .one()
            .found()
    }
}

/// Follows the feed at `url`, which was just read, last in the order, adding
/// its items to the corpus without raising `FeedArticleAdded`: only a
/// refresh brings in news. A URL is followed once.
pub struct CreateFeedCommand {
    pub url: String,
    pub title: String,
    pub full_page_policy: FeedFullPagePolicy,
    pub items: Vec<FeedItemCapture>,
}
impl DatabaseCommand for CreateFeedCommand {
    type Response = Feed;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Feed>> {
        let followed = ctx
            .db
            .select_live::<Feed>()
            .filter_eq(Feed::URL, self.url.clone())
            .exists()?;
        if followed {
            return Err(CommandError::Validation(ValidationError::in_field(
                "url",
                "You already follow that feed.".to_string(),
            )));
        }
        let feed = ctx
            .db
            .insert::<Feed>()
            .set(Feed::URL, self.url)
            .set_max_incremented(Feed::SORT_ORDER)
            .set(Feed::TITLE, self.title)
            .set(Feed::FULL_PAGE_POLICY, self.full_page_policy)
            .set_group(FeedStatus::Refreshed {
                at: ctx.now,
                added: 0,
            })
            .returning_one()
            .found()?;
        add_items(ctx, &feed, self.items)?.and_then(|added| {
            let added = added.len() as i64;
            let feed = ctx
                .db
                .update::<Feed>()
                .set_group(FeedStatus::Refreshed { at: ctx.now, added })
                .filter_eq(Feed::ID, feed.id)
                .returning_one()
                .found()?;
            Ok(changed(feed))
        })
    }
}

/// Stores a fresh read of live feed `feed_id`: its new items join the
/// corpus, each raising `FeedArticleAdded`.
pub struct RecordFeedCommand {
    pub feed_id: i64,
    pub capture: Result<FeedCapture, ArticleFetchFailure>,
}
impl DatabaseCommand for RecordFeedCommand {
    type Response = Feed;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Feed>> {
        let feed = GetFeedCommand {
            feed_id: self.feed_id,
        }
        .handle(&mut ctx.as_read())?;
        let now = ctx.now;
        let status = match self.capture {
            Ok(capture) => add_items(ctx, &feed, capture.items)?.map(|added| {
                for &article_id in &added {
                    ctx.events.send(
                        ctx.trace
                            .event(DatabaseEventKind::FeedArticleAdded { article_id }),
                    );
                }
                FeedStatus::Refreshed {
                    at: now,
                    added: added.len() as i64,
                }
            }),
            Err(failure) => FeedStatus::Failed { failure, at: now }.invalidating_nothing(),
        };
        status.and_then(|status| {
            let feed = ctx
                .db
                .update::<Feed>()
                .set_group(status)
                .filter_eq(Feed::ID, feed.id)
                .returning_one()
                .found()?;
            Ok(changed(feed))
        })
    }
}

/// Changes a live feed's title.
#[derive(FromRequest)]
#[request(RenameFeedRequest)]
pub struct RenameFeedCommand {
    pub feed_id: i64,
    pub title: String,
}
impl DatabaseCommand for RenameFeedCommand {
    type Response = Feed;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Feed>> {
        ensure_title(&self.title)?;
        let feed = ctx
            .db
            .update_live()
            .set(Feed::TITLE, self.title)
            .filter_eq(Feed::ID, self.feed_id)
            .returning_one()
            .found()?;
        Ok(changed(feed))
    }
}

/// Sets when a live feed's items are read from their pages.
#[derive(FromRequest)]
#[request(SetFeedFullPagePolicyRequest)]
pub struct SetFeedFullPagePolicyCommand {
    pub feed_id: i64,
    pub full_page_policy: FeedFullPagePolicy,
}
impl DatabaseCommand for SetFeedFullPagePolicyCommand {
    type Response = Feed;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Feed>> {
        let feed = ctx
            .db
            .update_live()
            .set(Feed::FULL_PAGE_POLICY, self.full_page_policy)
            .filter_eq(Feed::ID, self.feed_id)
            .returning_one()
            .found()?;
        Ok(changed(feed))
    }
}

/// Gives each feed in `feed_ids` its position in the list as its sort order.
#[derive(FromRequest)]
#[request(ReorderFeedsRequest)]
pub struct ReorderFeedsCommand {
    pub feed_ids: Vec<i64>,
}
impl DatabaseCommand for ReorderFeedsCommand {
    type Response = Vec<FeedSummary>;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Vec<FeedSummary>>> {
        for (position, feed_id) in self.feed_ids.into_iter().enumerate() {
            ctx.db
                .update_live::<Feed>()
                .set(Feed::SORT_ORDER, position as i64)
                .filter_eq(Feed::ID, feed_id)
                .returning_one()
                .found()?;
        }
        let feeds = ListFeedSummariesCommand.handle(&mut ctx.as_read())?;
        Ok(feeds.invalidating([Invalidation::Feeds]))
    }
}

/// Soft-deletes a feed and removes its items.
#[derive(FromRequest)]
#[request(DeleteFeedRequest)]
pub struct DeleteFeedCommand {
    pub feed_id: i64,
}
impl DatabaseCommand for DeleteFeedCommand {
    type Response = Feed;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Feed>> {
        let feed = ctx
            .db
            .update_live()
            .set_true(Feed::DELETED)
            .filter_eq(Feed::ID, self.feed_id)
            .returning_one()
            .found()?;
        ctx.events.send(
            ctx.trace
                .event(DatabaseEventKind::FeedDeleted { feed: feed.clone() }),
        );
        let removed = remove_items(ctx, &[self.feed_id], None)?;
        Ok(changed(feed).merge(removed))
    }
}

/// Removes the live items of `feed_ids`, only those published before `before`
/// when given; how many it removed. A removed item keeps its row, without its
/// contents, so its URL isn't added again.
fn remove_items(
    ctx: &mut CommandContext<'_>,
    feed_ids: &[i64],
    before: Option<i64>,
) -> DatabaseCommandResult<Changed<i64>> {
    if feed_ids.is_empty() {
        return Ok(items_changed(0));
    }
    let mut update = ctx
        .db
        .update_live::<Article>()
        .set_true(Article::DELETED)
        .set(Article::CONTENTS, "")
        .set_now(Article::MODIFIED)
        .filter_in_values("feed_id", feed_ids.iter().copied());
    if let Some(before) = before {
        update = update.filter("published_at < ?").bind(before);
    }
    let removed = update.execute()? as i64;
    Ok(items_changed(removed))
}

/// Removes feeds' items published before a time.
#[derive(FromRequest)]
#[request(PruneFeedsRequest)]
pub struct PruneFeedsCommand {
    pub feed_ids: Vec<i64>,
    pub before: i64,
}
impl DatabaseCommand for PruneFeedsCommand {
    type Response = FeedCleanup;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<FeedCleanup>> {
        let removed = remove_items(ctx, &self.feed_ids, Some(self.before))?;
        Ok(removed.map(|removed| FeedCleanup { removed }))
    }
}

/// The `FROM … WHERE …` of a corpus search, and its parameters: the live
/// articles from feeds (only `feed_ids`' when given) matching FTS5 `query`. Each match joins as `hit`, with its `score` and `excerpt`.
fn corpus_search(query: String, feed_ids: Option<&[i64]>) -> (String, Vec<Value>) {
    let mut params = vec![query.into_value()];
    let feeds = match feed_ids {
        Some(feed_ids) => {
            params.extend(feed_ids.iter().map(|id| id.into_value()));
            format!("AND feed_id IN ({})", placeholders(feed_ids.len()))
        }
        None => String::new(),
    };
    let from = format!(
        "FROM articles
        INNER JOIN (
            SELECT rowid AS article_id,
                bm25(feed_corpus_fts, 5.0, 2.0, 1.0) AS score,
                snippet(feed_corpus_fts, 2, '', '', '…', 24) AS excerpt
            FROM feed_corpus_fts WHERE feed_corpus_fts MATCH ?
        ) AS hit ON hit.article_id = articles.id
        WHERE deleted = FALSE {feeds}"
    );
    (from, params)
}

/// A page of a corpus search selecting `columns`, best first: a title match
/// outweighs an author's, which outweighs one in the contents.
fn corpus_page(from: &str, columns: &str, offset: u64, limit: u64) -> String {
    format!("SELECT {columns} {from} ORDER BY hit.score LIMIT {limit} OFFSET {offset}")
}

/// A page of the articles from feeds matching `query`, best first, with the
/// passage that matched.
#[derive(FromRequest)]
#[request(SearchFeedCorpusRequest)]
pub struct SearchFeedCorpusCommand {
    pub query: String,
    pub feed_ids: Option<Vec<i64>>,
    pub offset: u64,
    pub limit: Option<u64>,
}
impl DatabaseCommand for SearchFeedCorpusCommand {
    type Response = FeedCorpusPage;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        let (from, params) = corpus_search(self.query, self.feed_ids.as_deref());
        let container = ctx.db.container();
        let page = corpus_page(
            &from,
            &FeedCorpusHit::columns(),
            self.offset,
            page_size(self.limit),
        );
        let total = searched(container.query_row(
            &format!("SELECT COUNT(*) AS value {from}"),
            &params,
            |row| read_column(row, "value"),
        ))?
        .unwrap_or(0);
        Ok(FeedCorpusPage {
            hits: container.query_rows(&page, &params, FeedCorpusHit::from_row)?,
            total,
        })
    }
}

/// The 200 articles from feeds best matching `query`, whole, best first;
/// for scripts.
pub struct SearchFeedCorpusArticlesCommand {
    pub query: String,
    pub feed_ids: Option<Vec<i64>>,
}
impl DatabaseCommand for SearchFeedCorpusArticlesCommand {
    type Response = Vec<Article>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Vec<Article>> {
        let (from, params) = corpus_search(self.query, self.feed_ids.as_deref());
        let page = corpus_page(&from, &Article::select_list(), 0, 200);
        searched(
            ctx.db
                .container()
                .query_rows(&page, &params, Article::from_row),
        )
    }
}

#[cfg(test)]
mod tests;
