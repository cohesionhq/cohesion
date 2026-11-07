use std::ops::Range;

use cohesion_db::{CommandError, DatabaseContext, GetAgentMessageCommand};
use serde::Deserialize;

use super::agent_tool::{AgentTool, Seat};
use super::agent_transcript::message_text;
use crate::tool_definition::{Field, FieldKind, Tool};
use crate::tools::ToolFuture;
use crate::transcript_item::ToolCallStatus;

#[derive(Deserialize)]
struct ReadCall {
    id: i64,
    start: Option<i64>,
    end: Option<i64>,
}

/// Lets the agent's conversation read a message of its own conversation
/// that its transcript only excerpts or left out, or a script output its code
/// tool cut.
pub(super) struct ReadMessageTool {
    pub database: DatabaseContext,
    pub agent_id: i64,
}

/// The characters `start..end` of a text of `length` characters, `end`
/// defaulting to the end and clamped to it.
fn range(start: Option<i64>, end: Option<i64>, length: usize) -> Result<Range<usize>, String> {
    let start = start.unwrap_or(0);
    let end = end.map_or(length as i64, |end| end.min(length as i64));
    if start < 0 || start > end {
        return Err(format!(
            "start must be from 0 to end; the message is {length} characters long"
        ));
    }
    Ok(start as usize..end as usize)
}

/// The characters of `text` in `range`.
fn slice(text: &str, range: Range<usize>) -> String {
    text.chars().skip(range.start).take(range.len()).collect()
}

impl AgentTool for ReadMessageTool {
    fn definition(&self) -> Tool {
        Tool {
            name: "read_message",
            description: "Reads the text of a message in this conversation, or part of it. Use \
                it when a line in your conversation or a script's output ends in a note \
                of omitted characters, for the output of a script a fork ran, which your \
                conversation lists by id and title, or for an older message by its id."
                .into(),
            fields: vec![
                Field {
                    name: "id",
                    description: "The id of the message, as in `[#42]`.",
                    kind: FieldKind::Integer,
                    required: true,
                },
                Field {
                    name: "start",
                    description: "The character to start at; 0 unless given.",
                    kind: FieldKind::Integer,
                    required: false,
                },
                Field {
                    name: "end",
                    description: "The character to stop before; the end of the message unless given.",
                    kind: FieldKind::Integer,
                    required: false,
                },
            ],
        }
    }

    fn call<'a>(&'a self, _seat: &'a Seat, arguments: &'a str) -> ToolFuture<'a> {
        Box::pin(async move {
            let call: ReadCall = match serde_json::from_str(arguments) {
                Ok(call) => call,
                Err(e) => return ToolCallStatus::Failure(e.to_string()),
            };
            let message = match self
                .database
                .send(GetAgentMessageCommand {
                    message_id: call.id,
                })
                .await
            {
                Ok(message) if message.agent_id == self.agent_id => message,
                Ok(_) | Err(CommandError::NotFound(_)) => {
                    return ToolCallStatus::Failure(
                        "no message in this conversation has that id".to_string(),
                    );
                }
                Err(e) => return ToolCallStatus::Failure(format!("could not read: {e:?}")),
            };
            let Some(text) = message_text(&message.message_content) else {
                return ToolCallStatus::Failure("that message has no text".to_string());
            };
            match range(call.start, call.end, text.chars().count()) {
                Ok(range) => ToolCallStatus::Success(slice(&text, range)),
                Err(e) => ToolCallStatus::Failure(e),
            }
        })
    }
}

#[cfg(test)]
mod tests;
