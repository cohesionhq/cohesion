//! How an agent message's content is stored: `kind` and the columns that kind
//! sets, rather than JSON. The wire keeps [`AgentMessageContent`]'s tagged
//! shape; the table's checks hold each kind to its columns. `text`, `title` and
//! the item columns are shared where kinds say the same sort of thing.

use cohesion_macros::TextEnum;
use serde::{Deserialize, Serialize};

use super::{
    ActorSessionStop, AgentAgendaChange, AgentAgendaUpdate, AgentAsk, AgentAskReply, AgentChoice,
    AgentCodeExecution, AgentCodeOutcome, AgentForwardedItem, AgentForwardedItemKind,
    AgentMemoryChange, AgentMessageContent, AgentPeer, AgentWake, ContextRebuildReason, HookKind,
};
use crate::stored_model::ColumnGroup;
use crate::value::{IntoValue, Json, Row, RowError, Value, read_column};

/// Which kind of content an agent message holds.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum AgentMessageKind {
    Text,
    CodeExecution,
    Error,
    Documentation,
    Choice,
    Memory,
    Forward,
    AskedAgent,
    AskedByAgent,
    Note,
    Woke,
    RebuiltContext,
    Agenda,
}

/// How a script's `outcome` column reads.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum AgentCodeStatus {
    Succeeded,
    Failed,
    Cancelled,
}

/// How a question's `reply` column reads.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum AgentReplyStatus {
    Waiting,
    Answered,
    Unanswered,
}

/// What the `wake` column says woke the agent.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum AgentWakeKind {
    Cue,
    ActorSession,
    AssignedTask,
}

/// The columns one message sets; the rest stay `NULL`.
struct Columns {
    kind: AgentMessageKind,
    text: Option<String>,
    title: Option<String>,
    code: Option<String>,
    outcome: Option<AgentCodeStatus>,
    outcome_text: Option<String>,
    options: Option<Vec<String>>,
    context: Option<String>,
    selected: Option<i64>,
    change: Option<AgentMemoryChange>,
    item_kind: Option<AgentForwardedItemKind>,
    item_workspace_id: Option<i64>,
    item_id: Option<i64>,
    peer_id: Option<i64>,
    peer_name: Option<String>,
    reply: Option<AgentReplyStatus>,
    reply_text: Option<String>,
    wake: Option<AgentWakeKind>,
    cue_name: Option<String>,
    hook: Option<HookKind>,
    actor_session_id: Option<i64>,
    stop: Option<ActorSessionStop>,
    reason: Option<ContextRebuildReason>,
    agenda_change: Option<AgentAgendaChange>,
    agenda: Option<Vec<String>>,
}

impl Columns {
    fn of(kind: AgentMessageKind) -> Self {
        Self {
            kind,
            text: None,
            title: None,
            code: None,
            outcome: None,
            outcome_text: None,
            options: None,
            context: None,
            selected: None,
            change: None,
            item_kind: None,
            item_workspace_id: None,
            item_id: None,
            peer_id: None,
            peer_name: None,
            reply: None,
            reply_text: None,
            wake: None,
            cue_name: None,
            hook: None,
            actor_session_id: None,
            stop: None,
            reason: None,
            agenda_change: None,
            agenda: None,
        }
    }

    fn text(kind: AgentMessageKind, text: String) -> Self {
        Self {
            text: Some(text),
            ..Self::of(kind)
        }
    }

    /// These columns holding the items an `agenda` message names.
    fn listing(self, agenda: Vec<String>) -> Self {
        Self {
            agenda: Some(agenda),
            ..self
        }
    }

    fn ask(kind: AgentMessageKind, ask: AgentAsk) -> Self {
        let (reply, reply_text) = match ask.reply {
            AgentAskReply::Waiting => (AgentReplyStatus::Waiting, None),
            AgentAskReply::Answered { text } => (AgentReplyStatus::Answered, Some(text)),
            AgentAskReply::Unanswered { reason } => (AgentReplyStatus::Unanswered, Some(reason)),
        };
        Self {
            text: Some(ask.question),
            peer_id: Some(ask.agent.id),
            peer_name: Some(ask.agent.name),
            reply: Some(reply),
            reply_text,
            ..Self::of(kind)
        }
    }
}

fn read_ask(row: &dyn Row) -> Result<AgentAsk, RowError> {
    Ok(AgentAsk {
        agent: AgentPeer {
            id: read_column(row, "peer_id")?,
            name: read_column(row, "peer_name")?,
        },
        question: read_column(row, "text")?,
        reply: match read_column::<AgentReplyStatus>(row, "reply")? {
            AgentReplyStatus::Waiting => AgentAskReply::Waiting,
            AgentReplyStatus::Answered => AgentAskReply::Answered {
                text: read_column(row, "reply_text")?,
            },
            AgentReplyStatus::Unanswered => AgentAskReply::Unanswered {
                reason: read_column(row, "reply_text")?,
            },
        },
    })
}

impl ColumnGroup for AgentMessageContent {
    const COLUMNS: &'static [&'static str] = &[
        "kind",
        "text",
        "title",
        "code",
        "outcome",
        "outcome_text",
        "options",
        "context",
        "selected",
        "change",
        "item_kind",
        "item_workspace_id",
        "item_id",
        "peer_id",
        "peer_name",
        "reply",
        "reply_text",
        "wake",
        "cue_name",
        "hook",
        "actor_session_id",
        "stop",
        "reason",
        "agenda_change",
        "agenda",
    ];

    fn from_row(row: &dyn Row) -> Result<Self, RowError> {
        use AgentMessageKind as Kind;
        Ok(match read_column::<Kind>(row, "kind")? {
            Kind::Text => Self::Text {
                text: read_column(row, "text")?,
            },
            Kind::CodeExecution => Self::CodeExecution(AgentCodeExecution {
                title: read_column(row, "title")?,
                code: read_column(row, "code")?,
                outcome: match read_column::<AgentCodeStatus>(row, "outcome")? {
                    AgentCodeStatus::Succeeded => AgentCodeOutcome::Succeeded {
                        result: read_column(row, "outcome_text")?,
                    },
                    AgentCodeStatus::Failed => AgentCodeOutcome::Failed {
                        error: read_column(row, "outcome_text")?,
                    },
                    AgentCodeStatus::Cancelled => AgentCodeOutcome::Cancelled,
                },
            }),
            Kind::Error => Self::Error {
                error: read_column(row, "text")?,
            },
            Kind::Documentation => Self::Documentation {
                module: read_column(row, "text")?,
            },
            Kind::Choice => Self::Choice(AgentChoice {
                question: read_column(row, "text")?,
                options: read_column::<Json<Vec<String>>>(row, "options")?.0,
                context: read_column(row, "context")?,
                selected: read_column::<Option<i64>>(row, "selected")?
                    .map(|selected| selected as usize),
            }),
            Kind::Memory => Self::Memory {
                memory: read_column(row, "text")?,
                change: read_column(row, "change")?,
            },
            Kind::Forward => Self::Forward {
                note: read_column(row, "text")?,
                item: AgentForwardedItem {
                    kind: read_column(row, "item_kind")?,
                    workspace_id: read_column(row, "item_workspace_id")?,
                    id: read_column(row, "item_id")?,
                    title: read_column(row, "title")?,
                },
            },
            Kind::AskedAgent => Self::AskedAgent(read_ask(row)?),
            Kind::AskedByAgent => Self::AskedByAgent(read_ask(row)?),
            Kind::Note => Self::Note {
                note: read_column(row, "text")?,
            },
            Kind::Woke => Self::Woke {
                by: match read_column::<AgentWakeKind>(row, "wake")? {
                    AgentWakeKind::Cue => AgentWake::Cue {
                        cue_name: read_column(row, "cue_name")?,
                        hook: read_column(row, "hook")?,
                    },
                    AgentWakeKind::ActorSession => AgentWake::ActorSession {
                        actor_session_id: read_column(row, "actor_session_id")?,
                        title: read_column(row, "title")?,
                        stop: read_column(row, "stop")?,
                    },
                    AgentWakeKind::AssignedTask => AgentWake::AssignedTask {
                        workspace_id: read_column(row, "item_workspace_id")?,
                        task_id: read_column(row, "item_id")?,
                        title: read_column(row, "title")?,
                    },
                },
            },
            Kind::RebuiltContext => Self::RebuiltContext {
                reason: read_column(row, "reason")?,
            },
            Kind::Agenda => Self::Agenda(AgentAgendaUpdate {
                change: read_column(row, "agenda_change")?,
                items: read_column::<Json<Vec<String>>>(row, "agenda")?.0,
            }),
        })
    }

    fn into_values(self) -> Vec<Value> {
        use AgentMessageKind as Kind;
        let columns = match self {
            Self::Text { text } => Columns::text(Kind::Text, text),
            Self::CodeExecution(execution) => {
                let (outcome, outcome_text) = match execution.outcome {
                    AgentCodeOutcome::Succeeded { result } => {
                        (AgentCodeStatus::Succeeded, Some(result))
                    }
                    AgentCodeOutcome::Failed { error } => (AgentCodeStatus::Failed, Some(error)),
                    AgentCodeOutcome::Cancelled => (AgentCodeStatus::Cancelled, None),
                };
                Columns {
                    title: Some(execution.title),
                    code: Some(execution.code),
                    outcome: Some(outcome),
                    outcome_text,
                    ..Columns::of(Kind::CodeExecution)
                }
            }
            Self::Error { error } => Columns::text(Kind::Error, error),
            Self::Documentation { module } => Columns::text(Kind::Documentation, module),
            Self::Choice(choice) => Columns {
                text: Some(choice.question),
                options: Some(choice.options),
                context: choice.context,
                selected: choice.selected.map(|selected| selected as i64),
                ..Columns::of(Kind::Choice)
            },
            Self::Memory { memory, change } => Columns {
                change: Some(change),
                ..Columns::text(Kind::Memory, memory)
            },
            Self::Forward { note, item } => Columns {
                title: Some(item.title),
                item_kind: Some(item.kind),
                item_workspace_id: Some(item.workspace_id),
                item_id: Some(item.id),
                ..Columns::text(Kind::Forward, note)
            },
            Self::AskedAgent(ask) => Columns::ask(Kind::AskedAgent, ask),
            Self::AskedByAgent(ask) => Columns::ask(Kind::AskedByAgent, ask),
            Self::Note { note } => Columns::text(Kind::Note, note),
            Self::Woke { by } => match by {
                AgentWake::Cue { cue_name, hook } => Columns {
                    wake: Some(AgentWakeKind::Cue),
                    cue_name: Some(cue_name),
                    hook: Some(hook),
                    ..Columns::of(Kind::Woke)
                },
                AgentWake::ActorSession {
                    actor_session_id,
                    title,
                    stop,
                } => Columns {
                    wake: Some(AgentWakeKind::ActorSession),
                    actor_session_id: Some(actor_session_id),
                    title: Some(title),
                    stop: Some(stop),
                    ..Columns::of(Kind::Woke)
                },
                AgentWake::AssignedTask {
                    workspace_id,
                    task_id,
                    title,
                } => Columns {
                    wake: Some(AgentWakeKind::AssignedTask),
                    item_workspace_id: Some(workspace_id),
                    item_id: Some(task_id),
                    title: Some(title),
                    ..Columns::of(Kind::Woke)
                },
            },
            Self::RebuiltContext { reason } => Columns {
                reason,
                ..Columns::of(Kind::RebuiltContext)
            },
            Self::Agenda(update) => Columns {
                agenda_change: Some(update.change),
                ..Columns::text(Kind::Agenda, update.changed_text())
            }
            .listing(update.items),
        };
        vec![
            columns.kind.into_value(),
            columns.text.into_value(),
            columns.title.into_value(),
            columns.code.into_value(),
            columns.outcome.into_value(),
            columns.outcome_text.into_value(),
            columns.options.map(Json).into_value(),
            columns.context.into_value(),
            columns.selected.into_value(),
            columns.change.into_value(),
            columns.item_kind.into_value(),
            columns.item_workspace_id.into_value(),
            columns.item_id.into_value(),
            columns.peer_id.into_value(),
            columns.peer_name.into_value(),
            columns.reply.into_value(),
            columns.reply_text.into_value(),
            columns.wake.into_value(),
            columns.cue_name.into_value(),
            columns.hook.into_value(),
            columns.actor_session_id.into_value(),
            columns.stop.into_value(),
            columns.reason.into_value(),
            columns.agenda_change.into_value(),
            columns.agenda.map(Json).into_value(),
        ]
    }
}
