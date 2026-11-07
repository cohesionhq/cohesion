//! Shows what Cohesion makes of a web page or a feed, for building and
//! checking cohesion-internet's cleanup rules against real sites. It reads
//! the address the way the daemon does and prints the Markdown it would
//! store: a page's main content, or each of a feed's items, from the feed
//! or from its page. A tool for working on cohesion-internet; nothing
//! depends on it.
//!
//! ```sh
//! cargo run -p cohesion-internet-harness -- feed <url> > before.md
//! # change a rule
//! cargo run -p cohesion-internet-harness -- feed <url> > after.md
//! diff before.md after.md
//! ```
//!
//! `page <url>` reads it as a single article, `feed <url>` as a
//! feed being added, following a page to the feed it links to.

use std::collections::HashSet;
use std::process::ExitCode;

use cohesion_core::model::{ArticleCapture, ArticleFetchFailure, FeedFullPagePolicy};
use cohesion_internet::article_snapshot::{ArticleFetchAccess, Url, article_url, snapshot};
use cohesion_internet::feed::read_feed;

/// How to read the address.
enum Reading {
    Page,
    Feed,
}

const USAGE: &str = "usage: cohesion-internet-harness (page | feed) <url>";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (reading, text) = match args.as_slice() {
        [kind, text] if kind == "page" => (Reading::Page, text),
        [kind, text] if kind == "feed" => (Reading::Feed, text),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    let Some(url) = article_url(text) else {
        eprintln!("{text}: not an http(s) address");
        return ExitCode::FAILURE;
    };
    let access = ArticleFetchAccess::granted();
    let read = match reading {
        Reading::Page => page(access, &url),
        Reading::Feed => feed(access, &url),
    };
    match read {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => {
            eprintln!("{url}: {failure:?}");
            ExitCode::FAILURE
        }
    }
}

/// Prints `url` read as a page.
fn page(access: ArticleFetchAccess, url: &Url) -> Result<(), ArticleFetchFailure> {
    print_capture(&snapshot(access, url)?);
    Ok(())
}

/// Prints `url` read as a feed: its title, then each item's.
fn feed(access: ArticleFetchAccess, url: &Url) -> Result<(), ArticleFetchFailure> {
    let feed = read_feed(
        access,
        url,
        FeedFullPagePolicy::WhenExcerpt,
        &HashSet::new(),
    )?;
    println!("# {}\n", feed.title.as_deref().unwrap_or(&feed.url));
    println!("<!-- {} -->", feed.url);
    for item in feed.items {
        println!(
            "\n---\n\n## {}\n",
            item.title.as_deref().unwrap_or("Untitled")
        );
        println!("<!-- {} -->", item.url);
        match item.fetched {
            Ok(capture) => print_capture(&capture),
            Err(failure) => println!("<!-- unread: {failure:?} -->"),
        }
    }
    Ok(())
}

/// A page's or item's text, after a line saying where it came from: a
/// fetched page has a title, the feed's text doesn't.
fn print_capture(capture: &ArticleCapture) {
    let source = match &capture.page_title {
        Some(title) => format!("page {} “{title}”", capture.final_url),
        None => "feed".to_string(),
    };
    let truncated = if capture.truncated { ", truncated" } else { "" };
    println!("<!-- from {source}{truncated} -->\n\n{}", capture.text);
}
