use crate::thinking::Thinking;
use crate::transcript::RemoteTranscriptState;
use crate::transcript_item::{Prompt, ToolCallRequest, TranscriptItem};
use crate::usage::{TurnCost, Usage};

/// One answer from the model. The exchange goes on while a turn's output
/// holds tool calls, whatever the provider says its stop reason was.
#[derive(Debug)]
pub struct Turn {
    pub remote: Option<RemoteTranscriptState>,
    pub output: Vec<TurnOutput>,
    pub usage: Usage,
    /// What the provider reported the turn cost, if it did.
    pub cost: TurnCost,
}

#[derive(Debug)]
pub enum TurnOutput {
    Message(String),
    ToolCall(ToolCallRequest),
    Thinking(Thinking),
}
impl From<TurnOutput> for TranscriptItem {
    fn from(value: TurnOutput) -> Self {
        match value {
            TurnOutput::Message(msg) => Self::AssistantMessage(Prompt {
                text: msg,
                images: Vec::new(),
            }),
            TurnOutput::ToolCall(tcr) => Self::ToolCallRequest(tcr),
            TurnOutput::Thinking(thk) => Self::Thinking(thk),
        }
    }
}
