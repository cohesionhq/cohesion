//! Re-fetching an article's page: the page is fetched on a blocking thread,
//! then stored by one database command.

use cohesion_core::protocol::{
    Local, RefetchedArticle, ResnapshotArticleRequest, Route, ServerError,
};
use cohesion_db::{GetArticleCommand, ResnapshotArticleCommand};
use cohesion_internet::article_snapshot::{ArticleFetchAccess, snapshot_address};

use crate::cohesion_context::CohesionContext;

impl Route<Local, ResnapshotArticleRequest> for CohesionContext {
    async fn route(
        &self,
        request: ResnapshotArticleRequest,
    ) -> Result<RefetchedArticle, ServerError> {
        let article = self
            .database
            .send(GetArticleCommand {
                article_id: request.article_id,
            })
            .await?;
        let fetched = tokio::task::spawn_blocking(move || {
            snapshot_address(ArticleFetchAccess::granted(), &article.url)
        })
        .await
        .expect("fetching a page doesn't panic");
        Ok(self
            .database
            .send(ResnapshotArticleCommand {
                article_id: request.article_id,
                fetched,
            })
            .await?)
    }
}
