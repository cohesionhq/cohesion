use cohesion_macros::{Model, TextEnum, TsWire};
use serde::{Deserialize, Serialize};

use crate::model::{ActorSessionStop, HookKind};

/// Who sent an agent message.
///
/// - `"user"`: the user.
/// - `"agent"`: the agent speaking to the user: in its conversation, or
///   from a fork of it that another agent's question woke (the message's
///   `run_id` names the fork's run).
/// - `"backgroundAgent"`: the agent in a run one of its cues woke, before
///   cues woke forks; only its notes landed here. Another
///   agent's question to it stands here too, opening the fork it woke.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum IntelligenceAgentMessageSender {
    User,
    Agent,
    BackgroundAgent,
}

/// What an agent message says, tagged by `kind`.
///
/// - `"text"`: plain text.
/// - `"codeExecution"`: a script the agent ran.
/// - `"error"`: why the sender's turn or run failed, or (from
///   `backgroundAgent`, with no `run_id`) why one of its cues' functions did.
/// - `"documentation"`: the script API module the sender loaded, such as
///   `network`.
/// - `"choice"`: a question with answers to pick from.
/// - `"memory"`: a memory the agent remembered or forgot.
/// - `"forward"`: the user's message forwarding an item, with their note.
/// - `"askedAgent"`: the agent's question to another agent, and its answer.
/// - `"askedByAgent"`: another agent's question to the agent, and the answer
///   it gave.
/// - `"note"`: what the agent noted for its own later turns, which nobody
///   is told of.
/// - `"woke"`: why the agent woke apart from its conversation: a cue fired,
///   or an actor session it follows stopped. What that work posts follows,
///   marked with its run.
/// - `"rebuiltContext"`: the agent's context was built again from the
///   database rather than carrying on its last thread, which loses the
///   provider's cached prompt, and `reason` says why
///   (`null` on a line written before reasons were kept).
#[derive(Serialize, Deserialize, Clone, Debug, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AgentMessageContent {
    Text {
        text: String,
    },
    CodeExecution(AgentCodeExecution),
    Error {
        error: String,
    },
    Documentation {
        module: String,
    },
    Choice(AgentChoice),
    Memory {
        memory: String,
        change: AgentMemoryChange,
    },
    Forward {
        /// What the user wrote with the item, or empty.
        note: String,
        item: AgentForwardedItem,
    },
    AskedAgent(AgentAsk),
    AskedByAgent(AgentAsk),
    Note {
        note: String,
    },
    Woke {
        by: AgentWake,
    },
    RebuiltContext {
        #[serde(default)]
        reason: Option<ContextRebuildReason>,
    },
}

/// Why an agent's turn started a fresh model thread instead of carrying on
/// its last one, in a `rebuiltContext` message.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum ContextRebuildReason {
    /// The provider dropped the cached prompt since the thread's last call,
    /// so a rebuilt thread costs no more than carrying the old one on.
    CacheExpired,
    /// The thread's prompt grew past the size a live thread is kept at.
    ContextLimit,
    /// The Memory capacity setting changed.
    MemoryCapacityChanged,
    /// The `agent` tier's default model changed.
    ModelChanged,
    /// The agent had no live thread: it hadn't spoken since the daemon
    /// started.
    NoThread,
    /// The last turn failed, which drops its thread.
    TurnFailed,
}

impl AgentMessageContent {
    /// Whether this is the agent's own bookkeeping (a note, a wake line, a
    /// compressed context),
    /// which raises no hook: it is news to no one but the agent's own later
    /// turns.
    pub fn is_bookkeeping(&self) -> bool {
        matches!(
            self,
            Self::Note { .. } | Self::Woke { .. } | Self::RebuiltContext { .. }
        )
    }
}

/// What woke an agent apart from its conversation, in a `woke` message.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AgentWake {
    /// One of its cues fired on its hook.
    Cue { cue_name: String, hook: HookKind },
    /// An actor session it follows stopped on its own.
    ActorSession {
        actor_session_id: i64,
        title: String,
        stop: ActorSessionStop,
    },
    /// Someone other than the agent assigned it a task: the user, a script,
    /// or another agent.
    AssignedTask {
        workspace_id: i64,
        task_id: i64,
        title: String,
    },
}

/// What became of a memory, in a `memory` message.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum AgentMemoryChange {
    Remembered,
    Forgotten,
}

impl AgentMessageContent {
    /// Whether it can be reacted to: a text message or a forward.
    pub fn is_reactable(&self) -> bool {
        matches!(self, Self::Text { .. } | Self::Forward { .. })
    }
}

/// What kind of item a `forward` message forwards.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum AgentForwardedItemKind {
    Task,
    Document,
    Journal,
}

impl AgentForwardedItemKind {
    /// What the item is called in a sentence: `journal entry`.
    pub fn noun(self) -> &'static str {
        match self {
            Self::Task => "task",
            Self::Document => "document",
            Self::Journal => "journal entry",
        }
    }
}

/// The item a `forward` message forwards, which the agent reads from its
/// `cohesion://` link with its code tool rather than being sent a copy.
#[derive(Serialize, Deserialize, Clone, Debug, TsWire)]
pub struct AgentForwardedItem {
    pub kind: AgentForwardedItemKind,
    pub workspace_id: i64,
    pub id: i64,
    /// The task's or document's title when it was forwarded; a journal
    /// entry's is its day, `YYYY-MM-DD`.
    pub title: String,
}

impl AgentForwardedItem {
    /// The sentence the agent reads the item from, after the user's note:
    /// `Forwarded task [Fix login](cohesion://task/3/42).`
    pub fn reference(&self) -> String {
        // A journal entry is a document, as its link names it.
        let link = match self.kind {
            AgentForwardedItemKind::Task => "task",
            AgentForwardedItemKind::Document | AgentForwardedItemKind::Journal => "document",
        };
        let noun = self.kind.noun();
        // Backslashes and brackets in a title would end the link's text early.
        let mut title = String::with_capacity(self.title.len());
        for c in self.title.chars() {
            if matches!(c, '\\' | '[' | ']') {
                title.push('\\');
            }
            title.push(c);
        }
        format!(
            "Forwarded {noun} [{title}](cohesion://{link}/{}/{}).",
            self.workspace_id, self.id
        )
    }
}

/// One agent's question to another, stored twice: as `askedAgent` in the
/// conversation of the agent that asked, and as `askedByAgent` in the
/// conversation of the agent that answered, each naming the other in
/// `agent`.
#[derive(Serialize, Deserialize, Clone, Debug, TsWire)]
pub struct AgentAsk {
    /// The other agent: the one asked in `askedAgent`, the one asking in
    /// `askedByAgent`.
    pub agent: AgentPeer,
    pub question: String,
    pub reply: AgentAskReply,
}

/// An agent named in another's conversation, as it was named when it was
/// asked or asked.
#[derive(Serialize, Deserialize, Clone, Debug, TsWire)]
pub struct AgentPeer {
    pub id: i64,
    pub name: String,
}

/// Where a question to another agent stands, tagged by `status`; both of
/// its messages always say the same.
///
/// - `"waiting"`: the agent asked is still answering.
/// - `"answered"`: it answered with `text`.
/// - `"unanswered"`: no answer came, for `reason`, such as the wait timing
///   out.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, TsWire)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum AgentAskReply {
    Waiting,
    Answered { text: String },
    Unanswered { reason: String },
}

/// What a message is to the fork that posted it, which says where a client
/// shows it: a fork's messages are told apart by `run_id`, and may fall among
/// the conversation's own or another fork's.
///
/// - `"trigger"`: what woke the fork, its `woke` line or another agent's
///   question to it. A client collapses the fork's steps under it.
/// - `"step"`: work the fork did on the way: a script, a module it loaded, a
///   memory, a note, a question to another agent. It belongs under the
///   trigger, not in the main transcript.
/// - `"surfaced"`: what the fork has to show the user: a text, a question to
///   pick an answer to, or why it failed. It stays in the main transcript.
///
/// An older fork may have no trigger, from before wake lines.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum AgentRunPart {
    Trigger,
    Step,
    Surfaced,
}

/// One entry in an agent's conversation.
#[derive(Clone, Debug, Serialize, Deserialize, Model, TsWire)]
#[model(table = "intelligence_agent_messages", backup = Included)]
pub struct IntelligenceAgentMessage {
    pub id: i64,
    pub agent_id: i64,
    /// The background run that sent it, if any.
    pub run_id: Option<i64>,
    /// What woke the background run that sent this, e.g. `"Test task" was
    /// created`; `null` for foreground messages.
    pub run_description: Option<String>,
    /// What the message is to the fork `run_id` names; `null` for a message
    /// of the conversation itself.
    #[model(expr = IntelligenceAgentMessage::RUN_PART_SQL)]
    pub run_part: Option<AgentRunPart>,
    pub sent_at: i64,
    pub sender: IntelligenceAgentMessageSender,
    /// What it says, stored as `kind` and the columns each kind sets.
    #[model(group)]
    pub message_content: AgentMessageContent,
    /// An emoji reacting to the message, from the side that didn't send it:
    /// the user on the agent's messages, the agent on the user's.
    pub reaction: Option<String>,
    /// When `reaction` was last set or cleared.
    pub reacted_at: Option<i64>,
    /// How many images came with it, the user's or the agent's, shown with
    /// its text; `ListAgentMessageAttachments` fetches them.
    #[model(expr = IntelligenceAgentMessage::ATTACHMENT_COUNT_SQL)]
    pub attachment_count: i64,
    /// The message of the same conversation this one replies to, which a
    /// client quotes above it; `null` when it stands alone.
    pub reply_to: Option<i64>,
    /// The message in one line: what a reply quoting it shows, and how a
    /// client shows a kind it has no view for. Its text, a forward's note (else the item's title), a choice's
    /// question, a script's title, `Learning about network`, `Asked Scout:
    /// …`, `Scout asked: …`, or an error; for the agent's bookkeeping, what
    /// follows `excerpt_verb`: the memory, the note, or what woke it
    /// (`Actor session 264 stopped`, `Cue “Morning” fired`, `“Fix
    /// the build” was assigned to you`). A compressed context's is
    /// `Context rebuilt`, then why when it is known, with no verb.
    #[model(expr = IntelligenceAgentMessage::EXCERPT_SQL)]
    pub excerpt: String,
    /// What the agent did, for a line of its bookkeeping: `Remembered`,
    /// `Forgot`, `Noted` or `Woke`, which reads alone as the heading of a
    /// run of such lines and before `excerpt` as `Noted: …`; `null` for
    /// every other kind.
    #[model(expr = IntelligenceAgentMessage::EXCERPT_VERB_SQL)]
    pub excerpt_verb: Option<String>,
    /// What follows `reaction_excerpt_verb` in a line about `reaction`: the
    /// emoji and the message it reacts to, quoted and cut to 140 characters
    /// (`👍 to “Done.”`); `null` with no reaction.
    #[model(expr = IntelligenceAgentMessage::REACTION_EXCERPT_SQL)]
    pub reaction_excerpt: Option<String>,
    /// `Reacted` when the message has a `reaction`, the heading of a run of
    /// lines about reactions and the start of one; `null` otherwise.
    #[model(expr = IntelligenceAgentMessage::REACTION_EXCERPT_VERB_SQL)]
    pub reaction_excerpt_verb: Option<String>,
}

/// The SQL of [`IntelligenceAgentMessage::excerpt`], as a literal so the
/// line about a reaction can quote it with `concat!`.
macro_rules! excerpt_sql {
    () => {
        "(SELECT CASE \
        WHEN line = '' AND images = 1 THEN '1 image' \
        WHEN line = '' AND images > 1 THEN images || ' images' \
        WHEN length(line) > 200 THEN substr(line, 1, 200) || '…' \
        ELSE line END \
        FROM (SELECT (SELECT COUNT(*) FROM attachments \
                WHERE attachments.agent_message_id = intelligence_agent_messages.id) AS images, \
            trim(replace(replace(replace(replace(replace(replace(replace( \
            CASE intelligence_agent_messages.kind \
            WHEN 'text' THEN intelligence_agent_messages.text \
            WHEN 'forward' THEN coalesce(nullif(trim(intelligence_agent_messages.text), ''), \
                intelligence_agent_messages.title) \
            WHEN 'choice' THEN intelligence_agent_messages.text \
            WHEN 'codeExecution' THEN intelligence_agent_messages.title \
            WHEN 'documentation' THEN 'Learning about ' || intelligence_agent_messages.text \
            WHEN 'memory' THEN intelligence_agent_messages.text \
            WHEN 'askedAgent' THEN 'Asked ' || intelligence_agent_messages.peer_name \
                || ': ' || intelligence_agent_messages.text \
            WHEN 'askedByAgent' THEN intelligence_agent_messages.peer_name \
                || ' asked: ' || intelligence_agent_messages.text \
            WHEN 'error' THEN intelligence_agent_messages.text \
            WHEN 'note' THEN intelligence_agent_messages.text \
            WHEN 'woke' THEN CASE intelligence_agent_messages.wake \
                WHEN 'cue' THEN 'Cue “' || intelligence_agent_messages.cue_name || '” fired' \
                WHEN 'assignedTask' THEN '“' || intelligence_agent_messages.title || '” was assigned to you' \
                ELSE 'Actor session ' || intelligence_agent_messages.actor_session_id || ' stopped' END \
            WHEN 'rebuiltContext' THEN 'Context rebuilt' || CASE intelligence_agent_messages.reason \
                WHEN 'cacheExpired' THEN ' because provider cache expired' \
                WHEN 'contextLimit' THEN ' because context reached its size limit' \
                WHEN 'memoryCapacityChanged' THEN ' because memory capacity changed' \
                WHEN 'modelChanged' THEN ' because the default model changed' \
                WHEN 'noThread' THEN ' because there was no live thread' \
                WHEN 'turnFailed' THEN ' because the last turn failed' \
                ELSE '' END \
            ELSE '' END, \
            '**', ''), '__', ''), '`', ''), char(13), ' '), char(10), ' '), char(9), ' '), \
            '  ', ' ')) AS line))"
    };
}

impl IntelligenceAgentMessage {
    /// The SQL of [`attachment_count`](Self::attachment_count).
    pub const ATTACHMENT_COUNT_SQL: &'static str = "(SELECT COUNT(*) FROM attachments \
        WHERE attachments.agent_message_id = intelligence_agent_messages.id)";

    /// The SQL of [`run_part`](Self::run_part). A kind it doesn't name stays
    /// in the main transcript, so nothing new goes out of sight by default.
    pub const RUN_PART_SQL: &'static str = "(CASE \
        WHEN intelligence_agent_messages.run_id IS NULL THEN NULL \
        WHEN intelligence_agent_messages.kind IN ('woke', 'askedByAgent') THEN 'trigger' \
        WHEN intelligence_agent_messages.kind IN ('codeExecution', 'documentation', 'memory', \
            'note', 'askedAgent') THEN 'step' \
        ELSE 'surfaced' END)";

    /// The SQL of [`excerpt`](Self::excerpt): the line for each kind with
    /// Markdown's emphasis and code marks taken out, on one line and cut
    /// to 200 characters; an empty text with images says how many.
    pub const EXCERPT_SQL: &'static str = excerpt_sql!();

    /// The SQL of [`reaction_excerpt`](Self::reaction_excerpt).
    pub const REACTION_EXCERPT_SQL: &'static str = concat!(
        "(SELECT CASE WHEN intelligence_agent_messages.reaction IS NULL THEN NULL \
        ELSE intelligence_agent_messages.reaction || ' to “' || CASE WHEN length(excerpt) > 140 \
            THEN substr(excerpt, 1, 140) || '…' ELSE excerpt END || '”' END \
        FROM (SELECT ",
        excerpt_sql!(),
        " AS excerpt))"
    );

    /// The SQL of [`reaction_excerpt_verb`](Self::reaction_excerpt_verb).
    pub const REACTION_EXCERPT_VERB_SQL: &'static str =
        "(CASE WHEN intelligence_agent_messages.reaction IS NOT NULL THEN 'Reacted' END)";

    /// The SQL of [`excerpt_verb`](Self::excerpt_verb).
    pub const EXCERPT_VERB_SQL: &'static str = "(CASE intelligence_agent_messages.kind \
        WHEN 'memory' THEN CASE intelligence_agent_messages.change \
            WHEN 'forgotten' THEN 'Forgot' ELSE 'Remembered' END \
        WHEN 'note' THEN 'Noted' \
        WHEN 'woke' THEN 'Woke' END)";
}

/// The emoji clients offer to react to a message with, in this order. Any
/// single emoji is accepted.
pub const AGENT_REACTIONS: [&str; 8] = ["👍", "👎", "❤️", "😂", "😮", "😢", "🎉", "🙏"];

/// A script the agent ran, stored as a `codeExecution` message.
#[derive(Serialize, Deserialize, Clone, Debug, TsWire)]
pub struct AgentCodeExecution {
    /// What the script does, as the agent described it.
    pub title: String,
    pub code: String,
    /// How the script ended.
    pub outcome: AgentCodeOutcome,
}

/// How a script the agent ran ended, tagged by `status`.
///
/// - `"succeeded"`: it ran to the end, returning `result`.
/// - `"failed"`: it threw or ran out of time, for `error`.
/// - `"cancelled"`: the user stopped the agent's turn while it ran.
#[derive(Serialize, Deserialize, Clone, Debug, TsWire)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum AgentCodeOutcome {
    Succeeded { result: String },
    Failed { error: String },
    Cancelled,
}

/// A question the agent asks with answers to pick from, lettered `A`, `B`,
/// `C` and so on, stored as a `choice` message. Picking one records it here
/// and starts the agent's turn on it; the pick posts no message of its own.
#[derive(Serialize, Deserialize, Clone, Debug, TsWire)]
pub struct AgentChoice {
    /// The question, in Markdown.
    pub question: String,
    /// The answers on offer, in letter order.
    pub options: Vec<String>,
    /// From a background run: what the agent's conversation needs to act on
    /// the pick, such as what the run found and what each answer leads to.
    /// The agent reads it with the pick; the user doesn't see it. `null`
    /// from the conversation itself.
    pub context: Option<String>,
    /// The index in `options` of the user's pick; `null` until they pick.
    pub selected: Option<usize>,
}

impl AgentChoice {
    /// Option `index` after its letter: `B. Tomorrow`.
    pub fn lettered(&self, index: usize) -> String {
        let letter = char::from(b'A' + index as u8);
        format!("{letter}. {}", self.options[index])
    }
}
