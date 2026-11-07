//! `Feed` in scripts.

use cohesion_core::model::{ArticleFilter, Feed};
use cohesion_db::{DeleteFeedCommand, ListArticlesCommand, RenameFeedCommand};
use cohesion_macros::js_entity;
use rquickjs::Ctx;

use super::article::JsArticle;

/// An RSS or Atom feed, followed once for every workspace. The daemon reads
/// every feed hourly; each item it hasn't seen becomes an unread `Article`,
/// part of the feed corpus (search it with `cohesion.searchCorpus()`). Get
/// them from `cohesion.feeds()`, in the user's order (`sortOrder`), or
/// `Article.feed()`. `createdAt` is in Unix seconds.
#[js_entity(Feed)]
impl JsFeed {
    /// Every article that came from this feed, most recently published first.
    pub fn articles<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Vec<JsArticle>> {
        let articles = self.read(
            &ctx,
            ListArticlesCommand {
                filter: ArticleFilter::Feed(self.inner.id),
            },
        )?;
        Ok(articles
            .into_iter()
            .map(|article| JsArticle::new(article, self.db.clone()))
            .collect())
    }

    /// Changes the title. An empty title throws. Not available in read-only
    /// contexts.
    pub fn rename<'js>(&mut self, ctx: Ctx<'js>, title: String) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            RenameFeedCommand {
                feed_id: self.inner.id,
                title,
            },
        )?;
        Ok(())
    }

    /// Stops following the feed and removes its articles. Not available in
    /// read-only contexts.
    pub fn delete<'js>(&mut self, ctx: Ctx<'js>) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            DeleteFeedCommand {
                feed_id: self.inner.id,
            },
        )?;
        Ok(())
    }
}
