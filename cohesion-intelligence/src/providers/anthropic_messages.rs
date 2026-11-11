use cohesion_core::model::{CacheLength, CacheLifetime};
use serde::{Deserialize, Serialize};

use super::endpoint_key::EndpointKey;
use super::json_schema::JsonSchemaObject;
use super::post_json::{self, post_json};
use crate::intelligence_backend::{BackendFuture, IntelligenceBackend};
use crate::intelligence_error::IntelligenceError;
use crate::thinking::{RedactedThinking, StandardThinking, Thinking};
use crate::tool_definition::Tool;
use crate::transcript::Transcript;
use crate::transcript_item::{ToolCallRequest, ToolCallStatus, TranscriptItem};
use crate::turn::{Turn, TurnOutput};
use crate::usage::{TurnCost, Usage};

const ANTHROPIC_VERSION: &str = "2023-06-01";
/// Requests aren't streamed, so stay well under the point where the API
/// refuses non-streaming requests for taking too long.
const MAX_TOKENS: u32 = 16_000;

/// Speaks Anthropic's Messages API. `base_url` includes the version, e.g.
/// `https://api.anthropic.com/v1`.
pub struct MessagesBackend {
    base_url: String,
    model_identifier: String,
    key: EndpointKey,
    /// The `ttl` a cache breakpoint asking for the long cache names, or
    /// none when the endpoint offers only the API's default five minutes.
    long_ttl: Option<String>,
    client: reqwest::Client,
}
impl MessagesBackend {
    /// The backend whose requests ask for `long`, the endpoint's long
    /// cache, where a transcript's [`CachePlan`](crate::transcript::CachePlan)
    /// asks for [`CacheLength::Long`], and the API's default elsewhere.
    pub fn new(
        base_url: String,
        key: EndpointKey,
        model_identifier: String,
        long: CacheLifetime,
    ) -> Self {
        MessagesBackend {
            base_url,
            model_identifier,
            key,
            long_ttl: super::long_ttl(long),
            client: post_json::client(),
        }
    }
}
impl IntelligenceBackend for MessagesBackend {
    fn send<'a>(&'a mut self, state: &'a Transcript) -> BackendFuture<'a> {
        Box::pin(self.send_turn(state))
    }
}
impl MessagesBackend {
    async fn send_turn(&mut self, state: &Transcript) -> Result<Turn, IntelligenceError> {
        let cache_control = |length| CacheControl {
            kind: "ephemeral",
            ttl: match length {
                CacheLength::Short => None,
                CacheLength::Long => self.long_ttl.as_deref(),
            },
        };
        let body = MessagesRequest::new(&self.model_identifier, state, cache_control)?;
        let mut request = self
            .client
            .post(format!("{}/messages", self.base_url.trim_end_matches('/')))
            .header("anthropic-version", ANTHROPIC_VERSION);
        if let Some(key) = self.key.current().await? {
            request = request.header("x-api-key", key);
        }
        let response: MessagesResponse = post_json(request, &body, is_context_limit_error).await?;
        response.into_turn()
    }
}

fn is_context_limit_error(status: u16, body: &str) -> bool {
    status == 413 || body.contains("prompt is too long")
}

#[derive(Serialize)]
struct MessagesRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    /// The system prompt, ending in its own cache breakpoint. A read lands
    /// only where an earlier request wrote one, and the tools and system
    /// prompt are the same for every thread and run of an agent's role, so
    /// a new thread or a fork reads them back instead of writing
    /// them again behind its own first message.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    system: Vec<SystemBlock<'a>>,
    /// The messages, the last block of the transcript's shared items
    /// ending in a breakpoint for the long cache, so a thread copying them
    /// reads them back after this one has moved on.
    messages: Vec<MessagesMessage<'a>>,
    /// The tools, the last ending in a cache breakpoint of its own: they
    /// come before the system prompt, and every agent's sessions of one
    /// kind share them, so one agent's new thread reads back the tools
    /// another agent's request wrote even where their system prompts differ.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<MessagesTool<'a>>,
    /// Caches the whole prefix each turn, so a tool loop only pays the full
    /// input rate for what was added since the last request, for as long as
    /// the transcript's tail asks. The API wants every breakpoint for a
    /// longer cache before those for a shorter one, which this, the last,
    /// keeps.
    cache_control: CacheControl<'a>,
}
impl<'a> MessagesRequest<'a> {
    /// The request for `state`, caching as its plan says, each length as
    /// `cache_control` gives it.
    fn new(
        model: &'a str,
        state: &'a Transcript,
        cache_control: impl Fn(CacheLength) -> CacheControl<'a>,
    ) -> Result<Self, IntelligenceError> {
        let long = cache_control(state.cache.prefix);
        let (anchor, tail) = state.cache.shared_breakpoint(state.items.len());
        let tail = cache_control(tail);
        let mut messages: Vec<MessagesMessage<'a>> = Vec::with_capacity(state.items.len());
        for (index, item) in state.items.iter().enumerate() {
            let (role, block) = match item {
                TranscriptItem::UserMessage(prompt) => {
                    (Role::User, ContentBlock::Text { text: &prompt.text })
                }
                TranscriptItem::ToolCallResponse(response) => (
                    Role::User,
                    ContentBlock::ToolResult {
                        tool_use_id: &response.call_identifier,
                        content: response.response(),
                        is_error: matches!(response.response, ToolCallStatus::Failure(_)),
                    },
                ),
                TranscriptItem::Thinking(Thinking::Standard(standard)) => {
                    // The API rejects thinking it can't verify, which is anything
                    // that didn't come from this backend.
                    let Some(signature) = &standard.signature else {
                        continue;
                    };
                    (
                        Role::Assistant,
                        ContentBlock::Thinking {
                            thinking: &standard.thought,
                            signature,
                        },
                    )
                }
                TranscriptItem::Thinking(Thinking::Redacted(redacted)) => (
                    Role::Assistant,
                    ContentBlock::RedactedThinking {
                        data: &redacted.trace,
                    },
                ),
                TranscriptItem::AssistantMessage(prompt) => {
                    (Role::Assistant, ContentBlock::Text { text: &prompt.text })
                }
                TranscriptItem::ToolCallRequest(request) => (
                    Role::Assistant,
                    ContentBlock::ToolUse {
                        id: &request.call_identifier,
                        name: &request.tool_name,
                        input: serde_json::from_str(&request.arguments)
                            .map_err(IntelligenceError::Serialization)?,
                    },
                ),
            };
            let mut blocks = vec![block];
            if let TranscriptItem::UserMessage(prompt) = item {
                blocks.extend(prompt.images.iter().map(|image| ContentBlock::Image {
                    source: ImageSource {
                        kind: "base64",
                        media_type: &image.media_type,
                        data: &image.data,
                    },
                }));
            }
            let mut blocks: Vec<CachedBlock<'a>> = blocks
                .into_iter()
                .map(|block| CachedBlock {
                    block,
                    cache_control: None,
                })
                .collect();
            if Some(index) == anchor
                && let Some(last) = blocks.last_mut()
                && last.block.takes_cache_control()
            {
                last.cache_control = Some(long);
            }
            // Roles alternate on the wire, so everything from one turn (and every
            // tool result answering it) shares a single message.
            match messages.last_mut() {
                Some(last) if last.role == role => last.content.extend(blocks),
                _ => messages.push(MessagesMessage {
                    role,
                    content: blocks,
                }),
            }
        }

        Ok(MessagesRequest {
            model,
            max_tokens: MAX_TOKENS,
            system: state
                .system_prompt
                .iter()
                .map(|text| SystemBlock {
                    kind: "text",
                    text,
                    cache_control: long,
                })
                .collect(),
            messages,
            tools: tools(&state.tools, long),
            cache_control: tail,
        })
    }
}

#[derive(Serialize)]
struct SystemBlock<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    text: &'a str,
    cache_control: CacheControl<'a>,
}

/// A cache breakpoint: the prefix up to it stays cached for the API's
/// default five minutes, or for `ttl` when it is set.
#[derive(Serialize, Clone, Copy)]
struct CacheControl<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    ttl: Option<&'a str>,
}

#[derive(Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Role {
    User,
    Assistant,
}

#[derive(Serialize)]
struct MessagesMessage<'a> {
    role: Role,
    content: Vec<CachedBlock<'a>>,
}

/// A content block, ending in a cache breakpoint when it closes the
/// transcript's shared items.
#[derive(Serialize)]
struct CachedBlock<'a> {
    #[serde(flatten)]
    block: ContentBlock<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_control: Option<CacheControl<'a>>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentBlock<'a> {
    Text {
        text: &'a str,
    },
    Image {
        source: ImageSource<'a>,
    },
    Thinking {
        thinking: &'a str,
        signature: &'a str,
    },
    RedactedThinking {
        data: &'a str,
    },
    ToolUse {
        id: &'a str,
        name: &'a str,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: &'a str,
        content: &'a str,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        is_error: bool,
    },
}

impl ContentBlock<'_> {
    /// Whether the API takes a cache breakpoint on the block: thinking
    /// takes none.
    fn takes_cache_control(&self) -> bool {
        !matches!(
            self,
            ContentBlock::Thinking { .. } | ContentBlock::RedactedThinking { .. }
        )
    }
}

#[derive(Serialize)]
struct ImageSource<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    media_type: &'a str,
    data: &'a str,
}

#[derive(Serialize)]
struct MessagesTool<'a> {
    name: &'a str,
    description: &'a str,
    input_schema: JsonSchemaObject<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_control: Option<CacheControl<'a>>,
}

/// `tools` on the wire, the last carrying `cache_control`.
fn tools<'a>(tools: &'a [Tool], cache_control: CacheControl<'a>) -> Vec<MessagesTool<'a>> {
    let last = tools.len().saturating_sub(1);
    tools
        .iter()
        .enumerate()
        .map(|(index, tool)| MessagesTool {
            name: tool.name,
            description: &tool.description,
            input_schema: JsonSchemaObject::from(tool),
            cache_control: (index == last).then_some(cache_control),
        })
        .collect()
}

#[derive(Deserialize)]
struct MessagesResponse {
    content: Vec<ResponseBlock>,
    stop_reason: Option<String>,
    usage: MessagesUsage,
}
impl MessagesResponse {
    fn into_turn(self) -> Result<Turn, IntelligenceError> {
        if self.stop_reason.as_deref() == Some("model_context_window_exceeded") {
            return Err(IntelligenceError::ContextLimitExceeded);
        }

        let output = self
            .content
            .into_iter()
            .filter_map(|block| match block {
                // An empty text block says nothing, and sent back in the
                // thread's next request the API refuses it.
                ResponseBlock::Text { text } => {
                    (!text.is_empty()).then_some(TurnOutput::Message(text))
                }
                // Kept even when the thought is empty (the default on newer models):
                // the signature must go back with the next request.
                ResponseBlock::Thinking {
                    thinking,
                    signature,
                } => Some(TurnOutput::Thinking(Thinking::Standard(StandardThinking {
                    thought: thinking,
                    signature: Some(signature),
                    in_content: false,
                }))),
                ResponseBlock::RedactedThinking { data } => {
                    Some(TurnOutput::Thinking(Thinking::Redacted(RedactedThinking {
                        trace: data,
                        summary: None,
                    })))
                }
                ResponseBlock::ToolUse { id, name, input } => {
                    Some(TurnOutput::ToolCall(ToolCallRequest {
                        call_identifier: id,
                        tool_name: name,
                        arguments: input.to_string(),
                    }))
                }
                ResponseBlock::Other => None,
            })
            .collect();

        Ok(Turn {
            remote: None,
            output,
            usage: Usage {
                input_tokens: self.usage.input_tokens,
                output_tokens: self.usage.output_tokens,
                cache_reads: self.usage.cache_read_input_tokens.unwrap_or(0),
                cache_writes: self.usage.cache_creation_input_tokens.unwrap_or(0),
                long_cache_writes: self
                    .usage
                    .cache_creation
                    .map_or(0, |creation| creation.ephemeral_1h_input_tokens),
            },
            cost: TurnCost::None,
        })
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ResponseBlock {
    Text {
        text: String,
    },
    Thinking {
        thinking: String,
        signature: String,
    },
    RedactedThinking {
        data: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct MessagesUsage {
    /// Excludes cache reads and writes.
    input_tokens: u64,
    output_tokens: u64,
    cache_read_input_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    /// How `cache_creation_input_tokens` splits by how long the cache lasts.
    cache_creation: Option<CacheCreation>,
}

#[derive(Deserialize)]
struct CacheCreation {
    #[serde(default)]
    ephemeral_1h_input_tokens: u64,
}

#[cfg(test)]
mod tests;
