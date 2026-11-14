use std::collections::HashMap;

use cohesion_core::model::TagFilter;
use cohesion_core::protocol::Vocabulary;
use cohesion_db::{
    CreateAgentTeamCommand, CreateCalendarCommand, CreateCalendarStoreCommand, GetAgentTeamCommand,
    GetIntelligenceAgentCommand, GetWorkspaceCommand, GuardedDatabaseContext,
    ListAgentTeamsCommand, ListCalendarEventsCommand, ListCalendarStoresCommand,
    ListCalendarsCommand, ListFeedsCommand, ListIntelligenceAgentsCommand, ListTasksCommand,
    ListWorkspacesCommand, LookupWorkspaceByTitleCommand, SearchFeedCorpusArticlesCommand,
};
use rquickjs::function::Opt;
use rquickjs::{Ctx, Exception, JsLifetime, Type, Value, class::Trace};

use cohesion_macros::{JsOptions, js_global};

use super::global::Global;
use super::sleep;
use crate::conversion::{OneOrMore, plain};
use crate::entities::agent::JsAgent;
use crate::entities::agent_team::JsTeam;
use crate::entities::article::JsArticle;
use crate::entities::calendar::{
    CalendarEventListOptions, JsCalendar, JsCalendarEvent, JsCalendarStore, color_choice,
};
use crate::entities::feed::JsFeed;
use crate::entities::task::JsTask;
use crate::entities::workspace::JsWorkspace;
use crate::entities::workspace_options::TaskListOptions;
use crate::traced_command::{read, write};

#[derive(Trace, JsLifetime)]
#[rquickjs::class]
pub struct CohesionGlobal {
    #[qjs(skip_trace)]
    database: GuardedDatabaseContext,
}

/// The script API's entry point: start here to reach workspaces, and from
/// them documents and tasks.
///
/// How scripts run:
/// - A script is a plain script, not a module: no `import`/`export`, no
///   top-level `return`, and no top-level `await`.
/// - Everything is synchronous. API calls return values directly, never
///   Promises, so there is nothing to `await`.
/// - The script's result is the value of its **last expression statement**,
///   converted with `JSON.stringify`. There is no `console`; to report data,
///   end the script with an expression such as `results;` or `({ count, titles });`
///   (wrap object literals in parentheses). Ending with a declaration or a
///   statement that has no value yields `undefined`.
/// - Entities (`Workspace`, `Document`, `Task`, `SearchResult`) serialize to
///   plain objects of their properties, so they can be returned directly.
/// - Changes (assignments to writable properties, `rename`, `delete`,
///   `createTask`) are saved immediately; there is no separate save step.
/// - Entity objects are snapshots taken when fetched; they only update when
///   changed through that same object.
/// - `Workspace`, `Document` and `Task` carry string metadata for scripts'
///   own bookkeeping: `task.setMetadata("todoist_id", "123")`,
///   `task.metadata("todoist_id")`, and `ws.tasks({ metadata: { todoist_id: "123" } })`
///   to find it again (`{ hasMetadata: "todoist_id" }` matches any value).
/// - Failed calls throw an `Error` with a readable message; an uncaught throw
///   ends the script and becomes its error.
/// - Each script runs in a fresh environment, limited to 60 seconds and 20 MB.
///
/// ```js
/// const ws = cohesion.workspace("Personal");
/// const open = ws.search("invoice")
///   .filter(r => r.entityKind === "task")
///   .map(r => r.open())
///   .filter(t => t.status === TaskStatus.incomplete);
/// open.map(t => t.title);
/// ```
#[js_global("cohesion")]
#[rquickjs::methods]
impl CohesionGlobal {
    /// Every workspace that has not been deleted.
    pub fn workspaces<'js>(
        &self,
        ctx: Ctx<'js>,
        options: Opt<WorkspaceListOptions>,
    ) -> rquickjs::Result<Vec<JsWorkspace>> {
        let options = options.0.unwrap_or_default();
        let workspaces = read(
            &ctx,
            &self.database,
            ListWorkspacesCommand {
                metadata: options.metadata.unwrap_or_default(),
                has_metadata: options
                    .has_metadata
                    .map(OneOrMore::into_vec)
                    .unwrap_or_default(),
            },
        )?;
        Ok(workspaces
            .into_iter()
            .map(|workspace| JsWorkspace::new(workspace, self.database.clone()))
            .collect())
    }

    /// Every task that has not been deleted, across all workspaces and folders,
    /// in creation order unless `sort` says otherwise.
    ///
    /// Without a `status` option, only `"incomplete"` tasks.
    pub fn tasks<'js>(
        &self,
        ctx: Ctx<'js>,
        options: Opt<TaskListOptions>,
    ) -> rquickjs::Result<Vec<JsTask>> {
        let options = options.0.unwrap_or_default();
        let folder = options.folder_filter();
        let tasks = read(
            &ctx,
            &self.database,
            ListTasksCommand(options.into_listing(None, folder, TagFilter::default())),
        )?;
        Ok(tasks
            .into_iter()
            .map(|task| JsTask::new(task, self.database.clone()))
            .collect())
    }

    /// One workspace, by integer id or by exact (case-sensitive) title.
    ///
    /// Throws if no live workspace matches, if several share the title
    /// (look those up by id instead), or if `lookup` is neither an integer
    /// nor a string. To match titles loosely, filter `workspaces()` instead.
    pub fn workspace<'js>(
        &self,
        ctx: Ctx<'js>,
        #[ts(type = "number | string")] lookup: Value<'js>,
    ) -> rquickjs::Result<JsWorkspace> {
        let workspace = match lookup.type_of() {
            Type::Int => read(
                &ctx,
                &self.database,
                GetWorkspaceCommand {
                    id: lookup
                        .as_int()
                        .expect("Int values should be integers")
                        .into(),
                },
            )?,
            Type::String => read(
                &ctx,
                &self.database,
                LookupWorkspaceByTitleCommand {
                    title: lookup
                        .as_string()
                        .expect("String values should be strings")
                        .to_string()?,
                },
            )?,
            _ => {
                return Err(Exception::throw_syntax(
                    &ctx,
                    "A workspace may only be retrieved by title or identifier.",
                ));
            }
        };

        Ok(JsWorkspace::new(workspace, self.database.clone()))
    }

    /// Every feed followed, in the user's order.
    ///
    /// Feeds are shared by every workspace; their items are articles, which
    /// `searchCorpus()` searches.
    pub fn feeds<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Vec<JsFeed>> {
        let feeds = read(&ctx, &self.database, ListFeedsCommand)?;
        Ok(feeds
            .into_iter()
            .map(|feed| JsFeed::new(feed, self.database.clone()))
            .collect())
    }

    /// Every calendar store (shown to people as accounts): those only in
    /// Cohesion, then each calendar backend account's.
    #[qjs(rename = "calendarStores")]
    pub fn calendar_stores<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Vec<JsCalendarStore>> {
        let stores = read(&ctx, &self.database, ListCalendarStoresCommand {})?;
        Ok(stores
            .into_iter()
            .map(|store| JsCalendarStore::new(store, self.database.clone()))
            .collect())
    }

    /// Adds a store only in Cohesion, titled `title`, to make calendars in.
    /// An empty title throws. Not available in read-only contexts.
    #[qjs(rename = "createCalendarStore")]
    pub fn create_calendar_store<'js>(
        &self,
        ctx: Ctx<'js>,
        title: String,
    ) -> rquickjs::Result<JsCalendarStore> {
        let store = write(&ctx, &self.database, CreateCalendarStoreCommand { title })?;
        Ok(JsCalendarStore::new(store, self.database.clone()))
    }

    /// Every calendar, by store, then title.
    ///
    /// Calendars are shared by every workspace.
    pub fn calendars<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Vec<JsCalendar>> {
        let calendars = read(&ctx, &self.database, ListCalendarsCommand {})?;
        Ok(calendars
            .into_iter()
            .map(|calendar| JsCalendar::new(calendar, self.database.clone()))
            .collect())
    }

    /// Makes a calendar in a store only in Cohesion (`CalendarStore.accountId`
    /// is `null`); a backend's calendars come from the backend. Throws on an
    /// empty title or a color that isn't one. Not available in read-only
    /// contexts.
    #[qjs(rename = "createCalendar")]
    pub fn create_calendar<'js>(
        &self,
        ctx: Ctx<'js>,
        options: CalendarCreateOptions,
    ) -> rquickjs::Result<JsCalendar> {
        let color = color_choice(&ctx, &options.color)?;
        let calendar = write(
            &ctx,
            &self.database,
            CreateCalendarCommand {
                store_id: options.store_id,
                title: options.title,
                color,
                workspace_id: None,
            },
        )?;
        Ok(JsCalendar::new(calendar, self.database.clone()))
    }

    /// The events on every calendar that overlap `from` to `to`, in Unix
    /// seconds, by when they start.
    #[qjs(rename = "calendarEvents")]
    pub fn calendar_events<'js>(
        &self,
        ctx: Ctx<'js>,
        options: CalendarEventListOptions,
    ) -> rquickjs::Result<Vec<JsCalendarEvent>> {
        let events = read(
            &ctx,
            &self.database,
            ListCalendarEventsCommand {
                calendar_ids: None,
                from: options.from,
                to: options.to,
            },
        )?;
        Ok(events
            .into_iter()
            .map(|event| JsCalendarEvent::new(event, self.database.clone()))
            .collect())
    }

    /// Full-text search over every article that came from a feed: the feed
    /// corpus, which `Workspace.search()` doesn't cover.
    ///
    /// Matches title, author and page text, best first; `query` is FTS5 syntax,
    /// as in `Workspace.search()`. At most 200 results.
    #[qjs(rename = "searchCorpus")]
    pub fn search_corpus<'js>(
        &self,
        ctx: Ctx<'js>,
        query: String,
        options: Opt<CorpusSearchOptions>,
    ) -> rquickjs::Result<Vec<JsArticle>> {
        let articles = read(
            &ctx,
            &self.database,
            SearchFeedCorpusArticlesCommand {
                query,
                feed_ids: options.0.unwrap_or_default().feed_ids,
            },
        )?;
        Ok(articles
            .into_iter()
            .map(|article| JsArticle::new(article, self.database.clone()))
            .collect())
    }

    /// Every intelligence agent: the pinned ones in the user's order, then
    /// the rest, the latest to message first.
    pub fn agents<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Vec<JsAgent>> {
        let agents = read(&ctx, &self.database, ListIntelligenceAgentsCommand {})?;
        Ok(agents
            .into_iter()
            .map(|agent| JsAgent::new(agent, self.database.clone()))
            .collect())
    }

    /// One intelligence agent, by id, such as an `AgentMessage`'s `agentId`.
    ///
    /// Throws a `ReferenceError` if no agent has that id.
    pub fn agent<'js>(&self, ctx: Ctx<'js>, id: i64) -> rquickjs::Result<JsAgent> {
        let agent = read(
            &ctx,
            &self.database,
            GetIntelligenceAgentCommand { agent_id: id },
        )?;
        Ok(JsAgent::new(agent, self.database.clone()))
    }

    /// Every team of agents, by title.
    pub fn teams<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Vec<JsTeam>> {
        let teams = read(&ctx, &self.database, ListAgentTeamsCommand {})?;
        Ok(teams
            .into_iter()
            .map(|team| JsTeam::new(team, self.database.clone()))
            .collect())
    }

    /// One team of agents, by id, such as an `Agent`'s `teamId`.
    ///
    /// Throws a `ReferenceError` if no team has that id.
    pub fn team<'js>(&self, ctx: Ctx<'js>, id: i64) -> rquickjs::Result<JsTeam> {
        let team = read(&ctx, &self.database, GetAgentTeamCommand { team_id: id })?;
        Ok(JsTeam::new(team, self.database.clone()))
    }

    /// Creates a team of agents, with none on it yet; put agents on it with
    /// `Agent.setTeam(team.id)`. The title is unique among teams, ignoring
    /// case: an empty or taken one throws. Not available in read-only
    /// contexts.
    #[qjs(rename = "createTeam")]
    pub fn create_team<'js>(&self, ctx: Ctx<'js>, title: String) -> rquickjs::Result<JsTeam> {
        let team = write(&ctx, &self.database, CreateAgentTeamCommand { title })?;
        Ok(JsTeam::new(team, self.database.clone()))
    }

    /// How Cohesion's apps word its kinds, such as `"Needs you"` for an
    /// actor session that `"needsAttention"`: the same words the apps show, so
    /// a script needn't keep its own copy.
    ///
    /// Plain data, keyed as the apps receive it (`actor_session_statuses`,
    /// not `actorSessionStatuses`).
    #[ts(returns = "Vocabulary")]
    pub fn vocabulary<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        plain(&ctx, &Vocabulary::new())
    }

    /// Pauses the script for `seconds`, up to 30 seconds, then returns.
    ///
    /// Fractions are fine (`cohesion.sleep(0.5)`), and 0 returns at once.
    /// Throws a `RangeError` for a negative, non-finite or `NaN` value, for
    /// more than 30 seconds, or when the pause would pass the script's
    /// 60-second limit; sleeps in one run add up toward that limit. Throws
    /// an `Error` if the script is stopped (the user's Stop, or an
    /// extension being turned off or reloaded) while it sleeps.
    ///
    /// The script holds its thread while it sleeps: an extension's other
    /// hooks and timers wait behind it. Cues can't sleep (it throws); to
    /// act later from one, set a cue.
    pub fn sleep<'js>(&self, ctx: Ctx<'js>, seconds: f64) -> rquickjs::Result<()> {
        sleep(&ctx, seconds)
    }
}

/// Options for `cohesion.workspaces()`.
#[derive(JsOptions, Default)]
pub struct WorkspaceListOptions {
    /// Only workspaces carrying every one of these `setMetadata` pairs
    /// (`{ metadata: { todoist_id: "123" } }`).
    pub metadata: Option<HashMap<String, String>>,
    /// Only workspaces carrying this `setMetadata` key, or every one of
    /// these keys, whatever the value (`{ hasMetadata: "todoist_id" }`).
    pub has_metadata: Option<OneOrMore<String>>,
}

/// Options for `cohesion.createCalendar()`.
#[derive(JsOptions)]
pub struct CalendarCreateOptions {
    /// The store to make it in (`CalendarStore.id`), one only in Cohesion.
    pub store_id: i64,
    /// May not be empty.
    pub title: String,
    /// A palette color's name (`"blue"`) or an exact `"#rrggbb"`.
    pub color: String,
}
/// Options for `cohesion.searchCorpus()`.
#[derive(JsOptions, Default)]
pub struct CorpusSearchOptions {
    /// Only articles from the feeds with these ids (`Feed.id`); every feed
    /// when absent.
    pub feed_ids: Option<Vec<i64>>,
}

impl Global for CohesionGlobal {
    fn install<'a>(
        context: &Ctx<'a>,
        database: GuardedDatabaseContext,
    ) -> Result<(), rquickjs::Error> {
        context
            .globals()
            .set("cohesion", CohesionGlobal { database })?;
        Ok(())
    }
}
