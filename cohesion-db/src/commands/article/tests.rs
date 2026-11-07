use cohesion_core::model::{
    Article, ArticleCapture, ArticleFetchFailure, ArticleFilter, ArticleSnapshot,
    FeedFullPagePolicy, FeedItemCapture, Published, PublishedPrecision,
};

use crate::test_support::context;
use crate::{CreateFeedCommand, DatabaseContext, ListArticlesCommand, ResnapshotArticleCommand};

const URL: &str = "https://example.com/post";

fn capture(title: &str, text: &str) -> ArticleCapture {
    ArticleCapture {
        final_url: URL.to_string(),
        page_title: Some(title.to_string()),
        author: None,
        published: None,
        text: text.to_string(),
        truncated: false,
    }
}

fn unreachable() -> ArticleFetchFailure {
    ArticleFetchFailure::Unreachable {
        message: "timed out".to_string(),
    }
}

/// Follows a feed holding one undated, untitled item at `URL`; returns the item.
fn brought_in(
    db: &DatabaseContext,
    fetched: Result<ArticleCapture, ArticleFetchFailure>,
) -> Article {
    db.send_blocking(CreateFeedCommand {
        url: "https://example.com/feed.xml".to_string(),
        title: "feed".to_string(),
        full_page_policy: FeedFullPagePolicy::WhenExcerpt,
        items: vec![FeedItemCapture {
            url: URL.to_string(),
            title: None,
            author: None,
            published: None,
            fetched,
        }],
    })
    .unwrap();
    db.send_blocking(ListArticlesCommand {
        filter: ArticleFilter::Feeds,
    })
    .unwrap()
    .remove(0)
}

#[test]
fn a_failed_fetch_keeps_an_earlier_capture() {
    let db = context();
    let id = brought_in(&db, Ok(capture("Post", "text"))).id;
    let refetched = db
        .send_blocking(ResnapshotArticleCommand {
            article_id: id,
            fetched: Err(unreachable()),
        })
        .unwrap();
    assert_eq!(refetched.failure, Some(unreachable()));
    assert!(matches!(
        refetched.article.snapshot,
        ArticleSnapshot::Captured { .. }
    ));
    assert_eq!(refetched.article.contents, "text");
}

#[test]
fn an_article_never_fetched_takes_the_page_title_once_it_is() {
    let db = context();
    let article = brought_in(&db, Err(unreachable()));
    // Titled after the URL's last path component until the page is read.
    assert_eq!(article.title, "post");
    assert_eq!(article.published_at, article.created_at);
    assert!(matches!(article.snapshot, ArticleSnapshot::Failed { .. }));

    let refetched = db
        .send_blocking(ResnapshotArticleCommand {
            article_id: article.id,
            fetched: Ok(ArticleCapture {
                author: Some("Ada".to_string()),
                published: Some(Published {
                    at: 1_700_000_000,
                    precision: PublishedPrecision::Time,
                }),
                ..capture("Post", "text")
            }),
        })
        .unwrap();
    assert_eq!(refetched.failure, None);
    assert_eq!(refetched.article.title, "Post");
    assert_eq!(refetched.article.contents, "text");
    assert_eq!(refetched.article.author.as_deref(), Some("Ada"));
    assert_eq!(refetched.article.published_at, 1_700_000_000);
}

#[test]
fn a_page_without_a_date_counts_as_published_when_brought_in() {
    let db = context();
    let article = brought_in(&db, Ok(capture("Post", "text")));
    assert_eq!(article.published_at, article.created_at);
    assert_eq!(article.author, None);
    assert!(!article.read);
}
