use serde::{Deserialize, Serialize};

use super::endpoint_key::EndpointKey;
use super::json_schema::JsonSchemaObject;
use super::post_json::{self, post_json};
use crate::intelligence_backend::{BackendFuture, IntelligenceBackend};
use crate::intelligence_error::IntelligenceError;
use crate::thinking::{StandardThinking, Thinking};
use crate::tool_definition::Tool;
use crate::transcript::Transcript;
use crate::transcript_item::{ToolCallRequest, TranscriptItem};
use crate::turn::{Turn, TurnOutput};
use crate::usage::{TurnCost, Usage};

pub struct ChatCompletionsBackend {
    base_url: String,
    model_identifier: String,
    key: EndpointKey,
    client: reqwest::Client,
}
impl ChatCompletionsBackend {
    pub fn new(base_url: String, key: EndpointKey, model_identifier: String) -> Self {
        ChatCompletionsBackend {
            base_url,
            model_identifier,
            key,
            client: post_json::client(),
        }
    }
}
impl IntelligenceBackend for ChatCompletionsBackend {
    fn send<'a>(&'a mut self, state: &'a Transcript) -> BackendFuture<'a> {
        Box::pin(self.send_turn(state))
    }
}
impl ChatCompletionsBackend {
    async fn send_turn(&mut self, state: &Transcript) -> Result<Turn, IntelligenceError> {
        let body = ChatCompletionsRequest::new(&self.model_identifier, state);
        let mut request = self.client.post(format!(
            "{}/chat/completions",
            self.base_url.trim_end_matches('/')
        ));
        if let Some(key) = self.key.current().await? {
            request = request.bearer_auth(key);
        }
        let response: ChatCompletionsResponse =
            post_json(request, &body, is_context_limit_error).await?;
        Ok(response.into_turn())
    }
}

/// There's no standard error code for this, so match the ones used by
/// OpenAI, vLLM, and llama.cpp.
fn is_context_limit_error(_status: u16, body: &str) -> bool {
    [
        "context_length_exceeded",
        "maximum context length",
        "exceed_context_size_error",
    ]
    .iter()
    .any(|needle| body.contains(needle))
}

#[derive(Serialize)]
struct ChatCompletionsRequest<'a> {
    model: &'a str,
    messages: Vec<ChatCompletionsMessage<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ChatCompletionsTool<'a>>,
}
impl<'a> ChatCompletionsRequest<'a> {
    fn new(model: &'a str, state: &'a Transcript) -> Self {
        let mut messages = Vec::with_capacity(state.items.len() + 1);
        if let Some(system) = &state.system_prompt {
            messages.push(ChatCompletionsMessage::System { content: system });
        }
        for item in &state.items {
            match item {
                TranscriptItem::UserMessage(prompt) if prompt.images.is_empty() => {
                    messages.push(ChatCompletionsMessage::User {
                        content: ChatCompletionsUserContent::Text(&prompt.text),
                    });
                }
                TranscriptItem::UserMessage(prompt) => {
                    let parts =
                        std::iter::once(ChatCompletionsUserPart::Text { text: &prompt.text })
                            .chain(prompt.images.iter().map(|image| {
                                ChatCompletionsUserPart::ImageUrl {
                                    image_url: ChatCompletionsImageUrl {
                                        url: image.data_url(),
                                    },
                                }
                            }))
                            .collect();
                    messages.push(ChatCompletionsMessage::User {
                        content: ChatCompletionsUserContent::Parts(parts),
                    });
                }
                TranscriptItem::ToolCallResponse(response) => {
                    messages.push(ChatCompletionsMessage::Tool {
                        tool_call_id: &response.call_identifier,
                        content: response.response(),
                    });
                }
                // Thinking, text and tool calls from one turn are a single assistant
                // message on the wire, so fold them into the trailing one when they fit.
                TranscriptItem::Thinking(thinking) => {
                    let Thinking::Standard(standard) = thinking else {
                        continue;
                    };
                    // Send the thought back the way it arrived.
                    let (reasoning, parts) = if standard.in_content {
                        let part = ChatCompletionsMessagePart::Thinking {
                            thinking: vec![ChatCompletionsMessagePart::Text {
                                text: &standard.thought,
                            }],
                        };
                        (None, Some(ChatCompletionsMessageContent::Parts(vec![part])))
                    } else {
                        (Some(standard.thought.as_str()), None)
                    };
                    match messages.last_mut() {
                        Some(ChatCompletionsMessage::Assistant {
                            reasoning_content: reasoning_content @ None,
                            content: content @ None,
                            tool_calls,
                        }) if tool_calls.is_empty() => {
                            *reasoning_content = reasoning;
                            *content = parts;
                        }
                        _ => messages.push(ChatCompletionsMessage::Assistant {
                            reasoning_content: reasoning,
                            content: parts,
                            tool_calls: Vec::new(),
                        }),
                    }
                }
                TranscriptItem::AssistantMessage(prompt) => match messages.last_mut() {
                    Some(ChatCompletionsMessage::Assistant {
                        content: Some(ChatCompletionsMessageContent::Parts(parts)),
                        tool_calls,
                        ..
                    }) if tool_calls.is_empty() => {
                        parts.push(ChatCompletionsMessagePart::Text { text: &prompt.text })
                    }
                    Some(ChatCompletionsMessage::Assistant {
                        content: content @ None,
                        tool_calls,
                        ..
                    }) if tool_calls.is_empty() => {
                        *content = Some(ChatCompletionsMessageContent::Text(&prompt.text))
                    }
                    _ => messages.push(ChatCompletionsMessage::Assistant {
                        reasoning_content: None,
                        content: Some(ChatCompletionsMessageContent::Text(&prompt.text)),
                        tool_calls: Vec::new(),
                    }),
                },
                TranscriptItem::ToolCallRequest(request) => {
                    let call = ChatCompletionsToolCall {
                        id: &request.call_identifier,
                        kind: "function",
                        function: ChatCompletionsFunctionCall {
                            name: &request.tool_name,
                            arguments: &request.arguments,
                        },
                    };
                    match messages.last_mut() {
                        Some(ChatCompletionsMessage::Assistant { tool_calls, .. }) => {
                            tool_calls.push(call)
                        }
                        _ => messages.push(ChatCompletionsMessage::Assistant {
                            reasoning_content: None,
                            content: None,
                            tool_calls: vec![call],
                        }),
                    }
                }
            }
        }

        ChatCompletionsRequest {
            model,
            messages,
            tools: state.tools.iter().map(ChatCompletionsTool::from).collect(),
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "role", rename_all = "lowercase")]
enum ChatCompletionsMessage<'a> {
    System {
        content: &'a str,
    },
    User {
        content: ChatCompletionsUserContent<'a>,
    },
    Assistant {
        /// Non-standard, but required by reasoning models (DeepSeek, Kimi, etc.)
        /// to keep their thinking across tool calls.
        #[serde(skip_serializing_if = "Option::is_none")]
        reasoning_content: Option<&'a str>,
        content: Option<ChatCompletionsMessageContent<'a>>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<ChatCompletionsToolCall<'a>>,
    },
    Tool {
        tool_call_id: &'a str,
        content: &'a str,
    },
}

#[derive(Serialize)]
#[serde(untagged)]
enum ChatCompletionsMessageContent<'a> {
    Text(&'a str),
    /// Mistral's reasoning models expect their thinking chunks back as sent.
    Parts(Vec<ChatCompletionsMessagePart<'a>>),
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum ChatCompletionsMessagePart<'a> {
    Text {
        text: &'a str,
    },
    Thinking {
        thinking: Vec<ChatCompletionsMessagePart<'a>>,
    },
}

#[derive(Serialize)]
#[serde(untagged)]
enum ChatCompletionsUserContent<'a> {
    Text(&'a str),
    /// A user message with images.
    Parts(Vec<ChatCompletionsUserPart<'a>>),
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ChatCompletionsUserPart<'a> {
    Text { text: &'a str },
    ImageUrl { image_url: ChatCompletionsImageUrl },
}

#[derive(Serialize)]
struct ChatCompletionsImageUrl {
    url: String,
}

#[derive(Serialize)]
struct ChatCompletionsToolCall<'a> {
    id: &'a str,
    #[serde(rename = "type")]
    kind: &'static str,
    function: ChatCompletionsFunctionCall<'a>,
}

#[derive(Serialize)]
struct ChatCompletionsFunctionCall<'a> {
    name: &'a str,
    arguments: &'a str,
}

#[derive(Serialize)]
struct ChatCompletionsTool<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    function: ChatCompletionsFunction<'a>,
}
impl<'a> From<&'a Tool> for ChatCompletionsTool<'a> {
    fn from(tool: &'a Tool) -> Self {
        ChatCompletionsTool {
            kind: "function",
            function: ChatCompletionsFunction {
                name: tool.name,
                description: &tool.description,
                parameters: JsonSchemaObject::from(tool),
            },
        }
    }
}

#[derive(Serialize)]
struct ChatCompletionsFunction<'a> {
    name: &'a str,
    description: &'a str,
    parameters: JsonSchemaObject<'a>,
}

#[derive(Deserialize)]
struct ChatCompletionsResponse {
    choices: Vec<ChatCompletionsChoice>,
    usage: Option<ChatCompletionsUsage>,
}
impl ChatCompletionsResponse {
    fn into_turn(self) -> Turn {
        let mut output = Vec::new();
        if let Some(choice) = self.choices.into_iter().next() {
            let message = choice.message;
            let (thought, content) = match message.content {
                Some(ChatCompletionsContent::Text(text)) => (None, Some(text)),
                Some(ChatCompletionsContent::Parts(parts)) => {
                    let mut thought = String::new();
                    let mut text = String::new();
                    for part in parts {
                        match part {
                            ChatCompletionsContentPart::Text { text: chunk } => {
                                text.push_str(&chunk)
                            }
                            ChatCompletionsContentPart::Thinking { thinking } => {
                                thought.extend(thinking.into_iter().filter_map(|part| match part {
                                    ChatCompletionsContentPart::Text { text } => Some(text),
                                    _ => None,
                                }))
                            }
                            ChatCompletionsContentPart::Other => {}
                        }
                    }
                    (Some(thought), Some(text))
                }
                None => (None, None),
            };
            let thought = match message.reasoning_content.or(message.reasoning) {
                Some(reasoning) => Some((reasoning, false)),
                None => thought.map(|thought| (thought, true)),
            };
            if let Some((thought, in_content)) = thought
                && !thought.is_empty()
            {
                output.push(TurnOutput::Thinking(Thinking::Standard(StandardThinking {
                    thought,
                    signature: None,
                    in_content,
                })));
            }
            if let Some(content) = content
                && !content.is_empty()
            {
                output.push(TurnOutput::Message(content));
            }
            output.extend(message.tool_calls.into_iter().flatten().map(|call| {
                TurnOutput::ToolCall(ToolCallRequest {
                    call_identifier: call.id,
                    tool_name: call.function.name,
                    arguments: call.function.arguments,
                })
            }));
        }

        let (usage, cost) = self.usage.map_or_else(Default::default, |usage| {
            let (cache_reads, cache_writes) =
                usage.prompt_tokens_details.map_or((0, 0), |details| {
                    (details.cached_tokens, details.cache_write_tokens)
                });
            let cost = usage
                .cost
                .map_or(TurnCost::None, |total_cost| TurnCost::Simple { total_cost });
            let usage = Usage {
                input_tokens: usage
                    .prompt_tokens
                    .saturating_sub(cache_reads.saturating_add(cache_writes)),
                output_tokens: usage.completion_tokens,
                cache_reads,
                cache_writes,
                long_cache_writes: 0,
            };
            (usage, cost)
        });

        Turn {
            remote: None,
            output,
            usage,
            cost,
        }
    }
}

#[derive(Deserialize)]
struct ChatCompletionsChoice {
    message: ChatCompletionsResponseMessage,
}

#[derive(Deserialize)]
struct ChatCompletionsResponseMessage {
    content: Option<ChatCompletionsContent>,
    /// DeepSeek, vLLM, llama.cpp.
    reasoning_content: Option<String>,
    /// OpenRouter, Ollama.
    reasoning: Option<String>,
    tool_calls: Option<Vec<ChatCompletionsResponseToolCall>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ChatCompletionsContent {
    Text(String),
    /// Mistral's reasoning models split thinking and text into typed chunks.
    Parts(Vec<ChatCompletionsContentPart>),
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum ChatCompletionsContentPart {
    Text {
        text: String,
    },
    Thinking {
        thinking: Vec<ChatCompletionsContentPart>,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct ChatCompletionsResponseToolCall {
    id: String,
    function: ChatCompletionsResponseFunctionCall,
}

#[derive(Deserialize)]
struct ChatCompletionsResponseFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct ChatCompletionsUsage {
    /// Includes cached tokens, read and written.
    prompt_tokens: u64,
    completion_tokens: u64,
    prompt_tokens_details: Option<ChatCompletionsPromptTokensDetails>,
    /// What the request cost in USD, from a server that says (OpenRouter).
    cost: Option<f64>,
}

#[derive(Deserialize)]
struct ChatCompletionsPromptTokensDetails {
    #[serde(default)]
    cached_tokens: u64,
    /// Tokens written to the cache, from a server that says (OpenRouter).
    #[serde(default)]
    cache_write_tokens: u64,
}
