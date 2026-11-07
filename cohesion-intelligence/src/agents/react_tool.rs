use cohesion_db::{CommandError, DatabaseContext, ReactToUserMessageCommand};
use serde::Deserialize;

use super::agent_tool::{AgentTool, Seat};
use super::conversation_session::Branch;
use crate::tool_definition::{Field, FieldKind, Tool};
use crate::tools::{ToolFuture, asks_done, done_field};
use crate::transcript_item::ToolCallStatus;

#[derive(Deserialize)]
struct ReactCall {
    message_id: i64,
    emoji: String,
}

/// Lets the agent's conversation react to one of the user's messages with
/// an emoji, the way a co-worker would acknowledge a text.
pub(super) struct ReactTool {
    pub database: DatabaseContext,
    pub agent_id: i64,
}

impl AgentTool for ReactTool {
    fn definition(&self) -> Tool {
        Tool {
            name: "react",
            description: "Reacts to one of the user's messages with an emoji, shown on the \
                message like a reaction to a text, in place of a message when it says all there \
                is to say; prefer it to a message that only acknowledges. It counts as your reply to the \
                user, as `send_message` does: your final reply is never shown. Reacting again replaces your reaction."
                .into(),
            fields: vec![
                Field {
                    name: "message_id",
                    description: "The id of the user's message, as in `[#42]`.",
                    kind: FieldKind::Integer,
                    required: true,
                },
                Field {
                    name: "emoji",
                    description: "A single emoji, usually one of 👍 👎 ❤️ 😂 😮 😢 🎉 🙏; \"\" takes your reaction back.",
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
            let call: ReactCall = match serde_json::from_str(arguments) {
                Ok(call) => call,
                Err(e) => return ToolCallStatus::Failure(e.to_string()),
            };
            let result = self
                .database
                .send(ReactToUserMessageCommand {
                    agent_id: self.agent_id,
                    message_id: call.message_id,
                    reaction: Some(call.emoji),
                })
                .await;
            match result {
                Ok(message) => {
                    let reply = match message.reaction {
                        Some(_) => "Reacted.",
                        None => "Took your reaction back.",
                    };
                    // The conversation's turn keeps the message last
                    // reacted to, which stands as its reply when the agent
                    // says nothing else.
                    if let Branch::Conversation { reacted, .. } = &seat.branch {
                        *reacted.lock().expect("reacted is never poisoned") = Some(message);
                    }
                    ToolCallStatus::Success(reply.to_string())
                }
                Err(CommandError::Validation(e)) => ToolCallStatus::Failure(e.message),
                Err(CommandError::NotFound(_)) => ToolCallStatus::Failure(
                    "no message of the user's in this conversation has that id".to_string(),
                ),
                Err(e) => ToolCallStatus::Failure(format!("could not react: {e:?}")),
            }
        })
    }
}
