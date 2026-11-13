//! The wire protocol: request and frame envelopes, errors, every command
//! and reply (with their request types, grouped by domain in the
//! `*_requests` modules), what clients offer and are asked (`capabilities`),
//! and how a server context carries each command out.

mod activity_requests;
mod actor_requests;
mod agent_requests;
mod article_requests;
mod calendar_requests;
mod call;
mod capabilities;
mod client_requests;
mod command;
mod connector_requests;
mod contact_requests;
mod document_requests;
mod document_table_requests;
mod embed_requests;
mod envelope;
mod experiment_requests;
mod extension_requests;
mod feed_requests;
mod intelligence_requests;
mod mermaid_requests;
mod profile_requests;
mod reply_data;
mod rewind_requests;
mod route;
mod route_kinds;
mod script_requests;
mod security_requests;
mod server_error;
mod server_requests;
mod storage_requests;
mod tag_requests;
mod task_requests;
mod vocabulary;
mod workspace_requests;

pub use activity_requests::*;
pub use actor_requests::*;
pub use agent_requests::*;
pub use article_requests::*;
pub use calendar_requests::*;
pub use call::{Call, ReplyPayload};
pub use capabilities::{
    ActorCall, ActorSessionReport, ActorSessionUpdate, ActorUsage, ActorUsageUpdate, ClientCall,
    ClientCapabilities, ClientImage, ClientOutcome, ClientResult, EvictionReason, PlanUsage,
    SubagentLaunch, UsagePool,
};
pub use client_requests::*;
pub use command::Command;
pub use connector_requests::*;
pub use contact_requests::*;
pub use document_requests::*;
pub use document_table_requests::*;
pub use embed_requests::*;
pub use envelope::{
    CallResult, ClientFrame, ClientHello, Frame, Hello, Outcome, PROTOCOL_VERSION, RegisterRequest,
    Reply, Request, ServerMessage, Subscribe,
};
pub use experiment_requests::*;
pub use extension_requests::*;
pub use feed_requests::*;
pub use intelligence_requests::*;
pub use mermaid_requests::*;
pub use profile_requests::*;
pub use reply_data::ReplyData;
pub use rewind_requests::*;
pub use route::Route;
pub use route_kinds::{Database, FromClient, Intelligence, Local, Script};
pub use script_requests::*;
pub use security_requests::*;
pub use server_error::{ErrorCode, ServerError, ShuttingDown};
pub use server_requests::*;
pub use storage_requests::*;
pub use tag_requests::*;
pub use task_requests::*;
pub use vocabulary::{
    Described, HookWording, Labeled, ProviderWording, Vocabulary, actor_session_stop_label,
};
pub use workspace_requests::*;
