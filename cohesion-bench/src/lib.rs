//! Cohesion bench: scores language models on Cohesion's own agent, its
//! tools and its script API, so a change to a tool's definition or the API
//! can be measured, and models compared.
//!
//! Every run of a case gets a scratch daemon: the `CohesionContext`
//! cohesiond serves (cohesionkit, so the agents, tools, system prompt,
//! script engine and providers of cohesion-intelligence and cohesion-js,
//! unchanged), on a database of its own. The case's setup fills it, the
//! user's message goes to a new agent on the model under test through the
//! same request a client sends, and once the turn and everything it set
//! going has ended, the case's check judges the database and the reply.
//! What the run cost and did is read back from what Cohesion recorded: its
//! cost events and its threads' tool calls. Nothing is mocked, so the bench
//! measures whatever Cohesion is at the commit it is built from.
//!
//! The bench is a tool in the workspace that nothing depends on; cohesiond
//! doesn't link it.

pub mod case;
pub mod metrics;
pub mod model_spec;
pub mod prices;
pub mod report;
pub mod run;
pub mod scratch;
pub mod validate;

#[cfg(test)]
mod tests;
