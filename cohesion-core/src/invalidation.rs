use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::model::LinkTarget;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TsWire)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum Invalidation {
    Workspace(i64),
    /// A model call's cost was recorded.
    IntelligenceCostEvents,
    /// Anything about an agent but its messages changed: its row, cues,
    /// agenda or runs.
    Agent(i64),
    /// Message `message_id` in agent `agent_id`'s conversation was added or
    /// changed (a reaction, an answer, a script's result), so a feed
    /// refreshes just that message.
    AgentMessage {
        agent_id: i64,
        message_id: i64,
    },
    /// The teams of agents changed: one was created, renamed or deleted.
    AgentTeams,
    /// A workspace's task folders changed: one was created, renamed or deleted.
    TaskFolders(i64),
    /// A workspace's documents changed: one was created, renamed, moved,
    /// pinned or deleted, journals included. An edit to a document's text
    /// alone is `DocumentText`.
    Documents(i64),
    /// The text of one document changed: it was edited or appended to, so
    /// its `modified` and opening did too. Its title, folder and the
    /// workspace's other documents didn't, so a list refreshes just its row.
    DocumentText {
        workspace_id: i64,
        document_id: i64,
    },
    /// A workspace's document folders changed: one was created, renamed,
    /// moved or deleted, or became a table or stopped being one.
    DocumentFolders(i64),
    /// The table of document folder `id` changed: a property was added,
    /// changed, moved or deleted, a value was set or computed, or a document
    /// came into the folder, left it, was renamed or deleted.
    DocumentTable(i64),
    /// The tasks in one list changed; a `None` folder is the inbox.
    TaskList {
        workspace_id: i64,
        folder_id: Option<i64>,
    },
    /// One task's description changed. No list filters or sorts by it, so
    /// a list refreshes just that task's row.
    TaskDescription {
        workspace_id: i64,
        task_id: i64,
    },
    /// The connector accounts changed: one was added, re-authenticated or
    /// removed.
    ConnectorAccounts,
    /// A workspace's resources changed: one was imported, refreshed or removed.
    Resources(i64),
    /// The kinds of resource shown in a workspace changed: one was shown or
    /// hidden there.
    ResourceKinds(i64),
    /// The feed list changed: a feed was added, refreshed, renamed,
    /// reordered, changed or deleted, or how many of a feed's items are
    /// unread changed. Feeds are shared by every workspace.
    Feeds,
    /// An article from a feed changed: one was added or removed, re-fetched,
    /// renamed, or marked read or unread. Feed listings (`{ feed: id }`,
    /// `feeds`) and corpus searches are shared by every workspace, so this is
    /// one key.
    FeedArticles,
    /// The intelligence settings changed: an endpoint was created or
    /// deleted, a model was added to or removed from one or linked to the
    /// catalog, a tier's default was set or cleared, or a spend limit was
    /// set or cleared.
    Intelligence,
    /// The clients changed: one was registered or deleted, or a client
    /// connected or disconnected.
    Clients,
    /// Security changed: the settings, or something was added to the
    /// security activity.
    Security,
    /// The storage settings changed.
    Storage,
    /// An experiment was turned on or off.
    Experiments,
    /// A module was turned on or off in a workspace.
    Modules(i64),
    /// The user's profile changed: their name or photo was set or removed.
    Profile,
    /// A workspace's tags changed: one was created, renamed or deleted.
    Tags(i64),
    /// A workspace's contacts changed: one was created, changed or deleted,
    /// or one of its methods was added, changed or removed.
    Contacts(i64),
    /// The calendars changed: a store was added, renamed, synced or
    /// removed, or a calendar was created, renamed, recolored or deleted.
    /// Calendars are shared by every workspace.
    Calendars,
    /// Workspace `id`'s perspective changed: which calendars it shows.
    CalendarPerspective(i64),
    /// Calendar events changed: one was created, changed or deleted, in
    /// Cohesion or by a sync. Calendars are shared by every workspace, so
    /// this is one key.
    CalendarEvents,
    /// A workspace's saved task views changed: one was created, changed,
    /// renamed, reordered or deleted, or lost a folder or tag it used.
    SavedTaskViews(i64),
    /// The extension updates on offer changed: a registry check finished,
    /// or an extension was installed, replaced or uninstalled.
    ExtensionUpdates,
    /// The memories changed: one was remembered or forgotten, or an agent's
    /// went with it.
    Memories,
    /// The skills changed: one was saved, changed or deleted, or an agent's
    /// or a team's went with it.
    AgentSkills,
    /// The actors changed: a client offered one, changed one or stopped
    /// offering it.
    Actors,
    /// The actor sessions changed: one was started, said something, changed
    /// where it stands or ended.
    ActorSessions,
    /// Actor session `id` changed as a whole: its context, a stretch of its
    /// work ending, its subagents, or a request from one of them.
    ActorSession(i64),
    /// Message `message_id` in actor session `session_id`'s transcript was
    /// added, changed or taken out, so a transcript refreshes just that
    /// message.
    ActorSessionMessage {
        session_id: i64,
        message_id: i64,
    },
    /// How much of its limits an actor has used changed: its client
    /// reported it, or the client that did went away.
    ActorUsage,
    /// What links to an item changed: a link to it was written or removed,
    /// or an item linking to it was edited, renamed or deleted.
    Backlinks(LinkTarget),
    /// A script or an agent sent the user a notification, which nothing
    /// stores, so the key carries it for clients to show.
    Notification {
        title: String,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TsWire)]
pub struct InvalidationBatch {
    pub keys: Vec<Invalidation>,
}

#[derive(Default)]
pub struct InvalidationContext {
    keys: HashSet<Invalidation>,
}

impl InvalidationContext {
    pub fn invalidate(&mut self, key: Invalidation) {
        self.keys.insert(key);
    }

    /// Drops every pending key, e.g. when the command that raised them failed.
    pub fn clear(&mut self) {
        self.keys.clear();
    }

    /// Removes and returns every pending key, to publish once a command finishes.
    pub fn take(&mut self) -> Vec<Invalidation> {
        self.keys.drain().collect()
    }
}
