use cohesion_core::model::AgentAskReply;
use serde::Deserialize;

use super::agent_tool::{AgentTool, Seat};
use super::conversation_session::Branch;
use crate::tool_definition::{Field, FieldKind, Tool};
use crate::tools::{ToolFuture, asks_done, done_field};
use crate::transcript_item::ToolCallStatus;

#[derive(Deserialize)]
struct ReplyCall {
    text: String,
}

/// Lets a fork that another agent's question woke reply to it, while that
/// agent waits. Every session of the conversation has it, so a fork's tools
/// match the conversation's and its prompt stays cached; only a fork owing
/// a reply can use it.
pub(super) struct ReplyTool;

impl AgentTool for ReplyTool {
    fn definition(&self) -> Tool {
        Tool {
            name: "reply_to_agent",
            description: "Replies to the agent whose question opened this turn under \
                `# <name> asks`, when the question says it waits for your reply: the agent gets \
                `text`, and the user never sees it. This is the only way to answer it, since \
                your final response reaches no one. Reply once, with what it asked for, never \
                just \"Done\". Nowhere else is there anyone to reply to: a one-way question \
                wants the work done, and the user hears only `send_message`."
                .into(),
            fields: vec![
                Field {
                    name: "text",
                    description: "Your answer, in Markdown, standing on its own.",
                    kind: FieldKind::String,
                    required: true,
                },
                done_field(),
            ],
        }
    }

    fn ends_turn(&self, arguments: &str) -> bool {
        asks_done(arguments)
    }

    fn call<'a>(&'a self, seat: &'a Seat, arguments: &'a str) -> ToolFuture<'a> {
        Box::pin(async move {
            let call: ReplyCall = match serde_json::from_str(arguments) {
                Ok(call) => call,
                Err(e) => return ToolCallStatus::Failure(e.to_string()),
            };
            // The reply a fork owes; none anywhere else, including a fork
            // a one-way question woke.
            let Branch::Fork {
                owed: Some(owed), ..
            } = &seat.branch
            else {
                return ToolCallStatus::Failure(
                    "No agent waits for a reply from this turn. A one-way question wants only \
                     the work done and `OK` as your final response; to tell the user something, \
                     use `send_message`."
                        .to_string(),
                );
            };
            let text = call.text.trim();
            if text.is_empty() {
                return ToolCallStatus::Failure("the reply is empty".to_string());
            }
            let reply = AgentAskReply::Answered {
                text: text.to_string(),
            };
            if !owed.settle(reply).await {
                return ToolCallStatus::Failure(
                    "You have replied already; the agent has your reply.".to_string(),
                );
            }
            ToolCallStatus::Success("Replied.".to_string())
        })
    }
}
