use clap::{Args, Subcommand};
use cohesion_core::model::{
    ArticleFetchFailure, ArticleFilter, ArticleSnapshot, ArticleSummary, FeedFullPagePolicy,
    FeedStatus, FeedSummary,
};
use cohesion_core::protocol::{
    AddFeedRequest, GetArticleRequest, ListArticlesRequest, ListFeedsRequest,
    MarkArticlesReadRequest, RefreshFeedRequest, RefreshFeedsRequest, ReorderFeedsRequest,
    SetArticleReadRequest,
};

use super::noun::Noun;
use super::table::{self, ago, truncate};
use crate::{cli_context::Context, cli_error::CliError, cli_macros::row, name_lookup::find};

#[derive(Subcommand)]
pub enum ArticleCommand {
    /// List the articles from every feed, or one, newest first; `•` marks unread ones.
    List(View),
    /// Print an article's snapshot (how its page was fetched, then the page's text) and mark it
    /// read.
    Show {
        /// Article id or title (titles match case-insensitively).
        #[arg(value_name = "ARTICLE")]
        target: String,
    },
    /// Mark an article read.
    Read {
        /// Article id or title (titles match case-insensitively).
        #[arg(value_name = "ARTICLE")]
        target: String,
    },
    /// Mark an article unread.
    Unread {
        /// Article id or title (titles match case-insensitively).
        #[arg(value_name = "ARTICLE")]
        target: String,
    },
    /// Mark every unread article from every feed, or one, read.
    MarkRead(View),
    /// Manage the feeds, which every workspace shares, or list them when no subcommand is given.
    Feed(Noun<FeedCommand>),
}

/// Which articles a command covers: every feed's unless a flag names one.
#[derive(Args)]
pub struct View {
    /// Every article from this feed (id or title).
    #[arg(long, value_name = "FEED")]
    feed: Option<String>,
}

impl View {
    /// The articles' filter.
    async fn filter(self, ctx: &Context) -> Result<ArticleFilter, CliError> {
        Ok(match self.feed {
            Some(feed) => ArticleFilter::Feed(find_feed(ctx, &feed).await?.id),
            None => ArticleFilter::Feeds,
        })
    }
}

#[derive(Subcommand)]
pub enum FeedCommand {
    /// Read an RSS or Atom feed and follow it; its items join the feed corpus.
    Add {
        /// The feed's address, starting with http:// or https://.
        url: String,
        /// Fetch every item's page, not only when the feed holds an excerpt.
        #[arg(long)]
        full_page: bool,
    },
    /// Read a feed now (id or title), or every feed when none is named.
    Refresh {
        #[arg(value_name = "FEED")]
        feed: Option<String>,
    },
    /// Move a feed (id or title) before or after another in the feed list.
    Move {
        #[arg(value_name = "FEED")]
        feed: String,
        /// The feed it goes just before.
        #[arg(
            long,
            value_name = "FEED",
            conflicts_with = "after",
            required_unless_present = "after"
        )]
        before: Option<String>,
        /// The feed it goes just after.
        #[arg(long, value_name = "FEED")]
        after: Option<String>,
    },
}

pub async fn run(ctx: &Context, verb: Option<ArticleCommand>) -> Result<(), CliError> {
    match verb {
        None => list(ctx, ArticleFilter::Feeds).await,
        Some(ArticleCommand::List(view)) => list(ctx, view.filter(ctx).await?).await,
        Some(ArticleCommand::Show { target }) => show(ctx, target).await,
        Some(ArticleCommand::Read { target }) => set_read(ctx, target, true).await,
        Some(ArticleCommand::Unread { target }) => set_read(ctx, target, false).await,
        Some(ArticleCommand::MarkRead(view)) => mark_read(ctx, view.filter(ctx).await?).await,
        Some(ArticleCommand::Feed(Noun { verb })) => match verb {
            None => list_feeds(ctx).await,
            Some(FeedCommand::Add { url, full_page }) => add_feed(ctx, url, full_page).await,
            Some(FeedCommand::Refresh { feed }) => refresh_feeds(ctx, feed).await,
            Some(FeedCommand::Move {
                feed,
                before,
                after,
            }) => move_feed(ctx, feed, before, after).await,
        },
    }
}

async fn list(ctx: &Context, filter: ArticleFilter) -> Result<(), CliError> {
    let articles = every_article(ctx, filter).await?;
    table::list(
        &articles,
        "No articles yet.\nFollow a feed with `cohesion article feed add <url>`.",
        |article| {
            let unread = if article.read { "" } else { "•" };
            let failed = match article.snapshot {
                ArticleSnapshot::Captured { .. } => "",
                ArticleSnapshot::Failed { .. } => "not fetched",
            };
            row![>article.id, unread, truncate(&article.title, 50), truncate(&article.url, 50), failed]
        },
    );
    Ok(())
}

async fn set_read(ctx: &Context, target: String, read: bool) -> Result<(), CliError> {
    let article = find_article(ctx, &target).await?;
    let article = ctx
        .call(SetArticleReadRequest {
            article_id: article.id,
            read,
        })
        .await?;
    let state = if read { "read" } else { "unread" };
    println!("Marked \"{}\" ({}) {state}.", article.title, article.id);
    Ok(())
}

async fn mark_read(ctx: &Context, filter: ArticleFilter) -> Result<(), CliError> {
    let marked = ctx.call(MarkArticlesReadRequest { filter }).await?;
    match marked.marked {
        1 => println!("Marked 1 article read."),
        count => println!("Marked {count} articles read."),
    }
    Ok(())
}

async fn list_feeds(ctx: &Context) -> Result<(), CliError> {
    let feeds = ctx.call(ListFeedsRequest).await?;
    table::list(
        &feeds,
        "No feeds yet.\nFollow one with `cohesion article feed add <url>`.",
        |feed| {
            let unread = match feed.unread {
                0 => String::new(),
                count => format!("{count} unread"),
            };
            row![>feed.id, truncate(&feed.title, 40), truncate(&feed.url, 50), unread, status(&feed.status)]
        },
    );
    Ok(())
}

async fn add_feed(ctx: &Context, url: String, full_page: bool) -> Result<(), CliError> {
    let feed = ctx
        .call(AddFeedRequest {
            url,
            full_page_policy: match full_page {
                true => FeedFullPagePolicy::Always,
                false => FeedFullPagePolicy::WhenExcerpt,
            },
        })
        .await?;
    println!(
        "Following \"{}\" ({}); {}.",
        feed.title,
        feed.id,
        status(&feed.status)
    );
    Ok(())
}

async fn refresh_feeds(ctx: &Context, feed: Option<String>) -> Result<(), CliError> {
    let feeds = match feed {
        Some(feed) => vec![
            ctx.call(RefreshFeedRequest {
                feed_id: find_feed(ctx, &feed).await?.id,
            })
            .await?,
        ],
        None => ctx.call(RefreshFeedsRequest).await?,
    };
    for feed in &feeds {
        println!("{}: {}.", feed.title, status(&feed.status));
    }
    Ok(())
}

async fn move_feed(
    ctx: &Context,
    feed: String,
    before: Option<String>,
    after: Option<String>,
) -> Result<(), CliError> {
    let feeds = ctx.call(ListFeedsRequest).await?;
    let moved = find(&feeds, &feed)?.id;
    let (anchor, offset) = match (before, after) {
        (Some(before), _) => (before, 0),
        (None, Some(after)) => (after, 1),
        (None, None) => unreachable!("clap requires --before or --after"),
    };
    let anchor = find(&feeds, &anchor)?.id;
    let mut order: Vec<i64> = feeds
        .iter()
        .map(|feed| feed.id)
        .filter(|id| *id != moved)
        .collect();
    let at = order
        .iter()
        .position(|id| *id == anchor)
        .map_or(order.len(), |at| at + offset);
    order.insert(at, moved);
    let feeds = ctx.call(ReorderFeedsRequest { feed_ids: order }).await?;
    for feed in &feeds {
        println!("{}", feed.title);
    }
    Ok(())
}

/// The feed whose id or title is `query`.
async fn find_feed(ctx: &Context, query: &str) -> Result<FeedSummary, CliError> {
    let feeds = ctx.call(ListFeedsRequest).await?;
    find(&feeds, query).cloned()
}

/// How a feed was last read, for the end of a sentence.
fn status(status: &FeedStatus) -> String {
    let ago = |at: i64| ago(jiff::Timestamp::now().as_second() - at);
    match status {
        FeedStatus::Refreshed { at, added } => format!("{added} new, read {}", ago(*at)),
        FeedStatus::Failed { failure, at } => {
            format!("couldn't read it {}: {}", ago(*at), describe(failure))
        }
    }
}

async fn show(ctx: &Context, target: String) -> Result<(), CliError> {
    let found = find_article(ctx, &target).await?;
    let article = ctx
        .call(GetArticleRequest {
            article_id: found.id,
        })
        .await?;
    println!("{}\n{}", article.title, article.url);
    match &article.snapshot {
        ArticleSnapshot::Captured {
            final_url,
            truncated,
            ..
        } => {
            if final_url != &article.url {
                println!("Read from {final_url}.");
            }
            if *truncated {
                println!("The page was too long. Its text is cut short.");
            }
            println!("\n{}", article.contents);
        }
        ArticleSnapshot::Failed { failure, .. } => {
            println!("\nCouldn't fetch the page: {}.", describe(failure));
        }
    }
    if !article.read {
        ctx.call(SetArticleReadRequest {
            article_id: article.id,
            read: true,
        })
        .await?;
    }
    Ok(())
}

/// Every article `filter` covers, read a page at a time.
async fn every_article(
    ctx: &Context,
    filter: ArticleFilter,
) -> Result<Vec<ArticleSummary>, CliError> {
    let mut articles = Vec::new();
    loop {
        let page = ctx
            .call(ListArticlesRequest {
                filter,
                offset: articles.len() as u64,
                limit: Some(500),
            })
            .await?;
        let last = page.articles.is_empty();
        articles.extend(page.articles);
        if last || articles.len() as i64 >= page.total {
            return Ok(articles);
        }
    }
}

/// The article from any feed whose id or title is `query`.
async fn find_article(ctx: &Context, query: &str) -> Result<ArticleSummary, CliError> {
    let articles = every_article(ctx, ArticleFilter::Feeds).await?;
    find(&articles, query).cloned()
}

/// Why a fetch failed, for the end of a sentence.
fn describe(failure: &ArticleFetchFailure) -> String {
    match failure {
        ArticleFetchFailure::Unreachable { message } => format!("no response ({message})"),
        ArticleFetchFailure::Status { code } => format!("the server answered {code}"),
        ArticleFetchFailure::UnsupportedContent { content_type } => {
            format!("it isn't a web page ({content_type})")
        }
        ArticleFetchFailure::TooLarge => "the page is over 5 MiB".to_string(),
        ArticleFetchFailure::NotAFeed => "it isn't an RSS or Atom feed".to_string(),
    }
}
