//! One `IntelligenceBackend` per wire format:
//! OpenAI-compatible chat completions, the Responses API (OpenAI, xAI) and
//! Anthropic Messages. Each sends its turn through `post_json`, which maps failure
//! statuses to `IntelligenceError`.

mod anthropic_messages;
mod endpoint_key;
mod json_schema;
mod openai_chat_completions;
mod openai_responses;
mod post_json;

pub use anthropic_messages::MessagesBackend;
pub use endpoint_key::EndpointKey;
pub use openai_chat_completions::ChatCompletionsBackend;
pub use openai_responses::{ResponsesBackend, ResponsesPreset};
