//! The QuickJS script runtime: scripts run on a pool of worker threads with
//! the database exposed through the `cohesion` host namespace. Each sandbox
//! holds a `GuardedDatabaseContext`, so whether its scripts may write is
//! fixed when it is built; only `storage`, an extension's or agent's own
//! key/value store, is writable either way.
//!
//! A [`ScriptEngine`] schedules jobs onto workers, each of which runs them in a
//! [`Sandbox`]. What scripts see is the [`globals`] (`cohesion`,
//! `intelligence`, an extension's `accounts`, ...) and the `entities` they
//! hand out, all listed once in
//! [`script_api`], which also yields the `.d.ts` in
//! [`TS_DECLARATIONS`](script_api::TS_DECLARATIONS). What the `services`
//! global holds depends on the extensions installed, so
//! [`service_declarations`] renders its declarations at runtime.
//!
//! Agents run on pools of their own, and an [`AgentRole`] alone decides
//! what each script mounts and may write: a turn of an agent's conversation
//! (or a fork of it) gets a read-write `cohesion`, `network` and `agent` when
//! the user started its chain, and only reads when an event did; a cue
//! deciding whether to wake only reads ([`CueEvaluation`]). The `agent`
//! global only records [`ForegroundEffect`]s, which the run hands back to the
//! caller. Turns get `connectors`, which reaches connector extensions
//! through the [`ConnectorAccess`] the daemon gives the engine, and
//! `actors`, which starts and talks to actor sessions through its
//! [`ActorAccess`], as extensions granted `actors` do.
//! Every pool, and extensions granted `services`, get `services`, which
//! calls installed services through the engine's [`ServiceAccess`]; a
//! script with read-only database access, cues included, only their
//! read-only functions.
//!
//! Drawing a Mermaid diagram isn't a script at all: [`ScriptEngine::render_mermaid`]
//! runs Pele, in a runtime with nothing mounted, on workers of its own.
//! Neither is a database formula: [`ScriptEngine::evaluate_formula`] calls a
//! property's function with each row's values, likewise with nothing
//! mounted, on workers that keep the formulas they compiled.
//!
//! Fetching an article's page is `cohesion_internet::article_snapshot`; a
//! sandbox that mounts `ArticleFetch` lets its scripts call it through
//! `Article.resnapshot`.

// First, so its `macro_rules!` are in scope in every module below.
#[macro_use]
mod js_macros;

mod actor_access;
mod connector_access;
mod conversion;
mod engine;
mod entities;
mod formula;
pub mod globals;
mod mermaid;
mod sandbox;
pub mod script_api;
mod script_error;
mod script_output;
mod script_stop;
mod service_access;
mod service_declarations;
mod traced_command;

pub use actor_access::ActorAccess;
pub use connector_access::ConnectorAccess;
pub use engine::{AgentRole, CueEvaluation, GENERAL_PURPOSE_MOUNTS, ScopeHandle, ScriptEngine};
pub use formula::{Formula, FormulaOutput, FormulaRow};
pub use globals::{Effects, ForegroundEffect};
pub use sandbox::{Sandbox, SandboxOptions, ScriptArgument};
pub use script_error::ScriptError;
pub use script_output::ScriptOutput;
pub use script_stop::ScriptStop;
pub use service_access::{ServiceAccess, ServiceCall};
pub use service_declarations::service_declarations;

#[cfg(test)]
mod tests;
