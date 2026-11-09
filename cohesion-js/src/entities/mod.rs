//! Script-facing wrappers around the models. Each entity is an impl block
//! marked [`js_entity`](cohesion_macros::js_entity), which generates the class
//! struct, a getter per model column, `toJSON` and its TypeScript declaration,
//! so only methods are written by hand. Options objects that methods take are
//! structs deriving [`JsOptions`](cohesion_macros::JsOptions), read through
//! [`FromJsField`](crate::conversion::FromJsField).

pub mod account;
pub mod actor;
pub mod actor_session;
pub mod agent;
pub mod agent_team;
pub mod article;
pub mod connector;
pub mod connector_account;
pub mod connector_account_options;
pub mod contact;
pub mod document;
pub mod document_folder;
pub mod endpoint_model;
pub mod feed;
pub mod intelligence_endpoint;
pub mod resource;
pub mod rewind;
pub mod search_result;
pub mod tag;
pub mod task;
pub mod task_folder;
pub mod usage;
pub mod workspace;
pub mod workspace_options;

/// How many characters `excerpt()` returns; the daemon decides, scripts don't
/// choose.
const EXCERPT_CHARS: usize = 500;

/// The first [`EXCERPT_CHARS`] characters of `text`, with `…` after them when
/// the text was longer.
fn excerpt(text: &str) -> String {
    match text.char_indices().nth(EXCERPT_CHARS) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_string(),
    }
}
