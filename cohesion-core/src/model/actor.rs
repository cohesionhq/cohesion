use cohesion_macros::{Fields, Model, Projection, TextEnum, TsInterface, TsWire};
use serde::{Deserialize, Serialize};

use super::{Client, Initiative};
use crate::stored_model::ColumnGroup;
use crate::value::{IntoValue, Row, RowError, Value, read_column};

/// An agent on a client's device, such as Claude Code in one directory,
/// that Cohesion hands sessions to: a client offers each in its `Hello`, and
/// works each session in the actor's own system, reporting back what it says
/// and where it stands.
///
/// An actor has no name of its own: every read takes its client's name
/// (a join of `clients`), so renaming the client renames its actors. One
/// its client no longer offers is soft-deleted, keeping its sessions.
#[derive(Clone, Debug, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "actors", backup = Included)]
#[model(join = Client, on = "clients.id = actors.client_id")]
pub struct Actor {
    pub id: i64,
    #[fields(skip)]
    pub deleted: bool,
    /// The client that offers it, and whose online state it shares.
    pub client_id: i64,
    /// What its client calls it, unique among the client's actors.
    pub identifier: String,
    /// The agent's product, such as "Claude Code".
    pub product: String,
    /// Where it works, such as a directory: `/Users/ada/Developer/cohesion`.
    pub scope: String,
    /// How it can run a session, the first the default.
    #[model(json)]
    pub modes: Vec<ActorMode>,
    /// The models it can run a session on, the first the default.
    #[model(json)]
    pub models: Vec<ActorModel>,
    /// The product on its client's current name, such as "Claude Code on
    /// Ada's MacBook Pro".
    #[model(expr = Actor::TITLE_SQL)]
    pub title: String,
}

impl Actor {
    /// The SQL of [`title`](Self::title).
    pub const TITLE_SQL: &'static str = "actors.product || ' on ' || clients.client_name";

    /// Whether it offers mode `mode`.
    pub fn offers_mode(&self, mode: &str) -> bool {
        self.modes.iter().any(|offered| offered.identifier == mode)
    }

    /// The model `model` it offers, if it does.
    pub fn model(&self, model: &str) -> Option<&ActorModel> {
        self.models
            .iter()
            .find(|offered| offered.identifier == model)
    }
}

/// An actor as its client offers it in `Hello`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TsWire)]
pub struct ActorDefinition {
    /// What the daemon names it by in calls; unique among the client's
    /// actors.
    pub identifier: String,
    /// The agent's product, such as "Claude Code".
    pub product: String,
    /// Where it works, such as a directory.
    pub scope: String,
    /// How it can run a session, the first the default; none when it has one
    /// way only.
    pub modes: Vec<ActorMode>,
    /// The models it can run a session on, the first the default; none when
    /// it picks for itself.
    pub models: Vec<ActorModel>,
}

/// One way an actor can run a session, such as sandboxed or accepting edits on
/// its own.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TsWire, TsInterface)]
pub struct ActorMode {
    pub identifier: String,
    /// The mode as people know it, such as "Auto".
    pub title: String,
    /// What it lets the actor do without asking.
    pub description: Option<String>,
}

/// A model an actor can run a session on, and how hard it can make it think.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TsWire, TsInterface)]
pub struct ActorModel {
    pub identifier: String,
    /// The model as people know it, such as "Opus".
    pub title: String,
    /// The reasoning efforts it takes, least first; none when it takes no
    /// setting.
    pub efforts: Vec<ActorEffort>,
}

/// A reasoning effort an actor's model takes.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TsWire, TsInterface)]
pub struct ActorEffort {
    pub identifier: String,
    /// The effort as people know it, such as "High".
    pub title: String,
}

/// One session an actor works on, such as a Claude Code session: a thread in
/// the actor's own system, which it reports to Cohesion as it goes. While
/// it works, or needs the user, an activity shows it. A session stays until the
/// user, or an agent for them, resolves it, and never ends for good: a
/// message to one that is done, failed or resolved starts it again.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "actor_sessions", backup = Included)]
pub struct ActorSession {
    pub id: i64,
    pub actor_id: i64,
    pub title: String,
    /// The actor's mode it runs in; `null` for the actor's default.
    pub mode: Option<String>,
    /// The actor's model it runs on; `null` for the actor's default.
    pub model: Option<String>,
    /// The model's reasoning effort; `null` for the model's default.
    pub effort: Option<String>,
    pub started_at: i64,
    /// When its status last changed, which orders the sessions.
    pub status_changed_at: i64,
    /// Where it stands, as its actor last reported.
    pub status: ActorSessionStatus,
    /// Whether its actor finished a stretch of work, done or failed, since
    /// the user last read it; stopping it, or its client going away, doesn't.
    /// Resolving it reads it.
    pub unread: bool,
    /// When the user last read it; `null` if they never have.
    pub read_at: Option<i64>,
    /// How full its context window is, as its actor last reported; `null`
    /// until it says.
    #[model(group)]
    #[fields(skip)]
    pub context: Option<ContextUsage>,
    /// The agent following it, the last to tell it something, which hears
    /// each time it stops on its own; `null` when none does.
    #[model(group)]
    #[fields(skip)]
    pub follower: Option<ActorSessionFollower>,
    /// What makes it a subagent: the session one level up whose actor
    /// launched it, and how; `null` for a session Cohesion started. A
    /// subagent leaves the lists, has no activity, is never unread and wakes
    /// no follower; it goes with the session above it.
    #[model(group)]
    #[fields(skip)]
    pub subagent: Option<ActorSubagent>,
}

impl ActorSession {
    /// The agent whose `following_count` counts this session: its follower,
    /// until it is resolved.
    pub fn counted_by(&self) -> Option<i64> {
        self.follower
            .filter(|_| self.status != ActorSessionStatus::Resolved)
            .map(|follower| follower.agent_id)
    }
}

/// The agent following an actor session, and who started the chain of work that
/// told the session something, which the follower's fork inherits.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, TsWire)]
pub struct ActorSessionFollower {
    pub agent_id: i64,
    pub initiative: Initiative,
}

/// A session's follower as its two columns. Deleting the agent clears its column
/// and leaves the other, which means nothing without it.
impl ColumnGroup for Option<ActorSessionFollower> {
    const COLUMNS: &'static [&'static str] = &["follower_agent_id", "follower_initiative"];

    fn from_row(row: &dyn Row) -> Result<Self, RowError> {
        let agent_id: Option<i64> = read_column(row, "follower_agent_id")?;
        let initiative: Initiative = read_column(row, "follower_initiative")?;
        Ok(agent_id.map(|agent_id| ActorSessionFollower {
            agent_id,
            initiative,
        }))
    }

    fn into_values(self) -> Vec<Value> {
        match self {
            Some(follower) => vec![
                follower.agent_id.into_value(),
                follower.initiative.into_value(),
            ],
            None => vec![Value::Null, Initiative::default().into_value()],
        }
    }
}

/// A subagent: a session an actor launched from inside another to do part
/// of its work, such as Claude Code's Task tool. Its client reports into it
/// under the session Cohesion started, naming it by `key`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TsWire)]
pub struct ActorSubagent {
    /// The session one level up: the one Cohesion started, or another
    /// subagent.
    pub parent_session_id: i64,
    /// The client's own id for it, unique among its actor's subagents.
    pub key: String,
    /// What kind of subagent the actor launched, as it names them, such as
    /// "Explore"; `null` when it didn't say.
    pub agent_type: Option<String>,
    /// The tool use in the transcript one level up that launched it; `null`
    /// when the client didn't say.
    pub message_id: Option<i64>,
    /// Whether it runs in the background, going on after the tool use that
    /// launched it has ended.
    pub background: bool,
}

/// A subagent as its five columns, `parent_session_id` and `subagent_key`
/// both `NULL` for a session Cohesion started; the table's check keeps them
/// together.
impl ColumnGroup for Option<ActorSubagent> {
    const COLUMNS: &'static [&'static str] = &[
        "parent_session_id",
        "subagent_key",
        "subagent_type",
        "subagent_message_id",
        "subagent_background",
    ];

    fn from_row(row: &dyn Row) -> Result<Self, RowError> {
        let parent: Option<i64> = read_column(row, "parent_session_id")?;
        let key: Option<String> = read_column(row, "subagent_key")?;
        let agent_type: Option<String> = read_column(row, "subagent_type")?;
        let message_id: Option<i64> = read_column(row, "subagent_message_id")?;
        let background: bool = read_column(row, "subagent_background")?;
        Ok(match (parent, key) {
            (Some(parent_session_id), Some(key)) => Some(ActorSubagent {
                parent_session_id,
                key,
                agent_type,
                message_id,
                background,
            }),
            _ => None,
        })
    }

    fn into_values(self) -> Vec<Value> {
        match self {
            Some(subagent) => vec![
                subagent.parent_session_id.into_value(),
                subagent.key.into_value(),
                subagent.agent_type.into_value(),
                subagent.message_id.into_value(),
                subagent.background.into_value(),
            ],
            None => vec![
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
                false.into_value(),
            ],
        }
    }
}

/// How full a session's context window is, in tokens.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TsWire)]
pub struct ContextUsage {
    /// The tokens the context holds now, never fewer than none.
    pub used_tokens: i64,
    /// The most it can hold, more than zero.
    pub total_tokens: i64,
}

/// A session's context as its two columns, both `NULL` until its actor
/// reports one; the table's check keeps them together.
impl ColumnGroup for Option<ContextUsage> {
    const COLUMNS: &'static [&'static str] = &["context_used_tokens", "context_total_tokens"];

    fn from_row(row: &dyn Row) -> Result<Self, RowError> {
        let used: Option<i64> = read_column(row, "context_used_tokens")?;
        let total: Option<i64> = read_column(row, "context_total_tokens")?;
        Ok(match (used, total) {
            (Some(used_tokens), Some(total_tokens)) => Some(ContextUsage {
                used_tokens,
                total_tokens,
            }),
            _ => None,
        })
    }

    fn into_values(self) -> Vec<Value> {
        match self {
            Some(context) => vec![
                context.used_tokens.into_value(),
                context.total_tokens.into_value(),
            ],
            None => vec![Value::Null, Value::Null],
        }
    }
}

/// An actor session matching a full-text search of the sessions' titles and
/// what the user and the actor said in them, with the passage that
/// matched. Read only by the search, which joins each match as `hit`.
#[derive(Clone, Debug, Serialize, Deserialize, Projection, TsWire)]
#[projection(of = ActorSession)]
pub struct ActorSessionSearchHit {
    pub id: i64,
    pub actor_id: i64,
    pub title: String,
    pub status_changed_at: i64,
    /// A few words of the message that matched, in Markdown; `null` when
    /// the title matched best.
    #[projection(expr = "hit.excerpt")]
    pub excerpt: Option<String>,
}

/// Where an actor session stands.
///
/// - `"working"`: the actor is at it.
/// - `"needsAttention"`: the actor is waiting on the user, such as for a
///   response to a request.
/// - `"idle"`: done, until the user says more.
/// - `"failed"`: the actor couldn't go on; the user may say more to try
///   again.
/// - `"resolved"`: the user, or an agent for them, is done with it; lists
///   of sessions leave it out, and it is deleted once it has sat resolved
///   for the actor session retention. The user may say more to start it
///   again.
///
/// A working session, or one that needs attention, has an activity open, which
/// ends succeeded as it goes idle and failed as it fails.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum ActorSessionStatus {
    Working,
    NeedsAttention,
    Idle,
    Failed,
    Resolved,
}

/// Where an actor reports a session stands: any status but resolved, which
/// only the user, or an agent for them, decides.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ReportedActorSessionStatus {
    Working,
    NeedsAttention,
    Idle,
    Failed,
}

/// Why an actor session's status changed, which only what runs actor sessions
/// knows: a session goes idle when its actor finishes, when someone stops it
/// and when its client goes away. The session's follower hears every move but
/// the ones nobody needs to hear of. A resolved session stays resolved through
/// every move to rest: only work, or a put back, reopens it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActorSessionMove {
    /// The actor is at it.
    Working,
    /// The actor waits on the user.
    NeedsYou,
    /// The actor finished its turn.
    Finished,
    /// The actor couldn't go on.
    Failed,
    /// The session's client went away mid-turn.
    Interrupted,
    /// The user or a script stopped it.
    Stopped,
    /// The user, or an agent for them, resolved it, stopping it first if
    /// it was going.
    Resolved,
    /// A message that never reached the actor put it back in the status it
    /// had before.
    PutBack(ActorSessionStatus),
}

impl ActorSessionMove {
    /// The move the session's actor reports by its new `status`.
    pub fn reported(status: ReportedActorSessionStatus) -> Self {
        match status {
            ReportedActorSessionStatus::Working => Self::Working,
            ReportedActorSessionStatus::NeedsAttention => Self::NeedsYou,
            ReportedActorSessionStatus::Idle => Self::Finished,
            ReportedActorSessionStatus::Failed => Self::Failed,
        }
    }

    /// The status the session stands in after this move.
    pub fn status(self) -> ActorSessionStatus {
        match self {
            Self::Working => ActorSessionStatus::Working,
            Self::NeedsYou => ActorSessionStatus::NeedsAttention,
            Self::Finished | Self::Interrupted | Self::Stopped => ActorSessionStatus::Idle,
            Self::Failed => ActorSessionStatus::Failed,
            Self::Resolved => ActorSessionStatus::Resolved,
            Self::PutBack(status) => status,
        }
    }

    /// Whether a resolved session stays resolved through this move: a move to
    /// rest (its actor finishing, failing or going away after it was
    /// resolved, or a stop) leaves it be; work, or a put back, reopens it.
    pub fn keeps_resolved(self) -> bool {
        match self {
            Self::Finished | Self::Failed | Self::Interrupted | Self::Stopped => true,
            Self::Working | Self::NeedsYou | Self::PutBack(_) | Self::Resolved => false,
        }
    }

    /// How the session stopped on its own, which wakes its follower; `None`
    /// when it started working, someone stopped or resolved it, or it was
    /// put back.
    pub fn stop(self) -> Option<ActorSessionStop> {
        match self {
            Self::NeedsYou => Some(ActorSessionStop::NeedsYou),
            Self::Finished => Some(ActorSessionStop::Finished),
            Self::Failed => Some(ActorSessionStop::Failed),
            Self::Interrupted => Some(ActorSessionStop::Interrupted),
            Self::Working | Self::Stopped | Self::Resolved | Self::PutBack(_) => None,
        }
    }
}

/// How an actor session stopped on its own: each wakes the agent following it.
///
/// - `"needsYou"`: the actor waits on the user.
/// - `"finished"`: the actor finished its turn.
/// - `"failed"`: the actor couldn't go on.
/// - `"interrupted"`: the session's device went away mid-turn.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum ActorSessionStop {
    NeedsYou,
    Finished,
    Failed,
    Interrupted,
}

/// One entry in an actor session's transcript.
#[derive(Clone, Debug, Serialize, Deserialize, Model, TsWire)]
#[model(table = "actor_session_messages", backup = Included)]
pub struct ActorSessionMessage {
    pub id: i64,
    pub session_id: i64,
    pub sent_at: i64,
    /// Who said it and what, stored as `sender`, `kind` and the columns they
    /// set.
    #[model(group)]
    pub content: ActorSessionMessageContent,
    /// Whether the user has answered it; only a request can be. A request
    /// is answered by its response, or passed over by anything the user
    /// says after it, which the actor takes as their reply instead.
    #[model(expr = ActorSessionMessage::ANSWERED_SQL)]
    pub answered: bool,
    /// How many images came with it, the user's or the actor's;
    /// `ListActorSessionMessageAttachments` fetches them.
    #[model(expr = ActorSessionMessage::ATTACHMENT_COUNT_SQL)]
    pub attachment_count: i64,
}

impl ActorSessionMessage {
    /// The SQL of [`answered`](Self::answered).
    /// The kinds it lists are [`ActorMessageContent::request`]'s.
    pub const ANSWERED_SQL: &'static str = "(EXISTS (SELECT 1 FROM actor_session_messages r \
        WHERE r.request_id = actor_session_messages.id) \
        OR (actor_session_messages.sender = 'actor' \
        AND actor_session_messages.kind IN ('question', 'permission', 'plan') \
        AND EXISTS (SELECT 1 FROM actor_session_messages r \
        WHERE r.session_id = actor_session_messages.session_id AND r.id > actor_session_messages.id \
        AND r.sender = 'user' AND r.kind = 'text')))";

    /// The SQL of [`attachment_count`](Self::attachment_count).
    pub const ATTACHMENT_COUNT_SQL: &'static str = "(SELECT COUNT(*) FROM attachments \
        WHERE attachments.actor_session_message_id = actor_session_messages.id)";
}

/// Who said an actor session message, tagged by `sender`, and what it says,
/// tagged by `kind`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire)]
#[serde(tag = "sender", rename_all = "camelCase")]
pub enum ActorSessionMessageContent {
    Actor(ActorMessageContent),
    User(ActorUserContent),
}

/// What an actor says in a session, tagged by `kind`.
///
/// - `"text"`: the agent's own words, as Markdown.
/// - `"thinking"`: its reasoning on the way, as Markdown: a summary or the
///   full chain, whichever its client has, and when it ended. Clients
///   show it quieter than its words.
/// - `"toolUse"`: a tool it ran, such as editing a file, and when it ended.
/// - `"question"`: questions for the user, each with options (Claude Code's
///   `AskUserQuestion`, Codex's `request_user_input`).
/// - `"permission"`: something it needs the user's leave to do: run a
///   command, change files, or use a tool.
/// - `"plan"`: a plan it asks the user to approve before it starts
///   (Claude Code's `ExitPlanMode`).
///
/// The last three are requests: the session waits for the user's response.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ActorMessageContent {
    Text {
        text: String,
    },
    Thinking {
        text: String,
        /// When it ended, in Unix seconds, as Cohesion heard of it from an
        /// `end` update; `null` while it goes on, and for thinking whose
        /// actor never said.
        ended_at: Option<i64>,
    },
    ToolUse {
        /// The tool, such as "Edit", as the actor names it.
        tool: String,
        /// What kind of tool it is, as the actor's client classifies it;
        /// `"other"` when left out.
        #[serde(default)]
        category: ActorToolKind,
        /// What it did, in a line, such as "src/main.rs".
        summary: String,
        /// When it ended, in Unix seconds, as Cohesion heard of it from an
        /// `end` update; `null` while it runs, and for one whose actor
        /// never said (a tool it was stopped in, or an actor that doesn't
        /// report ends).
        ended_at: Option<i64>,
    },
    Question {
        questions: Vec<ActorQuestion>,
    },
    Permission(ActorPermissionRequest),
    Plan {
        /// The plan, as Markdown.
        plan: String,
    },
}

impl ActorMessageContent {
    /// Whether the user can respond to it, and with what kind of response.
    pub fn request(&self) -> Option<ActorRequestKind> {
        match self {
            Self::Text { .. } | Self::Thinking { .. } | Self::ToolUse { .. } => None,
            Self::Question { .. } => Some(ActorRequestKind::Question),
            Self::Permission(_) => Some(ActorRequestKind::Permission),
            Self::Plan { .. } => Some(ActorRequestKind::Plan),
        }
    }

    /// When it ended, for a message that runs for a while (a tool, or
    /// thinking): `None` inside while it runs. `None` for any other kind.
    pub fn ended_at_mut(&mut self) -> Option<&mut Option<i64>> {
        match self {
            Self::ToolUse { ended_at, .. } | Self::Thinking { ended_at, .. } => Some(ended_at),
            Self::Text { .. } | Self::Question { .. } | Self::Permission(_) | Self::Plan { .. } => {
                None
            }
        }
    }
}

/// What kind of tool an actor ran, which its client decides from the
/// tool's name, since only the client knows its actor's tools.
///
/// - `"command"`: runs a shell command (Claude Code's `Bash`).
/// - `"read"`: reads a file or directory (`Read`).
/// - `"edit"`: changes files (`Edit`, `Write`).
/// - `"search"`: searches files or their contents (`Grep`, `Glob`).
/// - `"web"`: fetches or searches the web (`WebFetch`, `WebSearch`).
/// - `"agent"`: hands work to another agent (`Agent`).
/// - `"skill"`: takes up a skill, instructions it loads for the session
///   (`Skill`).
/// - `"other"`: anything else, and a tool its client didn't classify.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum ActorToolKind {
    Command,
    Read,
    Edit,
    Search,
    Web,
    Agent,
    Skill,
    #[default]
    Other,
}

/// The kinds of request an actor makes, each answered by its own kind of
/// response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActorRequestKind {
    Question,
    Permission,
    Plan,
}

/// One question an actor asks.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire)]
pub struct ActorQuestion {
    /// A short label for it, such as "Auth method".
    pub header: Option<String>,
    pub question: String,
    /// The answers to pick from; none for a question answered in words.
    pub options: Vec<ActorQuestionOption>,
    /// Whether more than one option may be picked.
    pub multi_select: bool,
    /// Whether the user may answer in their own words instead.
    pub allows_other: bool,
}

/// An answer to pick for an actor's question.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire)]
pub struct ActorQuestionOption {
    pub label: String,
    /// What picking it means.
    pub description: Option<String>,
}

/// Something an actor needs the user's leave to do, and the decisions it
/// takes.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire)]
pub struct ActorPermissionRequest {
    pub subject: ActorPermissionSubject,
    /// Why it wants to, in its words.
    pub reason: Option<String>,
    /// The decisions it takes, in the order to offer them.
    pub decisions: Vec<ActorPermissionDecision>,
}

/// What an actor asks leave to do, tagged by `kind`.
///
/// - `"command"`: run a shell command.
/// - `"fileChange"`: change files.
/// - `"tool"`: use any other tool, with its input as JSON.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ActorPermissionSubject {
    Command {
        command: String,
        /// The directory it runs in.
        cwd: Option<String>,
    },
    FileChange {
        changes: Vec<ActorFileChange>,
    },
    Tool {
        name: String,
        input: serde_json::Value,
    },
}

/// One file an actor asks to change.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire)]
pub struct ActorFileChange {
    pub path: String,
    pub change: ActorFileChangeKind,
    /// The change as a unified diff, when the actor has one.
    pub diff: Option<String>,
}

/// How a file changes.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TsWire)]
#[serde(rename_all = "camelCase")]
pub enum ActorFileChangeKind {
    Add,
    Update,
    Delete,
}

/// What the user decides about an actor's permission request.
///
/// - `"allow"`: this once.
/// - `"allowForSession"`: this and the like for the rest of the session.
/// - `"deny"`: not this; the actor carries on without it.
/// - `"abort"`: not this, and stop what it is doing.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum ActorPermissionDecision {
    Allow,
    AllowForSession,
    Deny,
    Abort,
}

/// What the user says in an actor session, tagged by `kind`.
///
/// - `"text"`: a message, the first the session's prompt.
/// - `"answers"`: the answers to a `question`, one per question in order.
/// - `"permission"`: the decision on a `permission` request.
/// - `"plan"`: whether a `plan` is approved.
///
/// Every kind but `text` responds to the request `request_id`, at most once.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts_wire(input)]
pub enum ActorUserContent {
    Text {
        text: String,
    },
    Answers {
        request_id: i64,
        answers: Vec<ActorAnswer>,
    },
    Permission {
        request_id: i64,
        decision: ActorPermissionDecision,
        /// What to tell the actor with it, such as why not.
        message: Option<String>,
    },
    Plan {
        request_id: i64,
        approved: bool,
        /// What to change, when not approved.
        feedback: Option<String>,
    },
}

impl ActorUserContent {
    /// The request it responds to, and the kind that request must be.
    pub fn responds_to(&self) -> Option<(i64, ActorRequestKind)> {
        match self {
            Self::Text { .. } => None,
            Self::Answers { request_id, .. } => Some((*request_id, ActorRequestKind::Question)),
            Self::Permission { request_id, .. } => {
                Some((*request_id, ActorRequestKind::Permission))
            }
            Self::Plan { request_id, .. } => Some((*request_id, ActorRequestKind::Plan)),
        }
    }
}

/// The answer to one question of an actor's.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire)]
#[ts_wire(input)]
pub struct ActorAnswer {
    /// The labels of the options picked.
    pub selected: Vec<String>,
    /// The answer in the user's own words, when the question allows it.
    pub other: Option<String>,
}
