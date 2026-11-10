use cohesion_core::model::{CacheLength, CacheLifetime};
use serde::{Deserialize, Serialize};

use super::endpoint_key::EndpointKey;
use super::json_schema::JsonSchemaObject;
use super::post_json::{self, post, post_json};
use crate::intelligence_backend::{BackendFuture, IntelligenceBackend};
use crate::intelligence_error::IntelligenceError;
use crate::thinking::{RedactedThinking, Thinking};
use crate::tool_definition::Tool;
use crate::transcript::{RemoteTranscriptState, Transcript};
use crate::transcript_item::{ToolCallRequest, TranscriptItem};
use crate::turn::{Turn, TurnOutput};
use crate::usage::{TurnCost, Usage};

/// Speaks the Responses API, as served by OpenAI and xAI, keeping the
/// conversation as its [`ResponsesPreset`] says. `base_url` includes the
/// version, e.g. `https://api.openai.com/v1`.
///
/// Every request carries its session's `prompt_cache_key`, which both
/// providers use to route it to the server holding its cached prefix.
pub struct ResponsesBackend {
    base_url: String,
    model_identifier: String,
    key: EndpointKey,
    prompt_cache_key: String,
    preset: ResponsesPreset,
    /// The `ttl` of the endpoint's long cache, which a request asks for
    /// where its transcript's [`CachePlan`](crate::transcript::CachePlan)
    /// does, or none when the endpoint offers no long cache and requests
    /// leave the cache to the server's defaults.
    long_ttl: Option<String>,
    client: reqwest::Client,
}
impl ResponsesBackend {
    /// `prompt_cache_key` names requests that share a prefix: every turn of
    /// one transcript, and transcripts that start alike. `long` is the
    /// endpoint's long cache, if it offers one.
    pub fn new(
        base_url: String,
        key: EndpointKey,
        model_identifier: String,
        prompt_cache_key: String,
        preset: ResponsesPreset,
        long: CacheLifetime,
    ) -> Self {
        ResponsesBackend {
            base_url,
            model_identifier,
            key,
            prompt_cache_key,
            preset,
            long_ttl: match long {
                CacheLifetime::Short { .. } => None,
                CacheLifetime::Long(long) => Some(long.ttl()),
            },
            client: post_json::client(),
        }
    }
}

/// The settings a dialect wraps the Responses API with: whether the server
/// keeps the conversation.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ResponsesPreset {
    /// `Dialect::Responses`: responses are stored on the server, so after the
    /// first turn only the items added since the last response are sent,
    /// chained with `previous_response_id`.
    Stored,
    /// `Dialect::StatelessResponses`: nothing is stored, so each turn sends
    /// the whole transcript, with the system prompt as `instructions`, and
    /// streams its reply, read to the event that completes it. A ChatGPT
    /// plan serves only these.
    Stateless,
}
impl IntelligenceBackend for ResponsesBackend {
    fn send<'a>(&'a mut self, state: &'a Transcript) -> BackendFuture<'a> {
        Box::pin(self.send_turn(state))
    }
}
impl ResponsesBackend {
    async fn send_turn(&mut self, state: &Transcript) -> Result<Turn, IntelligenceError> {
        let remote = state
            .remote
            .as_ref()
            .filter(|_| self.preset == ResponsesPreset::Stored)
            .filter(|remote| remote.cursor <= state.items.len());
        match self.post(state, remote).await {
            // The server may have dropped the stored response (expiry, a different
            // project key); replaying the whole transcript starts a new chain.
            // OpenAI names the error `previous_response_not_found`; xAI answers 404.
            Err(IntelligenceError::UnexpectedStatus(status @ (400 | 404), body))
                if remote.is_some()
                    && (status == 404 || body.contains("previous_response_not_found")) =>
            {
                self.post(state, None).await
            }
            result => result,
        }
    }

    async fn post(
        &self,
        state: &Transcript,
        remote: Option<&RemoteTranscriptState>,
    ) -> Result<Turn, IntelligenceError> {
        let body = ResponsesRequest::new(
            &self.model_identifier,
            &self.prompt_cache_key,
            state,
            remote,
            self.preset,
            self.long_ttl.as_deref(),
        );
        let mut request = self
            .client
            .post(format!("{}/responses", self.base_url.trim_end_matches('/')));
        if let Some(key) = self.key.current().await? {
            request = request.bearer_auth(key);
        }
        let sent = state.items.len();
        match self.preset {
            ResponsesPreset::Stored => {
                let response: ResponsesResponse =
                    post_json(request, &body, is_context_limit_error).await?;
                response.into_turn(Some(sent))
            }
            ResponsesPreset::Stateless => {
                let events = post(request, &body, is_context_limit_error).await?;
                streamed_response(&events)?.into_turn(None)
            }
        }
    }
}

/// The response a stream of server-sent events ends with: the `response`
/// of its `response.completed` event, or of `response.incomplete`, whose
/// output stopped short. A `response.failed` or `error` event fails the
/// turn with its error, and so does a stream that ends before either.
fn streamed_response(events: &[u8]) -> Result<ResponsesResponse, IntelligenceError> {
    let events = String::from_utf8_lossy(events).replace("\r\n", "\n");
    for event in events.split("\n\n") {
        let data = event
            .lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .map(|data| data.strip_prefix(' ').unwrap_or(data))
            .collect::<Vec<_>>()
            .join("\n");
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let failure =
            match serde_json::from_str(&data).map_err(IntelligenceError::InvalidResponse)? {
                StreamEvent::Completed { response } => return Ok(response),
                StreamEvent::Failed { response } => response.error.unwrap_or_default(),
                StreamEvent::Error { code, message } => StreamError {
                    code: code.unwrap_or_default(),
                    message,
                },
                StreamEvent::Other => continue,
            };
        return Err(match failure.code.as_str() {
            "context_length_exceeded" => IntelligenceError::ContextLimitExceeded,
            _ => IntelligenceError::ResponseFailed {
                code: failure.code,
                message: failure.message,
            },
        });
    }
    Err(IntelligenceError::ResponseFailed {
        code: "stream_ended".to_string(),
        message: "The reply stopped before it finished.".to_string(),
    })
}

/// One server-sent event of a streamed response, by its `type`; only the
/// ones that end the stream matter.
#[derive(Deserialize)]
#[serde(tag = "type")]
enum StreamEvent {
    #[serde(rename = "response.completed", alias = "response.incomplete")]
    Completed { response: ResponsesResponse },
    #[serde(rename = "response.failed")]
    Failed { response: FailedResponse },
    #[serde(rename = "error")]
    Error {
        code: Option<String>,
        #[serde(default)]
        message: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct FailedResponse {
    error: Option<StreamError>,
}

#[derive(Deserialize, Default)]
struct StreamError {
    #[serde(default)]
    code: String,
    #[serde(default)]
    message: String,
}

fn is_context_limit_error(_status: u16, body: &str) -> bool {
    body.contains("context_length_exceeded")
}

#[derive(Serialize)]
struct ResponsesRequest<'a> {
    model: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_response_id: Option<&'a str>,
    /// A stateless request's system prompt. A stored one's rides as the
    /// chain's first input message instead: OpenAI drops `instructions` from
    /// a chained request and xAI refuses them alongside
    /// `previous_response_id`, but both keep a stored message. A ChatGPT
    /// plan, which serves only stateless requests, refuses system messages.
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<&'a str>,
    input: Vec<InputItem<'a>>,
    prompt_cache_key: &'a str,
    /// The cache the server's own breakpoint, at the end of the latest
    /// message, writes for, as the transcript's tail asks. Sent only to an
    /// endpoint with a long cache: one without leaves every cache setting
    /// to the server.
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_cache_options: Option<PromptCacheOptions<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ResponsesTool<'a>>,
    /// Keeps the response (and its reasoning) on the server so the next turn
    /// can chain from it.
    store: bool,
    /// Streams the reply as server-sent events.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stream: bool,
}
impl<'a> ResponsesRequest<'a> {
    fn new(
        model: &'a str,
        prompt_cache_key: &'a str,
        state: &'a Transcript,
        remote: Option<&'a RemoteTranscriptState>,
        preset: ResponsesPreset,
        long_ttl: Option<&'a str>,
    ) -> Self {
        let start = remote.map_or(0, |remote| remote.cursor);
        // A read lands only where an earlier request wrote, and the server
        // writes only at the end of the latest message, which a thread
        // copying this one's shared items never sends: it follows them with
        // its own. So the last shared item carries a breakpoint of its own,
        // for the long cache. Shared items that end the transcript are
        // cached as its tail is: the server's breakpoint lands on them.
        let shared = state.cache.shared_items;
        let (anchor, tail) = if shared > 0 && shared >= state.items.len() {
            (None, CacheLength::Long)
        } else {
            (shared.checked_sub(1), state.cache.tail)
        };
        let long = long_ttl
            .filter(|_| state.cache.prefix == CacheLength::Long)
            .map(|ttl| PromptCacheBreakpoint {
                mode: "explicit",
                ttl,
            });
        let breakpoint = |index: usize| long.filter(|_| Some(index) == anchor);
        let stored = preset == ResponsesPreset::Stored;
        // The system prompt ends in a breakpoint for the long cache too: every
        // thread and fork of an agent starts with it, so a new thread reads
        // it back rather than writing it behind its own first message. Only
        // a developer message holds one (`instructions` can't, so a
        // stateless request goes without).
        let system = state
            .system_prompt
            .as_deref()
            .filter(|_| stored && remote.is_none())
            .map(|content| InputItem::Message {
                role: if long_ttl.is_some() {
                    "developer"
                } else {
                    "system"
                },
                content: MessageContent::new(content, Vec::new(), long),
            });
        let items = state.items.iter().enumerate().skip(start);
        let items = items.filter_map(|(index, item)| match item {
            TranscriptItem::UserMessage(prompt) => Some(InputItem::Message {
                role: "user",
                content: MessageContent::new(
                    &prompt.text,
                    prompt.images.iter().map(|image| image.data_url()).collect(),
                    breakpoint(index),
                ),
            }),
            TranscriptItem::AssistantMessage(prompt) => Some(InputItem::Message {
                role: "assistant",
                content: MessageContent::Text(&prompt.text),
            }),
            TranscriptItem::ToolCallRequest(request) => Some(InputItem::FunctionCall {
                call_id: &request.call_identifier,
                name: &request.tool_name,
                arguments: &request.arguments,
            }),
            TranscriptItem::ToolCallResponse(response) => Some(InputItem::FunctionCallOutput {
                call_id: &response.call_identifier,
                output: MessageContent::new(response.response(), Vec::new(), breakpoint(index)),
            }),
            // Reasoning only survives through the stored response; a replayed
            // transcript goes without it.
            TranscriptItem::Thinking(_) => None,
        });

        ResponsesRequest {
            model,
            previous_response_id: remote.map(|remote| remote.conversation_id.as_str()),
            instructions: state.system_prompt.as_deref().filter(|_| !stored),
            input: system.into_iter().chain(items).collect(),
            prompt_cache_key,
            prompt_cache_options: long_ttl.map(|long_ttl| PromptCacheOptions {
                mode: "implicit",
                ttl: match tail {
                    CacheLength::Short => None,
                    CacheLength::Long => Some(long_ttl),
                },
            }),
            tools: state.tools.iter().map(ResponsesTool::from).collect(),
            store: stored,
            stream: !stored,
        }
    }
}

/// How long the server keeps the prompt cached, with the breakpoint it
/// places itself at the end of the latest message: its default (30
/// minutes on OpenAI) when `ttl` is none.
#[derive(Serialize, Clone, Copy)]
struct PromptCacheOptions<'a> {
    mode: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    ttl: Option<&'a str>,
}

/// A cache breakpoint of the request's own on a block of text, which
/// writes everything up to it to the cache for `ttl`.
#[derive(Serialize, Clone, Copy)]
struct PromptCacheBreakpoint<'a> {
    mode: &'static str,
    ttl: &'a str,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum InputItem<'a> {
    Message {
        role: &'static str,
        content: MessageContent<'a>,
    },
    FunctionCall {
        call_id: &'a str,
        name: &'a str,
        arguments: &'a str,
    },
    FunctionCallOutput {
        call_id: &'a str,
        output: MessageContent<'a>,
    },
}

#[derive(Serialize)]
#[serde(untagged)]
enum MessageContent<'a> {
    Text(&'a str),
    /// Text with images, or with a cache breakpoint, which only a part
    /// carries.
    Parts(Vec<InputPart<'a>>),
}
impl<'a> MessageContent<'a> {
    /// `text` with `breakpoint`, followed by `images` (data URLs), which
    /// take no breakpoint: the next one written covers them.
    fn new(
        text: &'a str,
        images: Vec<String>,
        breakpoint: Option<PromptCacheBreakpoint<'a>>,
    ) -> Self {
        if images.is_empty() && breakpoint.is_none() {
            return MessageContent::Text(text);
        }
        let text = InputPart::InputText {
            text,
            prompt_cache_breakpoint: breakpoint,
        };
        let images = images
            .into_iter()
            .map(|image_url| InputPart::InputImage { image_url });
        MessageContent::Parts(std::iter::once(text).chain(images).collect())
    }
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum InputPart<'a> {
    InputText {
        text: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        prompt_cache_breakpoint: Option<PromptCacheBreakpoint<'a>>,
    },
    InputImage {
        image_url: String,
    },
}

#[derive(Serialize)]
struct ResponsesTool<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    name: &'a str,
    description: &'a str,
    parameters: JsonSchemaObject<'a>,
}
impl<'a> From<&'a Tool> for ResponsesTool<'a> {
    fn from(tool: &'a Tool) -> Self {
        ResponsesTool {
            kind: "function",
            name: tool.name,
            description: &tool.description,
            parameters: JsonSchemaObject::from(tool),
        }
    }
}

#[derive(Deserialize)]
struct ResponsesResponse {
    id: String,
    #[serde(default)]
    output: Vec<OutputItem>,
    usage: Option<ResponsesUsage>,
}
impl ResponsesResponse {
    /// `sent` is how many transcript items a server storing the response has
    /// now seen, so the cursor lands after this turn's outputs once the
    /// session appends them; `None` when it stores nothing to chain from.
    fn into_turn(self, sent: Option<usize>) -> Result<Turn, IntelligenceError> {
        let mut output = Vec::new();
        for item in self.output {
            match item {
                OutputItem::Message { content } => {
                    let text: String = content
                        .into_iter()
                        .filter_map(|part| match part {
                            ContentPart::OutputText { text } => Some(text),
                            ContentPart::Refusal { refusal } => Some(refusal),
                            ContentPart::Other => None,
                        })
                        .collect();
                    output.push(TurnOutput::Message(text));
                }
                OutputItem::FunctionCall {
                    call_id,
                    name,
                    arguments,
                } => {
                    output.push(TurnOutput::ToolCall(ToolCallRequest {
                        call_identifier: call_id,
                        tool_name: name,
                        arguments,
                    }));
                }
                // The reasoning itself stays on the server; only its id and summary
                // come back.
                OutputItem::Reasoning { id, summary } => {
                    let summary: Vec<String> = summary.into_iter().map(|part| part.text).collect();
                    output.push(TurnOutput::Thinking(Thinking::Redacted(RedactedThinking {
                        trace: id,
                        summary: (!summary.is_empty()).then(|| summary.join("\n\n")),
                    })));
                }
                OutputItem::Other => {}
            }
        }

        let usage = self.usage.map_or_else(Usage::default, |usage| {
            let details = usage.input_tokens_details.unwrap_or_default();
            let long_cache_writes = details
                .cache_write_tokens_details
                .map_or(0, |writes| writes.ttl_12h_tokens);
            Usage {
                input_tokens: usage.input_tokens.saturating_sub(
                    details
                        .cached_tokens
                        .saturating_add(details.cache_write_tokens),
                ),
                output_tokens: usage.output_tokens,
                cache_reads: details.cached_tokens,
                cache_writes: details.cache_write_tokens,
                long_cache_writes,
            }
        });

        Ok(Turn {
            remote: sent.map(|sent| RemoteTranscriptState {
                conversation_id: self.id,
                cursor: sent + output.len(),
            }),
            output,
            usage,
            cost: TurnCost::None,
        })
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OutputItem {
    Message {
        content: Vec<ContentPart>,
    },
    FunctionCall {
        call_id: String,
        name: String,
        arguments: String,
    },
    Reasoning {
        id: String,
        #[serde(default)]
        summary: Vec<SummaryPart>,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentPart {
    OutputText {
        text: String,
    },
    Refusal {
        refusal: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct SummaryPart {
    text: String,
}

#[derive(Deserialize)]
struct ResponsesUsage {
    /// Includes cached tokens, read and written.
    input_tokens: u64,
    output_tokens: u64,
    input_tokens_details: Option<ResponsesInputTokensDetails>,
}

#[derive(Deserialize, Default)]
struct ResponsesInputTokensDetails {
    /// Read from the cache.
    #[serde(default)]
    cached_tokens: u64,
    /// Written to the cache, from a server that says (OpenAI, since GPT-5.6).
    #[serde(default)]
    cache_write_tokens: u64,
    /// How `cache_write_tokens` splits by how long the cache lasts.
    cache_write_tokens_details: Option<CacheWriteTokensDetails>,
}

#[derive(Deserialize)]
struct CacheWriteTokensDetails {
    #[serde(default)]
    ttl_12h_tokens: u64,
}

#[cfg(test)]
mod tests;
