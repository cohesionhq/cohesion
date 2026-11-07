use std::sync::Arc;

use cohesion_db::DatabaseContext;
use cohesion_js::ScriptEngine;

use super::agent_code_call::AgentCodeCall;
use super::agent_tool::{AgentTool, Seat};
use super::applied_effects::AppliedEffects;
use super::foreground_turn::ForegroundTurn;
use super::image_tray::ImageTray;
use super::installed_services::InstalledServices;
use crate::activity::Activity;
use crate::connected_services::ConnectedServices;
use crate::tool_definition::Tool;
use crate::tools::{ToolFuture, asks_done};
use crate::transcript_item::ToolCallStatus;

/// The code tool of an agent's conversation, its scripts mounting what the
/// turn's `AgentRole` gives, each run recorded in the agent's feed. A fork
/// of the conversation has the same, its runs marked with the fork's run.
pub(super) struct ForegroundCodeTool {
    pub engine: ScriptEngine,
    pub database: DatabaseContext,
    pub agent_id: i64,
    pub services: InstalledServices,
    pub connected_services: ConnectedServices,
    /// Where the images its scripts attach go, for `send_message`.
    pub images: ImageTray,
}

impl AgentTool for ForegroundCodeTool {
    fn definition(&self) -> Tool {
        AgentCodeCall::definition(&self.services, &self.connected_services)
    }

    fn activity(&self, arguments: &str) -> Option<Activity> {
        AgentCodeCall::activity(arguments)
    }

    fn globals(&self) -> Vec<&'static str> {
        AgentCodeCall::globals()
    }

    fn ends_turn(&self, arguments: &str) -> bool {
        asks_done(arguments)
    }

    fn call<'a>(&'a self, seat: &'a Seat, arguments: &'a str) -> ToolFuture<'a> {
        Box::pin(async move {
            let call = match AgentCodeCall::parse(arguments) {
                Ok(call) => call,
                Err(status) => return status,
            };
            // On its own task, so a run the user stops is recorded even when
            // the stop drops the call, as it does inside a client's request.
            let run = tokio::spawn(
                ScriptRun {
                    engine: self.engine.clone(),
                    database: self.database.clone(),
                    agent_id: self.agent_id,
                    // Its stop ends the scripts and their waits on actor
                    // sessions, and its trace is what their writes continue.
                    turn: seat.turn.clone(),
                    fork_run_id: seat.fork_run_id(),
                    images: self.images.clone(),
                }
                .run(call),
            );
            run.await
                .unwrap_or_else(|e| ToolCallStatus::Failure(format!("Error: {e}")))
        })
    }
}

/// What one script of the conversation's runs with, apart from the tool so
/// it can run on its own task.
struct ScriptRun {
    engine: ScriptEngine,
    database: DatabaseContext,
    agent_id: i64,
    turn: Arc<ForegroundTurn>,
    fork_run_id: Option<i64>,
    images: ImageTray,
}

impl ScriptRun {
    /// Runs `call`'s script as agent `agent_id`'s until `turn` is stopped,
    /// records it in the feed (marked with `fork_run_id` when a fork runs
    /// it), applies its effects, puts the images it attached in `images`,
    /// waits on its actor sessions and returns its output capped.
    async fn run(self, call: AgentCodeCall) -> ToolCallStatus {
        let ScriptRun {
            engine,
            database,
            agent_id,
            turn,
            fork_run_id,
            images,
        } = self;
        let ran = engine
            .traced_foreground_agent_run(
                agent_id,
                call.code.clone(),
                turn.script_stop(),
                turn.trace(),
            )
            .await;
        match ran {
            Ok((output, effects)) => {
                let message_id = call
                    .record(&database, agent_id, fork_run_id, Ok(&output.0))
                    .await;
                let (effects, attached) = effects.into_parts();
                AppliedEffects::foreground(&database, agent_id, turn.trace().initiative(), effects)
                    .await
                    .attached(&images, attached)
                    .tool_result(AgentCodeCall::capped(output.0, message_id))
            }
            Err(e) => {
                call.record(&database, agent_id, fork_run_id, Err(&e)).await;
                ToolCallStatus::Failure(e.to_string())
            }
        }
    }
}
