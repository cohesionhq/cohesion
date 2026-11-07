//! Events a committed command raises for listeners outside the database, and
//! the trace of how many events led to each one.

use std::sync::Arc;

use cohesion_core::model::{
    Activity, ActorSession, ActorSessionMove, Feed, Initiative, Resource, Task,
};

/// Something a committed command did that listeners outside the database react to.
#[derive(Debug, PartialEq)]
pub enum DatabaseEventKind {
    TaskCreated {
        task_id: i64,
        workspace_id: i64,
    },
    TaskStatusChanged {
        task_id: i64,
        workspace_id: i64,
    },
    TaskRenamed {
        task_id: i64,
        workspace_id: i64,
    },
    TaskMoved {
        task_id: i64,
        workspace_id: i64,
    },
    TaskPriorityChanged {
        task_id: i64,
        workspace_id: i64,
    },
    TaskSizeChanged {
        task_id: i64,
        workspace_id: i64,
    },
    TaskAssigneeChanged {
        task_id: i64,
        workspace_id: i64,
    },
    TaskDescriptionChanged {
        task_id: i64,
        workspace_id: i64,
    },
    TaskStartDateChanged {
        task_id: i64,
        workspace_id: i64,
    },
    TaskDueDateChanged {
        task_id: i64,
        workspace_id: i64,
    },
    /// A tag was put on a task or taken off it.
    TaskTagged {
        task_id: i64,
        workspace_id: i64,
    },
    /// A task was deleted; it travels as it was, since a hook can no longer
    /// look it up.
    TaskDeleted {
        task: Task,
    },
    TaskFolderCreated {
        folder_id: i64,
        workspace_id: i64,
    },
    TaskFolderDeleted {
        folder_id: i64,
        workspace_id: i64,
    },
    WorkspaceCreated {
        workspace_id: i64,
    },
    WorkspaceDeleted {
        workspace_id: i64,
    },
    /// A journal's contents were saved: edited, or appended to.
    JournalSaved {
        document_id: i64,
        workspace_id: i64,
    },
    /// A document other than a journal was saved: edited, or appended to.
    DocumentSaved {
        document_id: i64,
        workspace_id: i64,
    },
    /// A resource was imported into a workspace, or imported again.
    ResourceImported {
        resource_id: i64,
        workspace_id: i64,
    },
    /// A refresh changed a resource.
    ResourceUpdated {
        resource_id: i64,
        workspace_id: i64,
    },
    /// A resource was removed; it travels as it was, since its row is gone.
    ResourceDeleted {
        resource: Resource,
    },
    /// A feed refresh brought in a new article.
    FeedArticleAdded {
        article_id: i64,
    },
    /// A feed was unfollowed; it travels as it was, since a hook can no
    /// longer look it up.
    FeedDeleted {
        feed: Feed,
    },
    /// An unread article was marked read.
    ArticleRead {
        article_id: i64,
    },
    /// A message landed in an agent's conversation.
    AgentMessageCreated {
        message_id: i64,
    },
    /// The user or the agent reacted to a message in an agent's
    /// conversation.
    AgentMessageReacted {
        message_id: i64,
    },
    /// An agent started a turn answering the user: a message, a pick or a
    /// reaction.
    AgentTurnStarted {
        agent_id: i64,
    },
    /// An agent's turn answering the user ended, however it went.
    AgentTurnEnded {
        agent_id: i64,
    },
    /// An activity started. Each activity event carries the row as its
    /// command wrote it, so a hook sees that moment's progress and status.
    ActivityStarted {
        activity: Activity,
    },
    /// An activity's progress, status or title changed while it went on.
    ActivityUpdated {
        activity: Activity,
    },
    /// An activity ended, however it went.
    ActivityEnded {
        activity: Activity,
    },
    /// An actor session's status changed, carrying the actor_session as its command
    /// wrote it, so a hook sees the status it moved to, and why it moved.
    ActorSessionMoved {
        actor_session: ActorSession,
        moved: ActorSessionMove,
    },
    /// A script or an agent sent the user a notification, which nothing
    /// stores: the event is all there is of it.
    NotificationSent {
        title: String,
        message: String,
    },
    /// A client was revoked, by the user or for not connecting; its name
    /// travels with it, since a hook can no longer look it up.
    ClientRevoked {
        client_id: i64,
        name: String,
    },
}
/// Who wrote the changes a trace follows from, so a listener can skip the
/// events its own writes caused.
#[derive(Clone)]
enum TraceOrigin {
    /// An extension, by its script identifier.
    Extension(Arc<String>),
    /// An agent's own script, by agent id.
    Agent(i64),
    /// A script run by hand, neither an extension's nor an agent's.
    Script,
}

/// Who a write command writes as, for the versions Rewind keeps.
#[derive(Clone, PartialEq, Eq)]
pub(crate) enum Writer {
    User,
    Agent(i64),
    Extension(Arc<String>),
    Script,
}

/// How far a chain of work has come, in two counts. `depth` counts the
/// events a chain of hooks has raised, which [`MAX_DEPTH`](Self::MAX_DEPTH)
/// bounds; `hops` counts the forks a chain has been handed through,
/// actor-session follow-ups and questions between agents alike, which is
/// logged as each fork wakes and bounds nothing. A hop wakes an agent
/// without raising an event, so it leaves `depth` alone, and an event is
/// not a hop.
#[derive(Clone, Default)]
pub struct DatabaseEventTrace {
    depth: usize,
    hops: usize,
    origin: Option<TraceOrigin>,
    /// Who started the chain: the user (a fresh trace), or an event, from
    /// the cue that started it on.
    initiative: Initiative,
}
impl DatabaseEventTrace {
    /// This trace, attributed to the extension whose script is writing.
    pub fn from_extension(mut self, identifier: Arc<String>) -> Self {
        self.origin = Some(TraceOrigin::Extension(identifier));
        self
    }
    /// This trace, attributed to the agent whose script is writing.
    pub fn from_agent(mut self, agent_id: i64) -> Self {
        self.origin = Some(TraceOrigin::Agent(agent_id));
        self
    }
    /// Who Rewind says wrote what this trace follows from; no origin is the
    /// user, from any client.
    pub(crate) fn writer(&self) -> Writer {
        match &self.origin {
            Some(TraceOrigin::Extension(identifier)) => Writer::Extension(identifier.clone()),
            Some(TraceOrigin::Agent(agent_id)) => Writer::Agent(*agent_id),
            Some(TraceOrigin::Script) => Writer::Script,
            None => Writer::User,
        }
    }
    /// This trace, attributed to a script run by hand.
    pub fn from_script(mut self) -> Self {
        self.origin = Some(TraceOrigin::Script);
        self
    }
    /// Whether the extension with `identifier` made the writes this trace follows from.
    pub fn is_from_extension(&self, identifier: &str) -> bool {
        matches!(&self.origin, Some(TraceOrigin::Extension(origin)) if **origin == identifier)
    }
    /// Whether agent `agent_id` made the writes this trace follows from.
    pub fn is_from_agent(&self, agent_id: i64) -> bool {
        matches!(self.origin, Some(TraceOrigin::Agent(origin)) if origin == agent_id)
    }
    pub fn depth(&self) -> usize {
        self.depth
    }
    /// The deepest an event may be and still wake hooks: hooks of an event
    /// deeper than this don't run. A change from outside (a client, the
    /// user's message, a timer) is depth 1. Actor-session follow-ups and agents
    /// asking each other count hops instead, which no limit holds.
    pub const MAX_DEPTH: usize = 3;
    /// Whether what this trace raises is past [`MAX_DEPTH`](Self::MAX_DEPTH),
    /// so it wakes no hooks.
    pub fn is_too_deep(&self) -> bool {
        self.depth > Self::MAX_DEPTH
    }
    /// This trace one level deeper: what a hook handling an event of it
    /// writes under.
    pub fn deeper(&self) -> Self {
        Self {
            depth: self.depth + 1,
            ..self.clone()
        }
    }
    pub fn hops(&self) -> usize {
        self.hops
    }
    pub fn initiative(&self) -> Initiative {
        self.initiative
    }
    /// This trace, as a chain `initiative` started: a cue's fork is an
    /// event's, and a follow-up keeps what started the chain that told the
    /// actor_session something.
    pub fn started_by(mut self, initiative: Initiative) -> Self {
        self.initiative = initiative;
        self
    }
    /// This trace one hop further: what an agent woken by an actor session it follows,
    /// or by a question, writes and asks under.
    pub fn hop(&self) -> Self {
        Self {
            hops: self.hops + 1,
            ..self.clone()
        }
    }
    pub fn event(&self, kind: DatabaseEventKind) -> DatabaseEvent {
        DatabaseEvent {
            kind,
            trace: self.deeper(),
        }
    }
}
pub struct DatabaseEvent {
    pub kind: DatabaseEventKind,
    pub trace: DatabaseEventTrace,
}

/// Events a command raised, held until it commits.
#[derive(Default)]
pub struct EventContext {
    events: Vec<DatabaseEvent>,
}

impl EventContext {
    /// Queues `event` for listeners once the command commits.
    pub fn send(&mut self, event: DatabaseEvent) {
        self.events.push(event);
    }

    /// Consumes the context, yielding each queued event in the order it was sent.
    pub(crate) fn into_events(self) -> impl Iterator<Item = DatabaseEvent> {
        self.events.into_iter()
    }
}

/// Receives events on the worker thread after the command that raised them commits.
pub trait DatabaseEventListener: std::fmt::Debug + Send + Sync + 'static {
    fn send(&self, event: DatabaseEvent);
}

/// Discards every event, for contexts nothing listens to (e.g. tests).
impl DatabaseEventListener for () {
    fn send(&self, _event: DatabaseEvent) {}
}

#[cfg(test)]
mod tests;
