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

use std::sync::Arc;

use cohesion_core::model::{CacheLength, CacheLifetime, Dialect, EndpointBackend};

use crate::account_credentials::AccountCredentials;
use crate::intelligence_backend::IntelligenceBackend;
use anthropic_messages::MessagesBackend;
use endpoint_key::EndpointKey;
use openai_chat_completions::ChatCompletionsBackend;
use openai_responses::{ResponsesBackend, ResponsesPreset};

/// The built-in backend serving `backend`'s endpoint: speaking its dialect
/// to its base URL, with its API key or, for an account's endpoint, the
/// credential `credentials` gives. `cache_key` names the requests of a
/// Responses backend that share a prefix. Messages and Responses backends
/// ask for the endpoint's long cache where a transcript's plan says so (a
/// Responses server keeps one cache per request, so its every request asks
/// for it). Chat completions offer no choice of cache.
pub fn backend_for(
    backend: EndpointBackend,
    credentials: Arc<dyn AccountCredentials>,
    model_identifier: String,
    cache_key: String,
) -> Box<dyn IntelligenceBackend> {
    let long = backend.cache_lifetime(CacheLength::Long);
    let (dialect, base_url, key) = match backend {
        EndpointBackend::Integrated {
            dialect,
            base_url,
            api_key,
            ..
        } => (dialect, base_url, EndpointKey::Stored(api_key)),
        EndpointBackend::Account {
            dialect,
            base_url,
            account_id,
            ..
        } => (
            dialect,
            base_url,
            EndpointKey::Account {
                account_id,
                credentials,
            },
        ),
    };
    let preset = match dialect {
        Dialect::ChatCompletions => {
            return Box::new(ChatCompletionsBackend::new(base_url, key, model_identifier));
        }
        Dialect::Messages => {
            return Box::new(MessagesBackend::new(base_url, key, model_identifier, long));
        }
        Dialect::Responses => ResponsesPreset::Stored,
        Dialect::StatelessResponses => ResponsesPreset::Stateless,
    };
    Box::new(ResponsesBackend::new(
        base_url,
        key,
        model_identifier,
        cache_key,
        preset,
        long,
    ))
}

/// The `ttl` a request names to ask for `long`, the endpoint's long cache,
/// or none when the endpoint offers only its short one.
fn long_ttl(long: CacheLifetime) -> Option<String> {
    match long {
        CacheLifetime::Short { .. } => None,
        CacheLifetime::Long(long) => Some(long.ttl()),
    }
}
