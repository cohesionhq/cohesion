//! The events extensions can hook, and the queued form they travel in.

use std::sync::Arc;

use cohesion_core::model::{
    Activity, ActorSession, ActorSessionMove, Feed, HookKind, Resource, Task,
};
use cohesion_db::{DatabaseEventKind, DatabaseEventTrace, WorkGuard};
use cohesion_js::{Sandbox, ScriptArgument, ScriptError};

/// A hook event field's type: an id unless the table names another.
macro_rules! field_type {
    () => {
        i64
    };
    ($ty:ty) => {
        $ty
    };
}

/// Declares a struct per hook, with its kind, the global function it calls
/// and, for a hook a database event raises, that event and the entity its
/// function receives (built from the event's fields, which the struct
/// shares: ids, unless a field names its type); then [`event_hook`] from the
/// same table.
macro_rules! hook_events {
    ($(
        $name:ident: $kind:ident, $function:literal
            $(, from $event:ident => $argument:ident { $($field:ident $(: $ty:ty)?),* })?;
    )*) => {
        $(
            pub struct $name {
                $($(pub(super) $field: field_type!($($ty)?),)*)?
            }
            impl HookEvent for $name {
                fn kind(&self) -> HookKind {
                    HookKind::$kind
                }
                fn function_name(&self) -> &'static str {
                    $function
                }
                fn arguments(&self) -> Vec<ScriptArgument> {
                    vec![$(ScriptArgument::$argument { $($field: self.$field.clone()),* })?]
                }
            }
        )*

        /// The hook `event` raises. A journal or document save raises
        /// `journal:settled` or `document:settled`, which
        /// [`HookManager`](super::HookManager) holds back until it settles.
        /// Stale formulas raise no hook: the manager hands them to the
        /// formula runner before asking.
        pub(super) fn event_hook(event: DatabaseEventKind) -> Arc<dyn HookEvent> {
            match event {
                $($(DatabaseEventKind::$event { $($field),* } => Arc::new($name { $($field),* }),)?)*
                DatabaseEventKind::DatabaseFormulasStale { .. } => {
                    unreachable!("HookManager hands formula work to the formula runner")
                }
            }
        }
    };
}

hook_events! {
    LaunchHook: Launch, "onLaunch";
    DayStartHook: DayStart, "onDayStart";
    DayEndHook: DayEnd, "onDayEnd";
    TaskCreateHook: TaskCreate, "onTaskCreate",
        from TaskCreated => Task { task_id, workspace_id };
    TaskStatusHook: TaskStatus, "onTaskStatus",
        from TaskStatusChanged => Task { task_id, workspace_id };
    TaskRenameHook: TaskRename, "onTaskRename",
        from TaskRenamed => Task { task_id, workspace_id };
    TaskMoveHook: TaskMove, "onTaskMove",
        from TaskMoved => Task { task_id, workspace_id };
    TaskPriorityHook: TaskPriority, "onTaskPriority",
        from TaskPriorityChanged => Task { task_id, workspace_id };
    TaskSizeHook: TaskSize, "onTaskSize",
        from TaskSizeChanged => Task { task_id, workspace_id };
    TaskAssignedHook: TaskAssigned, "onTaskAssigned",
        from TaskAssigneeChanged => Task { task_id, workspace_id };
    TaskDescriptionHook: TaskDescription, "onTaskDescription",
        from TaskDescriptionChanged => Task { task_id, workspace_id };
    TaskStartDateHook: TaskStartDate, "onTaskStartDate",
        from TaskStartDateChanged => Task { task_id, workspace_id };
    TaskDueDateHook: TaskDueDate, "onTaskDueDate",
        from TaskDueDateChanged => Task { task_id, workspace_id };
    TaskTaggedHook: TaskTagged, "onTaskTagged",
        from TaskTagged => Task { task_id, workspace_id };
    TaskDeleteHook: TaskDelete, "onTaskDelete",
        from TaskDeleted => DeletedTask { task: Task };
    TaskFolderCreateHook: TaskFolderCreate, "onTaskFolderCreate",
        from TaskFolderCreated => TaskFolder { folder_id, workspace_id };
    TaskFolderDeleteHook: TaskFolderDelete, "onTaskFolderDelete",
        from TaskFolderDeleted => TaskFolder { folder_id, workspace_id };
    WorkspaceCreateHook: WorkspaceCreate, "onWorkspaceCreate",
        from WorkspaceCreated => Workspace { workspace_id };
    WorkspaceDeleteHook: WorkspaceDelete, "onWorkspaceDelete",
        from WorkspaceDeleted => Workspace { workspace_id };
    JournalSettledHook: JournalSettled, "onJournalSettled",
        from JournalSaved => Document { document_id, workspace_id };
    DocumentSettledHook: DocumentSettled, "onDocumentSettled",
        from DocumentSaved => Document { document_id, workspace_id };
    ResourceImportHook: ResourceImport, "onResourceImport",
        from ResourceImported => Resource { resource_id, workspace_id };
    ResourceUpdateHook: ResourceUpdate, "onResourceUpdate",
        from ResourceUpdated => Resource { resource_id, workspace_id };
    ResourceDeleteHook: ResourceDelete, "onResourceDelete",
        from ResourceDeleted => DeletedResource { resource: Resource };
    FeedArticleHook: FeedArticle, "onFeedArticle",
        from FeedArticleAdded => Article { article_id };
    FeedDeleteHook: FeedDelete, "onFeedDelete",
        from FeedDeleted => DeletedFeed { feed: Feed };
    ArticleReadHook: ArticleRead, "onArticleRead",
        from ArticleRead => Article { article_id };
    AgentMessageHook: AgentMessage, "onAgentMessage",
        from AgentMessageCreated => AgentMessage { message_id };
    AgentReactionHook: AgentReaction, "onAgentReaction",
        from AgentMessageReacted => AgentMessage { message_id };
    AgentTurnStartHook: AgentTurnStart, "onAgentTurnStart",
        from AgentTurnStarted => Agent { agent_id };
    AgentTurnEndHook: AgentTurnEnd, "onAgentTurnEnd",
        from AgentTurnEnded => Agent { agent_id };
    ActivityStartedHook: ActivityStarted, "onActivityStarted",
        from ActivityStarted => Activity { activity: Activity };
    ActivityUpdatedHook: ActivityUpdated, "onActivityUpdated",
        from ActivityUpdated => Activity { activity: Activity };
    ActivityEndedHook: ActivityEnded, "onActivityEnded",
        from ActivityEnded => Activity { activity: Activity };
    ActorMovedHook: ActorMoved, "onActorMoved",
        from ActorSessionMoved => ActorSession { actor_session: ActorSession, moved: ActorSessionMove };
    NotificationSendHook: NotificationSend, "onNotification",
        from NotificationSent => Notification { title: String, message: String };
    ClientRevokeHook: ClientRevoke, "onClientRevoke",
        from ClientRevoked => RevokedClient { client_id, name: String };
}

/// `task:start`, which calls `onTaskStart(task)` at the start of the day a
/// task starts on.
pub struct TaskStartHook {
    pub(super) task_id: i64,
    pub(super) workspace_id: i64,
}
impl HookEvent for TaskStartHook {
    fn kind(&self) -> HookKind {
        HookKind::TaskStart
    }
    fn function_name(&self) -> &'static str {
        "onTaskStart"
    }
    fn arguments(&self) -> Vec<ScriptArgument> {
        vec![ScriptArgument::Task {
            task_id: self.task_id,
            workspace_id: self.workspace_id,
        }]
    }
}

/// `task:due`, which calls `onTaskDue(task)` at the start of the day a task
/// is due.
pub struct TaskDueHook {
    pub(super) task_id: i64,
    pub(super) workspace_id: i64,
}
impl HookEvent for TaskDueHook {
    fn kind(&self) -> HookKind {
        HookKind::TaskDue
    }
    fn function_name(&self) -> &'static str {
        "onTaskDue"
    }
    fn arguments(&self) -> Vec<ScriptArgument> {
        vec![ScriptArgument::Task {
            task_id: self.task_id,
            workspace_id: self.workspace_id,
        }]
    }
}

/// `timer`, which calls `onTimer(timer)` with the minute that began.
pub struct TimerHook {
    /// When the minute began, in milliseconds since the epoch.
    pub(super) time: i64,
    /// The hour on the local clock, 0 to 23.
    pub(super) hour: i8,
    /// The minute on the local clock, 0 to 59.
    pub(super) minute: i8,
    /// When the minute the hook last fired for began, in milliseconds since
    /// the epoch: a minute before `time`, or earlier after the computer slept.
    pub(super) since: i64,
}
impl HookEvent for TimerHook {
    fn kind(&self) -> HookKind {
        HookKind::Timer
    }
    fn function_name(&self) -> &'static str {
        "onTimer"
    }
    fn arguments(&self) -> Vec<ScriptArgument> {
        vec![ScriptArgument::Json(serde_json::json!({
            "time": self.time,
            "hour": self.hour,
            "minute": self.minute,
            "since": self.since,
        }))]
    }
}

/// `extension:update`, which calls `onExtensionUpdate(update)` the first
/// time a registry offers an installed extension at a version.
pub struct ExtensionUpdateHook {
    pub(crate) extension_id: i64,
    /// The installed extension's manifest identifier.
    pub(crate) identifier: String,
    /// The installed extension's display name.
    pub(crate) name: String,
    pub(crate) installed_version: i64,
    /// The version the registry offers.
    pub(crate) version: i64,
    /// Where the registry serves the offered script.
    pub(crate) source_url: String,
}
impl HookEvent for ExtensionUpdateHook {
    fn kind(&self) -> HookKind {
        HookKind::ExtensionUpdate
    }
    fn function_name(&self) -> &'static str {
        "onExtensionUpdate"
    }
    fn arguments(&self) -> Vec<ScriptArgument> {
        vec![ScriptArgument::Json(serde_json::json!({
            "extensionId": self.extension_id,
            "identifier": self.identifier,
            "name": self.name,
            "installedVersion": self.installed_version,
            "version": self.version,
            "sourceURL": self.source_url,
        }))]
    }
}

pub trait HookEvent: 'static + Sync + Send {
    /// The hook this event runs, which extensions and agents' cues enable.
    fn kind(&self) -> HookKind;
    /// The global function the hook calls, for the logs.
    fn function_name(&self) -> &'static str;
    /// The entities handed to the hook's function, in order.
    fn arguments(&self) -> Vec<ScriptArgument>;
    fn evaluate(&self, sandbox: &Sandbox) -> Result<(), ScriptError> {
        sandbox.call_with_arguments(self.function_name(), &self.arguments())?;
        Ok(())
    }
}

pub struct HookInstance {
    pub hook: Arc<dyn HookEvent>,
    /// The one extension to run the hook in, instead of every extension with
    /// it enabled and the agents.
    pub extension_id: Option<i64>,
    pub trace: DatabaseEventTrace,
    /// Counts the hook as daemon work from queueing until it has run, and
    /// is only ever dropped.
    pub _running: WorkGuard,
}

#[cfg(test)]
mod tests;
