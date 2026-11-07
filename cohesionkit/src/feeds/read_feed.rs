//! Reading one feed, for adding it or refreshing it.

use std::collections::HashSet;

use cohesion_core::model::{ArticleFetchFailure, Feed, FeedCapture, FeedFullPagePolicy};
use cohesion_core::protocol::ServerError;
use cohesion_db::{FeedItemUrlsCommand, RecordFeedCommand};
use cohesion_internet::article_snapshot::{ArticleFetchAccess, Url, article_url};
use cohesion_internet::feed::read_feed;

use crate::cohesion_context::CohesionContext;

/// Reads the feed at `url` and its items but those linking to `known`, off
/// the async runtime: the user, or the daemon's own schedule, asked for it.
pub(crate) async fn read(
    url: Url,
    policy: FeedFullPagePolicy,
    known: HashSet<String>,
) -> Result<FeedCapture, ArticleFetchFailure> {
    tokio::task::spawn_blocking(move || {
        read_feed(ArticleFetchAccess::granted(), &url, policy, &known)
    })
    .await
    .expect("reading a feed doesn't panic")
}

/// Reads live feed `feed` again and stores what it finds: its new items, or
/// why it couldn't be read. Items it already has aren't read again.
pub(crate) async fn refresh_feed(ctx: &CohesionContext, feed: &Feed) -> Result<Feed, ServerError> {
    let url = article_url(&feed.url).expect("a followed feed's URL parses");
    let known = ctx
        .database
        .send(FeedItemUrlsCommand { feed_id: feed.id })
        .await?;
    let capture = read(url, feed.full_page_policy, known).await;
    Ok(ctx
        .database
        .send(RecordFeedCommand {
            feed_id: feed.id,
            capture,
        })
        .await?)
}

/// Refreshes every feed of `feeds` at once, each result in `feeds`' order.
pub(crate) async fn refresh_feeds(
    ctx: &CohesionContext,
    feeds: Vec<Feed>,
) -> Vec<Result<Feed, ServerError>> {
    let tasks = feeds
        .into_iter()
        .map(|feed| {
            let ctx = ctx.clone();
            tokio::spawn(async move { refresh_feed(&ctx, &feed).await })
        })
        .collect::<Vec<_>>();
    let mut refreshed = Vec::with_capacity(tasks.len());
    for task in tasks {
        refreshed.push(task.await.expect("refreshing a feed doesn't panic"));
    }
    refreshed
}
