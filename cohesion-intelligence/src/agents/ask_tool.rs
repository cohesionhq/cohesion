use cohesion_core::model::AgentAskReply;
use serde::Deserialize;

use super::agent_messenger::{ASK_LIMIT, AgentMessenger, Ask};
use super::agent_tool::{AgentTool, Seat};
use crate::activity::Activity;
use crate::tool_definition::{Field, FieldKind, Tool};
use crate::tools::{ToolFuture, asks_done};
use crate::transcript_item::ToolCallStatus;

#[derive(Deserialize)]
struct AskCall {
    agent_id: i64,
    question: String,
    /// Whether the call waits for a reply; it does unless told not to.
    /// `wait`, its old name, still reads, since memories and threads that
    /// predate the rename call it so.
    #[serde(default = "replies_by_default", alias = "wait")]
    reply: bool,
}

fn replies_by_default() -> bool {
    true
}

/// Lets an agent's conversation, and a fork of it, ask another of the
/// user's agents a question, which it answers in a fork of its own
/// conversation. By default the call waits for the fork's
/// `reply_to_agent` and returns it; with `reply` false it hands the agent
/// work one way and returns at once, and nothing comes back. Nothing ever
/// wakes the asker later. Only the conversation's sessions have it: a fork
/// can't ask, since the fork it would wake holds the conversation's
/// rights.
pub(super) struct AskAgentTool {
    pub messenger: AgentMessenger,
    pub agent_id: i64,
}

impl AgentTool for AskAgentTool {
    fn definition(&self) -> Tool {
        let minutes = ASK_LIMIT.as_secs() / 60;
        Tool {
            name: "ask_agent",
            description: format!(
                "Asks another of the user's agents a question, or hands it work. By default \
                 the call waits, up to {minutes} minutes, for its reply and returns it; past \
                 that, or with no reply, it returns \"No answer\" and nothing comes later. With \
                 `reply` false the work is handed over one way: the call returns at once, the \
                 agent does the work and replies to no one, and nothing ever wakes you about \
                 it. To hear back from one-way work, say so in the question (\"ask me when it \
                 is done\"), and the agent asks you in turn. It answers in a fork of its own \
                 conversation, knowing what it and the user said there but nothing of yours, \
                 so put everything it needs in the question. Its reply comes back to you, not \
                 the user, who sees only what you send them."
            )
            .into(),
            fields: vec![
                Field {
                    name: "agent_id",
                    description: "The id of the agent to ask, from `# Other agents`.",
                    kind: FieldKind::Integer,
                    required: true,
                },
                Field {
                    name: "question",
                    description: "What you ask, in Markdown, standing on its own: the work to \
                        do or the question, with the titles, ids and links it needs.",
                    kind: FieldKind::String,
                    required: true,
                },
                Field {
                    name: "reply",
                    description: "Whether you want a reply: true unless given, waiting for it. \
                        False hands the work over one way and returns at once; no reply ever \
                        comes.",
                    kind: FieldKind::Boolean,
                    required: false,
                },
                Field {
                    name: "done",
                    description: "Optional: with `reply` false, true when handing this over is the \
                        last thing your turn does: it ends once the work is handed over, with no \
                        final response. A call that waits for a reply goes on, to read it.",
                    kind: FieldKind::Boolean,
                    required: false,
                },
            ],
        }
    }

    /// Only a one-way hand-off ends the turn: a reply comes back to be read.
    fn ends_turn(&self, arguments: &str) -> bool {
        serde_json::from_str::<AskCall>(arguments).is_ok_and(|call| !call.reply)
            && asks_done(arguments)
    }

    fn activity(&self, arguments: &str) -> Option<Activity> {
        let call: AskCall = serde_json::from_str(arguments).ok()?;
        call.reply.then_some(Activity::Asking)
    }

    fn call<'a>(&'a self, seat: &'a Seat, arguments: &'a str) -> ToolFuture<'a> {
        Box::pin(async move {
            let call: AskCall = match serde_json::from_str(arguments) {
                Ok(call) => call,
                Err(e) => return ToolCallStatus::Failure(e.to_string()),
            };
            let question = call.question.trim();
            if question.is_empty() {
                return ToolCallStatus::Failure("the question is empty".to_string());
            }
            let ask = Ask {
                asker_id: self.agent_id,
                asker_run_id: seat.fork_run_id(),
                recipient_id: call.agent_id,
                question: question.to_string(),
                // The question continues the turn's trace, and the turn's
                // stop ends the wait.
                trace: seat.turn.trace(),
            };
            if !call.reply {
                return match self.messenger.hand_off(ask).await {
                    Ok(message_id) => ToolCallStatus::Success(format!(
                        "Handed over as [#{message_id}]; no reply comes back."
                    )),
                    Err(refusal) => ToolCallStatus::Failure(refusal),
                };
            }
            let asked = match self.messenger.ask(ask).await {
                Ok(asked) => asked,
                Err(refusal) => return ToolCallStatus::Failure(refusal),
            };
            let minutes = ASK_LIMIT.as_secs() / 60;
            match asked.wait(ASK_LIMIT, &seat.turn).await {
                Some(AgentAskReply::Answered { text }) => ToolCallStatus::Success(text),
                Some(AgentAskReply::Unanswered { reason }) => {
                    ToolCallStatus::Failure(format!("No answer: {reason}"))
                }
                Some(AgentAskReply::Waiting) => unreachable!("a fork settles its reply"),
                None => ToolCallStatus::Failure(format!(
                    "No answer: it did not reply within {minutes} minutes. It may still be \
                     working, but its reply won't reach you; carry on without it."
                )),
            }
        })
    }
}
