//! An agent's agenda: the steps it means to take, in order, which every
//! thread of its conversation reads and changes with three tools,
//! `agenda_append`, `agenda_finish` and `agenda_reset`. Each thread opens
//! with the agenda in its status, and a thread that runs on while another
//! changes it hears the change under `# Agenda changed` before its next
//! model call, or at its next turn.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use cohesion_core::model::{AGENDA_LIMIT, AgentAgendaItem};
use cohesion_db::{
    AgendaEdit, AppendAgentAgendaCommand, CommandError, DatabaseContext,
    FinishAgentAgendaItemCommand, ListAgentAgendaCommand, ResetAgentAgendaCommand,
};
use serde::Deserialize;

use super::agent_tool::{AgentTool, Seat};
use crate::tool_definition::{Field, FieldKind, Tool};
use crate::tools::ToolFuture;
use crate::transcript_item::ToolCallStatus;

/// Every agent's agenda as the daemon last read or changed it, shared by
/// the threads of each conversation. A cheap clone.
#[derive(Clone, Default)]
pub(super) struct Agendas(Arc<Mutex<HashMap<i64, Arc<Agenda>>>>);

impl Agendas {
    /// Agent `agent_id`'s agenda.
    pub fn of(&self, agent_id: i64) -> Arc<Agenda> {
        self.0
            .lock()
            .expect("the agendas lock is not poisoned")
            .entry(agent_id)
            .or_default()
            .clone()
    }
}

/// One agent's agenda as the daemon last read or changed it; `None` until
/// a thread opens or the agenda changes, when no thread can have missed a
/// change.
#[derive(Default)]
pub(super) struct Agenda(Mutex<Option<Vec<Entry>>>);

/// An agenda item as a thread hears it.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct Entry {
    text: String,
    done: bool,
}

impl Agenda {
    fn set(&self, items: &[AgentAgendaItem]) -> Vec<Entry> {
        let entries: Vec<Entry> = items
            .iter()
            .map(|item| Entry {
                text: item.text.clone(),
                done: item.finished_at.is_some(),
            })
            .collect();
        *self.0.lock().expect("the agenda lock is not poisoned") = Some(entries.clone());
        entries
    }

    fn current(&self) -> Option<Vec<Entry>> {
        self.0
            .lock()
            .expect("the agenda lock is not poisoned")
            .clone()
    }
}

/// One thread's hearing of its agent's agenda: the agenda, and the items
/// the thread last heard, which a change by another thread differs from.
#[derive(Default)]
pub(super) struct HeardAgenda {
    agenda: Arc<Agenda>,
    heard: Mutex<Option<Vec<Entry>>>,
}

impl HeardAgenda {
    pub fn new(agenda: Arc<Agenda>) -> Self {
        Self {
            agenda,
            heard: Mutex::default(),
        }
    }

    /// Hears `items`, the agenda as it is now, read for the thread's status
    /// or changed by its own tool, and returns it as the thread reads it.
    pub fn hear(&self, items: &[AgentAgendaItem]) -> String {
        let entries = self.agenda.set(items);
        let text = listed(&entries);
        *self.heard() = Some(entries);
        text
    }

    /// Publishes `items`, the agenda as it is now, changed outside this
    /// thread, so the thread hears the change at its next catch-up.
    pub fn publish(&self, items: &[AgentAgendaItem]) {
        self.agenda.set(items);
    }

    /// `# Agenda changed` and the agenda as it is now, when another thread
    /// changed it since this one last heard; `None` when none did.
    pub fn news(&self) -> Option<String> {
        let current = self.agenda.current()?;
        let mut heard = self.heard();
        if heard.as_ref() == Some(&current) {
            return None;
        }
        // A thread that never heard it (none does for long: each opens with
        // it) has nothing to correct while it is empty.
        let unheard = heard.is_none();
        *heard = Some(current.clone());
        if unheard && current.is_empty() {
            return None;
        }
        if current.is_empty() {
            return Some("# Agenda changed\nAgenda was cleared.".to_string());
        }
        Some(format!(
            "# Agenda changed\nAnother thread of yours changed your agenda. {}",
            listed(&current)
        ))
    }

    fn heard(&self) -> std::sync::MutexGuard<'_, Option<Vec<Entry>>> {
        self.heard
            .lock()
            .expect("the heard agenda's lock is not poisoned")
    }
}

/// Agent `agent_id`'s agenda, in order.
pub(super) async fn load(
    database: &DatabaseContext,
    agent_id: i64,
) -> Result<Vec<AgentAgendaItem>, CommandError> {
    database.send(ListAgentAgendaCommand { agent_id }).await
}

/// The agenda as a thread reads it: `Your agenda is empty.`, or `Your
/// agenda:` and each item on a line after its position, `[x]` once
/// finished, `[ ]` while open.
fn listed(entries: &[Entry]) -> String {
    if entries.is_empty() {
        return "Your agenda is empty.".to_string();
    }
    let lines: Vec<String> = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let mark = if entry.done { "x" } else { " " };
            format!("{}. [{mark}] {}", index + 1, entry.text)
        })
        .collect();
    format!("Your agenda:\n{}", lines.join("\n"))
}

#[derive(Deserialize)]
struct AppendCall {
    items: Vec<String>,
    pos: Option<i64>,
}

#[derive(Deserialize)]
struct FinishCall {
    pos: Option<i64>,
}

/// Which of the agenda tools a [`AgendaTool`] is.
pub(super) enum AgendaToolKind {
    Append,
    Finish,
    Reset,
}

/// One of the agenda tools, the same in every thread, whoever started its
/// chain: the agenda is the agent's own, as its notes are.
pub(super) struct AgendaTool {
    pub database: DatabaseContext,
    pub agent_id: i64,
    pub kind: AgendaToolKind,
}

impl AgentTool for AgendaTool {
    fn definition(&self) -> Tool {
        match self.kind {
            AgendaToolKind::Append => Tool {
                name: "agenda_append",
                description: format!(
                    "Adds steps to your agenda, in order: what you mean to do. Call it unasked as \
                     your first call, before any script or message, whenever the work will span \
                     several turns or sessions, will outlast this conversation, or needs a goal \
                     kept across waits (a dispatched actor session, a cue, the user's feedback, \
                     network calls and the like); the agenda keeps that goal across them. Short \
                     work done in one go needs none. The agenda is a record, not a queue: it \
                     runs nothing and queues no work for you, so you still do each step. It lets \
                     the user see what you are working toward and how far you have got, and \
                     keeps your other threads in step. It holds at most {AGENDA_LIMIT} items, \
                     finished ones included; returns it as it now stands."
                )
                .into(),
                fields: vec![
                    Field {
                        name: "items",
                        // At most `AGENDA_ITEM_CHARS`, which a test holds it to.
                        description: "The steps, in order, each one short line (at most 200 \
                            characters) saying what you'll do: \"Draft the outline\".",
                        kind: FieldKind::StringList,
                        required: true,
                    },
                    Field {
                        name: "pos",
                        description: "Where the first step goes, from 1; the items there move \
                            down. Leave it out to add the steps at the end.",
                        kind: FieldKind::Integer,
                        required: false,
                    },
                ],
            },
            AgendaToolKind::Finish => Tool {
                name: "agenda_finish",
                description: "Marks an item of your agenda finished, so the user sees \
                    your progress. Call it in the turn you finish each step, before starting \
                    the next, not all at the end; when you wake with open items, carry on from \
                    the first. The item stays, ticked, until you reset the agenda. Returns the \
                    agenda as it now stands."
                    .into(),
                fields: vec![Field {
                    name: "pos",
                    description: "The item's position, from 1. Leave it out to finish the \
                        first item not yet finished.",
                    kind: FieldKind::Integer,
                    required: false,
                }],
            },
            AgendaToolKind::Reset => Tool {
                name: "agenda_reset",
                description: "Clears your agenda, finished and open items alike. Call it \
                    unasked as soon as all the work is done or dropped, before your turn ends, \
                    so it no longer shows as in progress; or to replace the steps with new \
                    ones, which you then add with `agenda_append`."
                    .into(),
                fields: Vec::new(),
            },
        }
    }

    fn call<'a>(&'a self, seat: &'a Seat, arguments: &'a str) -> ToolFuture<'a> {
        Box::pin(async move {
            let run_id = seat.fork_run_id();
            let agent_id = self.agent_id;
            let edited = match self.kind {
                AgendaToolKind::Append => {
                    let call: AppendCall = match serde_json::from_str(arguments) {
                        Ok(call) => call,
                        Err(e) => return ToolCallStatus::Failure(e.to_string()),
                    };
                    self.database
                        .send_traced(
                            AppendAgentAgendaCommand {
                                agent_id,
                                run_id,
                                items: call.items,
                                position: call.pos,
                            },
                            seat.turn.trace(),
                        )
                        .await
                }
                AgendaToolKind::Finish => {
                    let call: FinishCall = match serde_json::from_str(arguments) {
                        Ok(call) => call,
                        Err(e) => return ToolCallStatus::Failure(e.to_string()),
                    };
                    self.database
                        .send_traced(
                            FinishAgentAgendaItemCommand {
                                agent_id,
                                run_id,
                                position: call.pos,
                            },
                            seat.turn.trace(),
                        )
                        .await
                }
                AgendaToolKind::Reset => {
                    self.database
                        .send_traced(
                            ResetAgentAgendaCommand { agent_id, run_id },
                            seat.turn.trace(),
                        )
                        .await
                }
            };
            match edited {
                Ok(AgendaEdit { agenda, .. }) => {
                    ToolCallStatus::Success(seat.turn.agenda().hear(&agenda))
                }
                Err(CommandError::Validation(e)) => ToolCallStatus::Failure(e.message),
                Err(e) => ToolCallStatus::Failure(format!("could not change the agenda: {e:?}")),
            }
        })
    }
}

#[cfg(test)]
mod tests;
