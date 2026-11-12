use cohesion_core::model::{
    ArticleCapture, ArticleFilter, FeedCapture, FeedFullPagePolicy, FeedItemCapture, FeedStatus,
    Published, PublishedPrecision, SearchEntityKind,
};

use crate::test_support::{context, recording_context};
use crate::{
    CreateFeedCommand, CreateWorkspaceCommand, DatabaseContext, DatabaseEventKind,
    DeleteFeedCommand, FeedItemUrlsCommand, ListArticlePageCommand, ListFeedSummariesCommand,
    MarkArticlesReadCommand, PruneFeedsCommand, RecordFeedCommand, RenameFeedCommand,
    ReorderFeedsCommand, SearchFeedCorpusCommand, SearchWorkspaceCommand, SetArticleReadCommand,
};

/// An item at `https://example.com/<slug>` about `text`, published at `published_at`.
fn item(slug: &str, text: &str, published_at: i64) -> FeedItemCapture {
    let url = format!("https://example.com/{slug}");
    FeedItemCapture {
        url: url.clone(),
        title: Some(format!("Item {slug}")),
        author: Some("Ada".to_string()),
        published: Some(Published {
            at: published_at,
            precision: PublishedPrecision::Time,
        }),
        fetched: Ok(ArticleCapture {
            final_url: url,
            page_title: None,
            author: None,
            published: None,
            text: text.to_string(),
            truncated: false,
        }),
    }
}

/// Follows the feed at `https://example.com/<name>.xml`.
fn follow_at(db: &DatabaseContext, name: &str, items: Vec<FeedItemCapture>) -> i64 {
    db.send_blocking(CreateFeedCommand {
        url: format!("https://example.com/{name}.xml"),
        title: name.to_string(),
        full_page_policy: FeedFullPagePolicy::WhenExcerpt,
        items,
    })
    .unwrap()
    .id
}

fn follow(db: &DatabaseContext, items: Vec<FeedItemCapture>) -> i64 {
    follow_at(db, "feed", items)
}

fn refresh(db: &DatabaseContext, feed: i64, items: Vec<FeedItemCapture>) -> i64 {
    let feed = db
        .send_blocking(RecordFeedCommand {
            feed_id: feed,
            capture: Ok(FeedCapture {
                url: String::new(),
                title: None,
                items,
            }),
        })
        .unwrap();
    match feed.status {
        FeedStatus::Refreshed { added, .. } => added,
        FeedStatus::Failed { .. } => panic!("the refresh was recorded as failed"),
    }
}

fn listed(db: &DatabaseContext, filter: ArticleFilter) -> Vec<String> {
    db.send_blocking(ListArticlePageCommand {
        filter,
        offset: 0,
        limit: None,
    })
    .unwrap()
    .articles
    .into_iter()
    .map(|article| article.url)
    .collect()
}

/// The id of the live article from any feed at `https://example.com/<slug>`.
fn item_id(db: &DatabaseContext, slug: &str) -> i64 {
    db.send_blocking(ListArticlePageCommand {
        filter: ArticleFilter::Feeds,
        offset: 0,
        limit: None,
    })
    .unwrap()
    .articles
    .into_iter()
    .find(|article| article.url.ends_with(&format!("/{slug}")))
    .unwrap()
    .id
}

fn corpus(db: &DatabaseContext, query: &str) -> Vec<String> {
    db.send_blocking(SearchFeedCorpusCommand {
        query: query.to_string(),
        feed_ids: None,
        offset: 0,
        limit: None,
    })
    .unwrap()
    .hits
    .into_iter()
    .map(|hit| hit.url)
    .collect()
}

/// `(title, unread)` of every feed, in its order.
fn feeds(db: &DatabaseContext) -> Vec<(String, i64)> {
    db.send_blocking(ListFeedSummariesCommand)
        .unwrap()
        .into_iter()
        .map(|feed| (feed.title, feed.unread))
        .collect()
}

#[test]
fn feed_items_join_the_corpus_but_no_workspaces_search() {
    let db = context();
    let ws = db
        .send_blocking(CreateWorkspaceCommand {
            title: "w".to_string(),
            emoji: None,
        })
        .unwrap()
        .id;
    let feed = follow(&db, vec![item("a", "sourdough starter", 2)]);
    follow_at(&db, "other", vec![item("b", "rye", 3)]);
    assert_eq!(corpus(&db, "sourdough"), ["https://example.com/a"]);
    assert_eq!(
        db.send_blocking(SearchWorkspaceCommand {
            workspace_id: ws,
            query: "sourdough".to_string(),
        })
        .unwrap()
        .len(),
        0
    );

    assert_eq!(
        listed(&db, ArticleFilter::Feeds),
        ["https://example.com/b", "https://example.com/a"]
    );
    assert_eq!(
        listed(&db, ArticleFilter::Feed(feed)),
        ["https://example.com/a"]
    );

    db.send_blocking(SetArticleReadCommand {
        article_id: item_id(&db, "a"),
        read: true,
    })
    .unwrap();
    assert_eq!(corpus(&db, "sourdough"), ["https://example.com/a"]);
}

#[test]
fn a_refresh_adds_only_new_items_and_removes_nothing() {
    let db = context();
    let feed = follow(&db, vec![item("a", "one", 1), item("b", "two", 2)]);
    // `a` dropped off the feed, `c` is new.
    assert_eq!(
        refresh(&db, feed, vec![item("b", "two", 2), item("c", "three", 3)]),
        1
    );
    assert_eq!(
        listed(&db, ArticleFilter::Feed(feed)),
        [
            "https://example.com/c",
            "https://example.com/b",
            "https://example.com/a"
        ]
    );
    // Another feed with the same item adds its own.
    let other = follow_at(&db, "other", vec![item("c", "three", 3)]);
    assert_eq!(listed(&db, ArticleFilter::Feed(other)).len(), 1);
}

#[test]
fn pruned_items_stay_gone_and_a_deleted_feed_takes_its_items() {
    let db = context();
    let items = || vec![item("old", "old", 10), item("new", "new", 30)];
    let feed = follow(&db, items());
    let other = follow_at(&db, "other", vec![item("elsewhere", "elsewhere", 10)]);

    let cleanup = db
        .send_blocking(PruneFeedsCommand {
            feed_ids: vec![feed],
            before: 20,
        })
        .unwrap();
    assert_eq!(cleanup.removed, 1);
    assert!(corpus(&db, "old").is_empty());
    assert_eq!(refresh(&db, feed, items()), 0);
    assert_eq!(
        listed(&db, ArticleFilter::Feed(feed)),
        ["https://example.com/new"]
    );

    db.send_blocking(DeleteFeedCommand { feed_id: feed })
        .unwrap();
    assert!(corpus(&db, "new").is_empty());
    assert_eq!(
        listed(&db, ArticleFilter::Feed(other)),
        ["https://example.com/elsewhere"]
    );
    assert_eq!(listed(&db, ArticleFilter::Feeds).len(), 1);
}

#[test]
fn a_refresh_knows_every_item_of_its_feed_pruned_ones_included() {
    let db = context();
    let feed = follow(&db, vec![item("old", "old", 10), item("new", "new", 30)]);
    follow_at(&db, "other", vec![item("elsewhere", "elsewhere", 30)]);
    db.send_blocking(PruneFeedsCommand {
        feed_ids: vec![feed],
        before: 20,
    })
    .unwrap();

    let mut known = db
        .send_blocking(FeedItemUrlsCommand { feed_id: feed })
        .unwrap()
        .into_iter()
        .collect::<Vec<_>>();
    known.sort();
    assert_eq!(
        known,
        ["https://example.com/new", "https://example.com/old"]
    );
}

#[test]
fn read_is_counted_per_feed_and_a_view_marks_what_it_covers() {
    let db = context();
    let feed = follow(&db, vec![item("a", "one", 1), item("b", "two", 2)]);
    follow_at(&db, "other", vec![item("c", "three", 3)]);
    assert_eq!(
        feeds(&db),
        [("feed".to_string(), 2), ("other".to_string(), 1)]
    );

    let read = db
        .send_blocking(SetArticleReadCommand {
            article_id: item_id(&db, "a"),
            read: true,
        })
        .unwrap();
    assert!(read.read);
    assert_eq!(feeds(&db)[0].1, 1);

    let marked = db
        .send_blocking(MarkArticlesReadCommand {
            filter: ArticleFilter::Feed(feed),
        })
        .unwrap();
    assert_eq!(marked.marked, 1);
    assert_eq!(
        feeds(&db),
        [("feed".to_string(), 0), ("other".to_string(), 1)]
    );

    let marked = db
        .send_blocking(MarkArticlesReadCommand {
            filter: ArticleFilter::Feeds,
        })
        .unwrap();
    assert_eq!(marked.marked, 1);
    assert_eq!(feeds(&db)[1].1, 0);
}

#[test]
fn feeds_are_followed_once_and_kept_in_order() {
    let db = context();
    let first = follow_at(&db, "first", vec![]);
    let second = follow_at(&db, "second", vec![]);
    let again = db.send_blocking(CreateFeedCommand {
        url: "https://example.com/first.xml".to_string(),
        title: "again".to_string(),
        full_page_policy: FeedFullPagePolicy::WhenExcerpt,
        items: vec![],
    });
    assert!(again.is_err());

    let reordered = db
        .send_blocking(ReorderFeedsCommand {
            feed_ids: vec![second, first],
        })
        .unwrap();
    assert_eq!(
        reordered.iter().map(|feed| feed.id).collect::<Vec<_>>(),
        [second, first]
    );
    // A new feed goes last.
    follow_at(&db, "third", vec![]);
    assert_eq!(
        feeds(&db)
            .into_iter()
            .map(|(title, _)| title)
            .collect::<Vec<_>>(),
        ["second", "first", "third"]
    );
}

#[test]
fn listings_and_searches_come_a_page_at_a_time_with_their_total() {
    let db = context();
    follow(
        &db,
        (1..=5).map(|n| item(&n.to_string(), "otters", n)).collect(),
    );

    let page = db
        .send_blocking(ListArticlePageCommand {
            filter: ArticleFilter::Feeds,
            offset: 1,
            limit: Some(2),
        })
        .unwrap();
    let urls: Vec<_> = page.articles.into_iter().map(|a| a.url).collect();
    assert_eq!(
        urls,
        ["https://example.com/4", "https://example.com/3"],
        "newest first, after skipping one"
    );
    assert_eq!(page.total, 5);

    let hits = db
        .send_blocking(SearchFeedCorpusCommand {
            query: "otters".to_string(),
            feed_ids: None,
            offset: 4,
            limit: Some(2),
        })
        .unwrap();
    assert_eq!((hits.hits.len(), hits.total), (1, 5));
}

#[test]
fn feed_and_article_events_follow_what_changed() {
    let (db, events) = recording_context();
    events.take();
    // Following a feed brings in its items quietly.
    let feed = follow(&db, vec![item("a", "first", 1)]);
    assert_eq!(events.take(), []);

    // A refresh raises one event per new item.
    refresh(
        &db,
        feed,
        vec![item("a", "first", 1), item("b", "second", 2)],
    );
    let raised = events.take();
    let &[DatabaseEventKind::FeedArticleAdded { article_id }] = raised.as_slice() else {
        panic!("one new item raises one FeedArticleAdded");
    };

    // Only unread to read counts, one at a time or in bulk.
    let read = |read| {
        db.send_blocking(SetArticleReadCommand { article_id, read })
            .unwrap();
        events.take()
    };
    assert_eq!(read(true), [DatabaseEventKind::ArticleRead { article_id }]);
    assert_eq!(read(true), []);
    assert_eq!(read(false), []);
    db.send_blocking(SetArticleReadCommand {
        article_id: item_id(&db, "a"),
        read: true,
    })
    .unwrap();
    events.take();
    read(false);
    db.send_blocking(MarkArticlesReadCommand {
        filter: ArticleFilter::Feed(feed),
    })
    .unwrap();
    assert_eq!(
        events.take(),
        [DatabaseEventKind::ArticleRead { article_id }]
    );

    db.send_blocking(DeleteFeedCommand { feed_id: feed })
        .unwrap();
    let raised = events.take();
    let [DatabaseEventKind::FeedDeleted { feed: deleted }] = raised.as_slice() else {
        panic!("unfollowing a feed raises FeedDeleted alone");
    };
    assert_eq!((deleted.id, deleted.deleted), (feed, true));
}

#[test]
fn a_live_feed_is_found_by_title_from_every_workspace() {
    let db = context();
    let [home, work] = ["home", "work"].map(|title| {
        db.send_blocking(CreateWorkspaceCommand {
            title: title.to_string(),
            emoji: None,
        })
        .unwrap()
        .id
    });
    let feed = follow_at(&db, "Bread weekly", vec![item("a", "sourdough", 1)]);
    let search = |workspace_id: i64, query: &str| -> Vec<(i64, i64)> {
        db.send_blocking(SearchWorkspaceCommand {
            workspace_id,
            query: query.to_string(),
        })
        .unwrap()
        .into_iter()
        .map(|hit| {
            assert!(matches!(hit.entity_kind, SearchEntityKind::Feed));
            (hit.entity_id, hit.workspace_id)
        })
        .collect()
    };
    // Each workspace finds it, as its own.
    assert_eq!(search(home, "bread"), [(feed, home)]);
    assert_eq!(search(work, "bread"), [(feed, work)]);

    db.send_blocking(RenameFeedCommand {
        feed_id: feed,
        title: "Rye digest".to_string(),
    })
    .unwrap();
    assert_eq!(search(home, "bread"), []);
    assert_eq!(search(home, "rye"), [(feed, home)]);

    db.send_blocking(DeleteFeedCommand { feed_id: feed })
        .unwrap();
    assert_eq!(search(home, "rye"), []);
}
