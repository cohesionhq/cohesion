//! LLM conversations: a backend per wire format (OpenAI Responses, stored or
//! stateless, Anthropic Messages and OpenAI-compatible chat completions, an
//! endpoint that signs in with an account getting its credential from the
//! daemon through [`AccountCredentials`]) in `providers/`, the
//! models.dev catalog of models and prices in `catalog/`, and `Session`
//! (`session/`), which runs a thread's turns against the database and
//! answers tool calls with whichever `SessionTool`s (`tools/`) it was built
//! with, on the endpoint model its caller resolved (a tier's default, via
//! `session::tier_model`). Every agent code tool's description is the core
//! of the script API in brief, an index of the modules its documentation tool loads
//! on demand, an index of the installed services its scripts call through
//! `services.use` (whose declarations the documentation tool also hands
//! out), and `ConnectedServices`, the accounts the user added, both loaded
//! when its session is built.
//!
//! `agents/` runs persistent agents: an [`AgentCoordinator`] owns one host
//! per agent for its foreground conversation, on the `agent` tier's model,
//! and starts forks, on the tier their cue's effort names,
//! when one of an agent's cues fires on a hook, and forks of a conversation
//! as the actor sessions it handed work stop working.
//!
//! Forks are held to the daemon's spend limits: `SpendLimits`
//! checks them before each of its model calls (a `Session` built with a
//! `SpendCap`) and tells the agent once when one holds it back; the user's
//! own turns are never held back.
//!
//! A `Transcript` is what every backend reads: `TranscriptItem`s (prompts,
//! `Thinking`, tool calls and their results) plus the `Tool`s on offer
//! (each `SessionTool::definition`). A backend answers with a `Turn`, whose
//! `TurnCost` is what the provider reported it cost; when it reported
//! nothing, the turn's `Usage` is priced at the endpoint model's rates,
//! which an imported model reads from the catalog.
//!
//! Only what the daemon uses is exported; everything else is reached by
//! its module path inside the crate.

mod account_credentials;
mod activity;
mod agents;
mod catalog;
mod connected_services;
mod intelligence_backend;
mod intelligence_error;
mod providers;
mod session;
mod spend_limits;
mod system_prompt;
#[cfg(test)]
mod test_model;
mod thinking;
mod tool_definition;
mod tools;
mod transcript;
mod transcript_item;
mod turn;
mod turn_input;
mod usage;

pub use account_credentials::{AccountCredentials, CredentialFuture};
pub use agents::{AgentCoordinator, AgentError};
pub use catalog::ModelCatalog;
pub use intelligence_error::IntelligenceError;
pub use session::SessionConstructionError;
