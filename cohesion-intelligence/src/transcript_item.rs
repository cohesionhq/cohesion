use cohesion_core::model::{Attachment, ImageBytes, ThreadItem};
use cohesion_db::{CreateIntelligenceMessageCommand, ThreadEntry};
use serde::{Deserialize, Serialize};

use crate::thinking::{RedactedThinking, StandardThinking, Thinking};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TranscriptItem {
    UserMessage(Prompt),
    AssistantMessage(Prompt),
    Thinking(Thinking),
    ToolCallRequest(ToolCallRequest),
    ToolCallResponse(ToolCallResponse),
}

impl TranscriptItem {
    /// The command that stores this item as a message of thread `thread_id`.
    pub(crate) fn as_create_message_command(
        &self,
        thread_id: i64,
    ) -> CreateIntelligenceMessageCommand {
        let images = match self {
            Self::UserMessage(prompt) => prompt
                .images
                .iter()
                .filter_map(|image| {
                    let data = ImageBytes::from_base64(&image.data).ok()?;
                    Some((image.media_type.clone(), data))
                })
                .collect(),
            _ => Vec::new(),
        };
        CreateIntelligenceMessageCommand {
            thread_id,
            item: self.stored(),
            images,
        }
    }

    /// This item as a thread message holds it.
    fn stored(&self) -> ThreadItem {
        match self {
            Self::UserMessage(m) => ThreadItem::UserMessage {
                text: m.text.clone(),
            },
            Self::AssistantMessage(m) => ThreadItem::AssistantMessage {
                text: m.text.clone(),
            },
            Self::Thinking(Thinking::Standard(s)) => ThreadItem::Thought {
                text: s.thought.clone(),
                signature: s.signature.clone(),
                in_content: s.in_content,
            },
            Self::Thinking(Thinking::Redacted(s)) => ThreadItem::RedactedThought {
                trace: s.trace.clone(),
                summary: s.summary.clone(),
            },
            Self::ToolCallRequest(r) => ThreadItem::ToolCallRequest {
                call_identifier: r.call_identifier.clone(),
                tool_name: r.tool_name.clone(),
                arguments: r.arguments.clone(),
            },
            Self::ToolCallResponse(r) => ThreadItem::ToolCallResponse {
                call_identifier: r.call_identifier.clone(),
                succeeded: matches!(r.response, ToolCallStatus::Success(_)),
                text: r.response().clone(),
            },
        }
    }
}

/// A stored thread message as the transcript a provider is sent holds it.
impl From<ThreadEntry> for TranscriptItem {
    fn from(entry: ThreadEntry) -> Self {
        match entry.message.item {
            ThreadItem::UserMessage { text } => Self::UserMessage(Prompt {
                text,
                images: entry
                    .images
                    .into_iter()
                    .map(|image| PromptImage {
                        media_type: image.media_type,
                        data: image.data.to_base64(),
                    })
                    .collect(),
            }),
            ThreadItem::AssistantMessage { text } => Self::AssistantMessage(Prompt {
                text,
                images: Vec::new(),
            }),
            ThreadItem::Thought {
                text,
                signature,
                in_content,
            } => Self::Thinking(Thinking::Standard(StandardThinking {
                thought: text,
                signature,
                in_content,
            })),
            ThreadItem::RedactedThought { trace, summary } => {
                Self::Thinking(Thinking::Redacted(RedactedThinking { trace, summary }))
            }
            ThreadItem::ToolCallRequest {
                call_identifier,
                tool_name,
                arguments,
            } => Self::ToolCallRequest(ToolCallRequest {
                call_identifier,
                tool_name,
                arguments,
            }),
            ThreadItem::ToolCallResponse {
                call_identifier,
                succeeded,
                text,
            } => Self::ToolCallResponse(ToolCallResponse {
                call_identifier,
                response: if succeeded {
                    ToolCallStatus::Success(text)
                } else {
                    ToolCallStatus::Failure(text)
                },
            }),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCallRequest {
    pub call_identifier: String,
    pub tool_name: String,
    pub arguments: String,
}
impl ToolCallRequest {
    pub fn respond(&self, response: ToolCallStatus) -> TranscriptItem {
        TranscriptItem::ToolCallResponse(ToolCallResponse {
            call_identifier: self.call_identifier.clone(),
            response,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToolCallStatus {
    Success(String),
    Failure(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCallResponse {
    pub call_identifier: String,
    pub response: ToolCallStatus,
}
impl ToolCallResponse {
    pub fn response(&self) -> &String {
        match &self.response {
            ToolCallStatus::Success(msg) => msg,
            ToolCallStatus::Failure(msg) => msg,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Prompt {
    pub text: String,
    /// Images sent with a user message, after its text.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<PromptImage>,
}

/// An image the model sees with a user message.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromptImage {
    /// `image/png`, `image/jpeg`, `image/gif` or `image/webp`.
    pub media_type: String,
    /// The image, base64-encoded.
    pub data: String,
}
/// An image attached to a user's message to an agent, as the model sees
/// it.
impl From<Attachment> for PromptImage {
    fn from(attachment: Attachment) -> Self {
        Self {
            media_type: attachment.media_type,
            data: attachment.data.to_base64(),
        }
    }
}
impl PromptImage {
    /// The image as a `data:` URL.
    pub fn data_url(&self) -> String {
        format!("data:{};base64,{}", self.media_type, self.data)
    }
}

#[cfg(test)]
mod tests;
