//! Scheduling scripts: a [`ScriptEngine`] owns pools of worker threads and
//! hands each job to the least busy worker of its pool.

mod agent_role;
mod cue_evaluation;
mod hosts;
mod mermaid_pool;
mod pressure_guard;
mod scope_handle;
mod script_engine;
mod script_job;
mod script_worker;

pub use agent_role::AgentRole;
pub use cue_evaluation::CueEvaluation;
pub(crate) use hosts::Hosts;
pub use scope_handle::ScopeHandle;
pub use script_engine::{GENERAL_PURPOSE_MOUNTS, ScriptEngine};
