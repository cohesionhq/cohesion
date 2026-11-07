use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{
    Article, ArticleCapture, ArticleFetchFailure, ArticleFilter, ArticleSnapshot, ArticleSummary,
    fallback_article_title,
};
use cohesion_core::protocol::{
    ArticlePage, ArticlesMarkedRead, GetArticleRequest, ListArticlesRequest,
    MarkArticlesReadRequest, RefetchedArticle, RenameArticleRequest, SetArticleReadRequest,
};

use cohesion_core::FromRequest;

use crate::changed::{Changed, Invalidates};
use crate::commands::page::page_size;
use crate::commands::title::ensure_title;
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, DatabaseEventKind, Filtered, Found,
    Read, ReadableDatabase, Write,
};

/// How fetching an article's page went.
pub type ArticleFetch = Result<ArticleCapture, ArticleFetchFailure>;

/// `article`, just written, having changed the feeds' listings.
fn changed(article: Article) -> Changed<Article> {
    article.invalidating([Invalidation::FeedArticles])
}

/// Like [`changed`], and also the feed list, whose unread counts follow
/// whether a feed's items are read and live.
fn counted(article: Article) -> Changed<Article> {
    changed(article).and([Invalidation::Feeds])
}

/// The snapshot a successful fetch at `now` stores.
pub(crate) fn captured(capture: &ArticleCapture, now: i64) -> ArticleSnapshot {
    ArticleSnapshot::Captured {
        final_url: capture.final_url.clone(),
        page_title: capture.page_title.clone(),
        fetched_at: now,
        truncated: capture.truncated,
    }
}

/// A live article by id.
fn live<'c>(db: impl ReadableDatabase<'c>, article_id: i64) -> DatabaseCommandResult<Article> {
    db.select_live()
        .filter("id = ?")
        .bind(article_id)
        .one()
        .found()
}

/// Stores a fresh fetch of live article `article`'s page. A failure leaves an article
/// whose page was fetched before as it is, and records itself on one never
/// fetched. An article never fetched takes the page's author and date, and
/// its title too while it still bears the one it fell back to.
fn record(
    ctx: &mut CommandContext<'_>,
    article: Article,
    fetched: ArticleFetch,
) -> DatabaseCommandResult<Changed<RefetchedArticle>> {
    let never_captured = matches!(article.snapshot, ArticleSnapshot::Failed { .. });
    let update = ctx
        .db
        .update::<Article>()
        .set_now(Article::MODIFIED)
        .filter_eq(Article::ID, article.id);
    let (update, failure) = match fetched {
        Ok(capture) => {
            let untitled = article.title == fallback_article_title(&article.url);
            let update = match &capture.page_title {
                Some(page_title) if never_captured && untitled => {
                    update.set(Article::TITLE, page_title.clone())
                }
                _ => update,
            };
            let update = match (&capture.author, never_captured) {
                (Some(author), true) => update.set(Article::AUTHOR, author.clone()),
                _ => update,
            };
            let update = match (capture.published, never_captured) {
                (Some(published), true) => update
                    .set(Article::PUBLISHED_AT, published.at)
                    .set(Article::PUBLISHED_PRECISION, published.precision),
                _ => update,
            };
            let update = update
                .set_group(captured(&capture, ctx.now))
                .set(Article::CONTENTS, capture.text);
            (update, None)
        }
        Err(failure) if never_captured => {
            let update = update.set_group(ArticleSnapshot::Failed {
                failure: failure.clone(),
                attempted_at: ctx.now,
            });
            (update, Some(failure))
        }
        Err(failure) => {
            let refetched = RefetchedArticle {
                article,
                failure: Some(failure),
            };
            return Ok(refetched.invalidating_nothing());
        }
    };
    let article = update.returning_one().found()?;
    Ok(changed(article).map(|article| RefetchedArticle { article, failure }))
}

/// Narrows `query` to the articles `filter` covers.
fn covering<Q: Filtered<Of = Article>>(query: Q, filter: ArticleFilter) -> Q {
    match filter {
        ArticleFilter::Feed(feed_id) => query.filter("feed_id = ?").bind(feed_id),
        ArticleFilter::Feeds => query,
    }
}

/// A page of the live articles `filter` covers, most recently published
/// first, without their contents.
#[derive(FromRequest)]
#[request(ListArticlesRequest)]
pub struct ListArticlePageCommand {
    pub filter: ArticleFilter,
    pub offset: u64,
    pub limit: Option<u64>,
}
impl DatabaseCommand for ListArticlePageCommand {
    type Response = ArticlePage;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<ArticlePage> {
        let listing = || covering(ctx.db.select_live_as::<ArticleSummary>(), self.filter);
        Ok(ArticlePage {
            articles: listing()
                .order("published_at DESC, id DESC")
                .limit(page_size(self.limit))
                .offset(self.offset)
                .all()?,
            total: listing().count()?,
        })
    }
}

/// The live articles `filter` covers, whole, most recently published first;
/// for scripts, which read their contents.
pub struct ListArticlesCommand {
    pub filter: ArticleFilter,
}
impl DatabaseCommand for ListArticlesCommand {
    type Response = Vec<Article>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Vec<Article>> {
        Ok(covering(ctx.db.select_live(), self.filter)
            .order("published_at DESC, id DESC")
            .all()?)
    }
}

/// Marks a live article read or unread.
#[derive(FromRequest)]
#[request(SetArticleReadRequest)]
pub struct SetArticleReadCommand {
    pub article_id: i64,
    pub read: bool,
}
impl DatabaseCommand for SetArticleReadCommand {
    type Response = Article;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Article>> {
        let was_read = live(ctx.db, self.article_id)?.read;
        let article = ctx
            .db
            .update_live()
            .set(Article::READ, self.read)
            .filter_eq(Article::ID, self.article_id)
            .returning_one()
            .found()?;
        if self.read && !was_read {
            ctx.events
                .send(ctx.trace.event(DatabaseEventKind::ArticleRead {
                    article_id: article.id,
                }));
        }
        Ok(counted(article))
    }
}

/// Marks every unread live article `filter` covers read.
#[derive(FromRequest)]
#[request(MarkArticlesReadRequest)]
pub struct MarkArticlesReadCommand {
    pub filter: ArticleFilter,
}
impl DatabaseCommand for MarkArticlesReadCommand {
    type Response = ArticlesMarkedRead;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<ArticlesMarkedRead>> {
        let unread = ctx
            .db
            .update_live::<Article>()
            .set_true(Article::READ)
            .filter("read = FALSE");
        let marked = covering(unread, self.filter).returning_all()?;
        let mut changes = ArticlesMarkedRead {
            marked: marked.len() as i64,
        }
        .invalidating_nothing();
        for article in marked {
            ctx.events
                .send(ctx.trace.event(DatabaseEventKind::ArticleRead {
                    article_id: article.id,
                }));
            changes = changes.merge(counted(article));
        }
        Ok(changes)
    }
}

/// A live article by id, whole.
#[derive(FromRequest)]
#[request(GetArticleRequest)]
pub struct GetArticleCommand {
    pub article_id: i64,
}
impl DatabaseCommand for GetArticleCommand {
    type Response = Article;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Article> {
        live(ctx.db, self.article_id)
    }
}

/// Stores a fresh fetch of live article `article_id`'s page. A failure leaves an
/// article whose page was fetched before as it is.
pub struct ResnapshotArticleCommand {
    pub article_id: i64,
    pub fetched: ArticleFetch,
}
impl DatabaseCommand for ResnapshotArticleCommand {
    type Response = RefetchedArticle;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<RefetchedArticle>> {
        let article = live(ctx.db, self.article_id)?;
        record(ctx, article, self.fetched)
    }
}

/// Changes a live article's title to a non-empty one.
#[derive(FromRequest)]
#[request(RenameArticleRequest)]
pub struct RenameArticleCommand {
    pub article_id: i64,
    pub title: String,
}
impl DatabaseCommand for RenameArticleCommand {
    type Response = Article;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Article>> {
        ensure_title(&self.title)?;
        let article = ctx
            .db
            .update_live()
            .set(Article::TITLE, self.title)
            .set_now(Article::MODIFIED)
            .filter_eq(Article::ID, self.article_id)
            .returning_one()
            .found()?;
        Ok(changed(article))
    }
}

#[cfg(test)]
mod tests;
