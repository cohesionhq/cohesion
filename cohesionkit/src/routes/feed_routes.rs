//! Requests that read a feed before storing it: following one and reading
//! one or every feed now.

use std::collections::HashSet;

use cohesion_core::model::{ArticleFetchFailure, Feed};
use cohesion_core::protocol::{
    AddFeedRequest, Local, RefreshFeedRequest, RefreshFeedsRequest, Route, ServerError,
};
use cohesion_db::{CreateFeedCommand, GetFeedCommand, ListFeedsCommand};
use cohesion_internet::article_snapshot::article_url;

use crate::cohesion_context::CohesionContext;
use crate::feeds::{read, refresh_feed, refresh_feeds};

impl Route<Local, AddFeedRequest> for CohesionContext {
    async fn route(&self, request: AddFeedRequest) -> Result<Feed, ServerError> {
        let url = article_url(&request.url).ok_or_else(|| {
            ServerError::validation(
                Some("url".to_string()),
                "Enter a feed address starting with http:// or https://.",
            )
        })?;
        let capture = read(url.clone(), request.full_page_policy, HashSet::new())
            .await
            .map_err(|failure| {
                ServerError::validation(Some("url".to_string()), unreadable(&failure))
            })?;
        let title = capture
            .title
            .unwrap_or_else(|| url.host_str().unwrap_or(url.as_str()).to_string());
        Ok(self
            .database
            .send(CreateFeedCommand {
                // The feed found from a page, after redirects.
                url: capture.url,
                title,
                full_page_policy: request.full_page_policy,
                items: capture.items,
            })
            .await?)
    }
}

/// Why a feed being added couldn't be read, as a sentence.
fn unreadable(failure: &ArticleFetchFailure) -> String {
    match failure {
        ArticleFetchFailure::Unreachable { .. } => "Couldn’t reach that address.".to_string(),
        ArticleFetchFailure::Status { code } => format!("The server answered {code}."),
        ArticleFetchFailure::UnsupportedContent { .. } | ArticleFetchFailure::NotAFeed => {
            "That address isn’t an RSS or Atom feed.".to_string()
        }
        ArticleFetchFailure::TooLarge => "That feed is over 5 MiB.".to_string(),
    }
}

impl Route<Local, RefreshFeedRequest> for CohesionContext {
    async fn route(&self, request: RefreshFeedRequest) -> Result<Feed, ServerError> {
        let feed = self
            .database
            .send(GetFeedCommand {
                feed_id: request.feed_id,
            })
            .await?;
        refresh_feed(self, &feed).await
    }
}

impl Route<Local, RefreshFeedsRequest> for CohesionContext {
    async fn route(&self, _: RefreshFeedsRequest) -> Result<Vec<Feed>, ServerError> {
        let feeds = self.database.send(ListFeedsCommand).await?;
        refresh_feeds(self, feeds).await.into_iter().collect()
    }
}
