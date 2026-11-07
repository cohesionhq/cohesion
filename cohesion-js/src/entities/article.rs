//! `Article` in scripts, and the plain objects its snapshot reads as.

use cohesion_core::model::{Article, ArticleFetchFailure, ArticleSnapshot};
use cohesion_core::ts_documentation::TsDocumentation;
use cohesion_db::{
    GetFeedCommand, RenameArticleCommand, ResnapshotArticleCommand, SetArticleReadCommand,
};
use cohesion_macros::js_entity;
use rquickjs::{Ctx, Exception, Value};
use serde::Serialize;

use cohesion_internet::article_snapshot::{ArticleFetchAccess, article_url, snapshot};

use super::feed::JsFeed;
use crate::conversion::ToJs;
use crate::globals::ArticleFetchMount;

/// A web page a feed brought in, with the text it had when it was last
/// fetched. Get them from `Feed.articles()` or `cohesion.searchCorpus()`.
///
/// Articles are shared by every workspace and make up the feed corpus that
/// `cohesion.searchCorpus()` searches. `contents` is the Markdown of the page's
/// main content (a plain-text page as it was served) as of `snapshot`, which
/// says how the last fetch went; it is `""` when the page has never been
/// fetched. `read` is whether someone has read it; new articles start unread.
/// `publishedAt` is when the feed or page says it was published, else when it
/// was brought in; when `publishedPrecision` is `"day"` only its UTC date is
/// known. Reading needs only read access; fetching again is for agents, not
/// extensions. The daemon never re-fetches a page on its own: call
/// `resnapshot()`. `createdAt` and `modified` are in Unix seconds.
#[js_entity(Article)]
impl JsArticle {
    /// The first 500 characters of `contents`, ending in `…` when there is
    /// more. Use it to tell items apart without reading each one in full.
    pub fn excerpt(&self) -> String {
        super::excerpt(&self.inner.contents)
    }

    /// Fetches the page again and replaces the snapshot and `contents`,
    /// updating this object. Returns `null` when the page was fetched, or
    /// why it couldn't be; an article fetched before then keeps its earlier
    /// snapshot. Not available in read-only contexts or to extensions.
    #[ts(returns = "ArticleFetchFailure | null")]
    pub fn resnapshot<'js>(&mut self, ctx: Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        crate::traced_command::ensure_writable(&ctx, &self.db)?;
        let access = article_fetch_access(&ctx)?;
        let url = article_url(&self.inner.url).expect("an article's URL parses");
        let refetched = self.write(
            &ctx,
            ResnapshotArticleCommand {
                article_id: self.inner.id,
                fetched: snapshot(access, &url),
            },
        )?;
        self.inner = refetched.article;
        match refetched.failure {
            Some(failure) => ScriptFetchFailure::from(&failure).to_js(&ctx),
            None => Ok(Value::new_null(ctx)),
        }
    }

    /// Changes the title. Saved immediately, and this object's `title` updates
    /// to match. An empty title throws. Not available in read-only contexts.
    pub fn rename<'js>(&mut self, ctx: Ctx<'js>, title: String) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            RenameArticleCommand {
                article_id: self.inner.id,
                title,
            },
        )?;
        Ok(())
    }

    /// Marks the article read. Not available in read-only contexts.
    #[qjs(rename = "markRead")]
    pub fn mark_read<'js>(&mut self, ctx: Ctx<'js>) -> rquickjs::Result<()> {
        self.set_read(&ctx, true)
    }

    /// Marks the article unread. Not available in read-only contexts.
    #[qjs(rename = "markUnread")]
    pub fn mark_unread<'js>(&mut self, ctx: Ctx<'js>) -> rquickjs::Result<()> {
        self.set_read(&ctx, false)
    }

    /// The feed the article came from, or `null` when that feed was deleted.
    pub fn feed<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Option<JsFeed>> {
        let feed = self.read(
            &ctx,
            GetFeedCommand {
                feed_id: self.inner.feed_id,
            },
        );
        match feed {
            Ok(feed) => Ok(Some(JsFeed::new(feed, self.db.clone()))),
            Err(_) => Ok(None),
        }
    }
}

impl JsArticle {
    fn set_read(&mut self, ctx: &Ctx<'_>, read: bool) -> rquickjs::Result<()> {
        self.inner = self.write(
            ctx,
            SetArticleReadCommand {
                article_id: self.inner.id,
                read,
            },
        )?;
        Ok(())
    }
}

/// What fetching an article's page throws in a sandbox that may not. Only
/// extensions reach it: every other sandbox that can write mounts
/// `ArticleFetch`.
const NO_ARTICLE_FETCH_MESSAGE: &str = "Fetching articles isn’t available to extensions; only you and your agents can fetch one again.";

/// The sandbox's access to fetch an article's page, throwing unless it mounts
/// `ArticleFetch`.
fn article_fetch_access(ctx: &Ctx<'_>) -> rquickjs::Result<ArticleFetchAccess> {
    match ctx.userdata::<ArticleFetchMount>() {
        Some(_) => Ok(ArticleFetchAccess::granted()),
        None => Err(Exception::throw_message(ctx, NO_ARTICLE_FETCH_MESSAGE)),
    }
}

/// A snapshot as scripts read it, with camelCase keys.
#[derive(Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum ScriptSnapshot<'a> {
    Captured {
        #[serde(rename = "finalUrl")]
        final_url: &'a str,
        #[serde(rename = "pageTitle")]
        page_title: Option<&'a str>,
        #[serde(rename = "fetchedAt")]
        fetched_at: i64,
        truncated: bool,
    },
    Failed {
        failure: ScriptFetchFailure<'a>,
        #[serde(rename = "attemptedAt")]
        attempted_at: i64,
    },
}

impl<'a> From<&'a ArticleSnapshot> for ScriptSnapshot<'a> {
    fn from(snapshot: &'a ArticleSnapshot) -> Self {
        match snapshot {
            ArticleSnapshot::Captured {
                final_url,
                page_title,
                fetched_at,
                truncated,
            } => Self::Captured {
                final_url,
                page_title: page_title.as_deref(),
                fetched_at: *fetched_at,
                truncated: *truncated,
            },
            ArticleSnapshot::Failed {
                failure,
                attempted_at,
            } => Self::Failed {
                failure: failure.into(),
                attempted_at: *attempted_at,
            },
        }
    }
}

impl TsDocumentation for ScriptSnapshot<'_> {
    const TS: &'static str = r#"/**
 * How an article's page was last fetched, by `state`:
 * - `"captured"`: the page was read; its text is the article's `contents`.
 *   `finalUrl` is where it was read from after redirects, `pageTitle` its
 *   own `<title>`, `fetchedAt` when (Unix seconds), and `truncated` whether
 *   the text was cut short because the page was too long.
 * - `"failed"`: the page has never been fetched; `failure` says why the
 *   last attempt, at `attemptedAt` (Unix seconds), didn't work.
 */
type ArticleSnapshot =
  | {
      state: "captured";
      finalUrl: string;
      pageTitle: string | null;
      fetchedAt: number;
      truncated: boolean;
    }
  | { state: "failed"; failure: ArticleFetchFailure; attemptedAt: number };"#;
    const TS_BRIEF: &'static str = Self::TS;
}

/// A fetch failure as scripts read it, with camelCase keys.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ScriptFetchFailure<'a> {
    Unreachable {
        message: &'a str,
    },
    Status {
        code: i64,
    },
    UnsupportedContent {
        #[serde(rename = "contentType")]
        content_type: &'a str,
    },
    TooLarge,
    NotAFeed,
}

impl<'a> From<&'a ArticleFetchFailure> for ScriptFetchFailure<'a> {
    fn from(failure: &'a ArticleFetchFailure) -> Self {
        match failure {
            ArticleFetchFailure::Unreachable { message } => Self::Unreachable { message },
            ArticleFetchFailure::Status { code } => Self::Status { code: *code },
            ArticleFetchFailure::UnsupportedContent { content_type } => {
                Self::UnsupportedContent { content_type }
            }
            ArticleFetchFailure::TooLarge => Self::TooLarge,
            ArticleFetchFailure::NotAFeed => Self::NotAFeed,
        }
    }
}

impl TsDocumentation for ScriptFetchFailure<'_> {
    const TS: &'static str = r#"/**
 * Why a page couldn't be fetched, by `kind`:
 * - `"unreachable"`: no response arrived (the address didn't resolve, the
 *   connection failed, or 20 seconds passed); `message` says what happened.
 * - `"status"`: the server answered with error status `code`, such as 404.
 * - `"unsupportedContent"`: the page isn't HTML or plain text; `contentType`
 *   is what the server said it is.
 * - `"tooLarge"`: the page is over 5 MiB.
 * - `"notAFeed"`: a feed's address answered with something other than an
 *   RSS or Atom feed.
 */
type ArticleFetchFailure =
  | { kind: "unreachable"; message: string }
  | { kind: "status"; code: number }
  | { kind: "unsupportedContent"; contentType: string }
  | { kind: "tooLarge" }
  | { kind: "notAFeed" };"#;
    const TS_BRIEF: &'static str = Self::TS;
}

/// Plain objects, as `ScriptSnapshot` declares them.
impl ToJs for ArticleSnapshot {
    fn to_js<'js>(&self, ctx: &Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        ScriptSnapshot::from(self).to_js(ctx)
    }
}
impl ToJs for ScriptSnapshot<'_> {
    fn to_js<'js>(&self, ctx: &Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        ctx.json_parse(serde_json::to_string(self).expect("an article snapshot serializes"))
    }
}
impl ToJs for ScriptFetchFailure<'_> {
    fn to_js<'js>(&self, ctx: &Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        ctx.json_parse(serde_json::to_string(self).expect("a fetch failure serializes"))
    }
}
