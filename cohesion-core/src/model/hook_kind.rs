use cohesion_macros::{TextEnum, TsWire};

/// An event the daemon calls scripts for: extensions that list it in their
/// manifest's `hooks`, and agents with a cue on it. `launch` calls
/// `onLaunch()`; `task:create` calls `onTaskCreate(task)`, `task:status`
/// `onTaskStatus(task)`, and so on for each `task:*`, `taskfolder:*` and
/// `workspace:*` kind with its entity (`task:tagged` when a tag is put on or
/// taken off a task, `task:delete` with the task as it was deleted), except
/// `task:start` and `task:due`, which call `onTaskStart(task)` and
/// `onTaskDue(task)` at `day:start` for each incomplete task whose start or
/// due date is that day;
/// `journal:settled` calls `onJournalSettled(document)` two minutes after a
/// journal's last save, and `document:settled` `onDocumentSettled(document)`
/// the same for any other document;
/// `resource:import` calls `onResourceImport(resource)` when a resource is imported,
/// `resource:update` calls `onResourceUpdate(resource)` when a refresh changes one
/// and `resource:delete` `onResourceDelete(resource)` with one as it was removed;
/// `feed:article` calls `onFeedArticle(article)` for each new article a feed
/// refresh brings in, `feed:delete` `onFeedDelete(feed)` with a feed as it was
/// unfollowed and `article:read` `onArticleRead(article)` when one goes from
/// unread to read;
/// `agent:message` calls `onAgentMessage(message)` when a message lands in an
/// agent's conversation and `agent:reaction` calls `onAgentReaction(message)`
/// when the user or the agent reacts to one; `activity:started` calls
/// `onActivityStarted(activity)` when an activity starts,
/// `activity:updated` `onActivityUpdated(activity)` when its progress,
/// status or title changes and `activity:ended` `onActivityEnded(activity)` when it
/// ends; `actor:moved` calls `onActorMoved(session)` when an actor session's
/// status changes and `actor:resolved` calls `onActorResolved(session)` when
/// one is resolved; `agent:turn:start` calls
/// `onAgentTurnStart(agent)` when an agent starts answering the user and
/// `agent:turn:end` `onAgentTurnEnd(agent)` when it finishes; `day:start` calls `onDayStart()` at 6:00 and
/// `day:end` calls `onDayEnd()` at 23:00, and `timer` calls
/// `onTimer(timer)` at the start of every minute, in the daemon's local time;
/// `extension:update` calls `onExtensionUpdate(update)` once for each
/// version a registry offers an installed extension at, across restarts;
/// `notification:send` calls `onNotification(notification)` when a script or
/// an agent sends the user a notification, for an extension to push to
/// their devices; `client:revoke` calls `onClientRevoke(client)` when a
/// client is revoked, for an extension to forget what it keeps for it.
#[derive(Clone, Debug, TextEnum, PartialEq, TsWire)]
#[text_enum(rename_all = "custom")]
pub enum HookKind {
    #[text_enum("launch")]
    Launch,
    #[text_enum("task:create")]
    TaskCreate,
    #[text_enum("task:status")]
    TaskStatus,
    #[text_enum("task:rename")]
    TaskRename,
    #[text_enum("task:move")]
    TaskMove,
    #[text_enum("task:priority")]
    TaskPriority,
    #[text_enum("task:size")]
    TaskSize,
    #[text_enum("task:assigned")]
    TaskAssigned,
    #[text_enum("task:description")]
    TaskDescription,
    #[text_enum("task:startdate")]
    TaskStartDate,
    #[text_enum("task:duedate")]
    TaskDueDate,
    #[text_enum("task:start")]
    TaskStart,
    #[text_enum("task:due")]
    TaskDue,
    #[text_enum("task:tagged")]
    TaskTagged,
    #[text_enum("task:delete")]
    TaskDelete,
    #[text_enum("taskfolder:create")]
    TaskFolderCreate,
    #[text_enum("taskfolder:delete")]
    TaskFolderDelete,
    #[text_enum("workspace:create")]
    WorkspaceCreate,
    #[text_enum("workspace:delete")]
    WorkspaceDelete,
    #[text_enum("journal:settled")]
    JournalSettled,
    #[text_enum("document:settled")]
    DocumentSettled,
    #[text_enum("resource:import")]
    ResourceImport,
    #[text_enum("resource:update")]
    ResourceUpdate,
    #[text_enum("resource:delete")]
    ResourceDelete,
    #[text_enum("feed:article")]
    FeedArticle,
    #[text_enum("feed:delete")]
    FeedDelete,
    #[text_enum("article:read")]
    ArticleRead,
    #[text_enum("agent:message")]
    AgentMessage,
    #[text_enum("agent:reaction")]
    AgentReaction,
    #[text_enum("agent:turn:start")]
    AgentTurnStart,
    #[text_enum("agent:turn:end")]
    AgentTurnEnd,
    #[text_enum("activity:started")]
    ActivityStarted,
    #[text_enum("activity:updated")]
    ActivityUpdated,
    #[text_enum("activity:ended")]
    ActivityEnded,
    #[text_enum("actor:moved")]
    ActorMoved,
    #[text_enum("actor:resolved")]
    ActorResolved,
    #[text_enum("day:start")]
    DayStart,
    #[text_enum("day:end")]
    DayEnd,
    #[text_enum("timer")]
    Timer,
    #[text_enum("extension:update")]
    ExtensionUpdate,
    #[text_enum("notification:send")]
    NotificationSend,
    #[text_enum("client:revoke")]
    ClientRevoke,
}
