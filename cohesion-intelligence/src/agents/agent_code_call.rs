use cohesion_core::log::Module;
use cohesion_core::log_error;
use cohesion_core::model::{
    AgentCodeExecution, AgentCodeOutcome, AgentMessageContent, IntelligenceAgentMessageSender,
};
use cohesion_db::{CreateAgentMessageCommand, DatabaseContext};
use cohesion_js::script_api::{Load, modules};
use cohesion_js::{AgentRole, ScriptError};
use serde::Deserialize;

use super::agent_transcript::excerpt;
use super::documentation_tool::module_index;
use super::installed_services::InstalledServices;
use crate::activity::Activity;
use crate::connected_services::ConnectedServices;
use crate::tool_definition::{Field, FieldKind, Tool};
use crate::tools::done_field;
use crate::transcript_item::ToolCallStatus;

/// The code field's description.
const CODE: &str = "JavaScript to run against the user's Cohesion data, using the \
    API declared in this tool's description. Plain synchronous JavaScript: no \
    imports, require or async; reach the web with the blocking `network.fetch`. \
    The value of the last expression is returned as JSON, e.g. \
    `cohesion.workspaces().map(w => w.title)`. Return only what you \
    need: output past 32,000 characters is cut.";

/// How many characters of a script's output its tool result keeps (about 8K
/// tokens); `read_message` returns the rest from the run's
/// `codeExecution` message.
const OUTPUT_CHARS: usize = 32_000;

/// The arguments of an agent's code tool, a conversation's or a fork's.
#[derive(Deserialize)]
pub(super) struct AgentCodeCall {
    pub title: String,
    pub code: String,
}
impl AgentCodeCall {
    /// The globals the code tool's scripts reach, as every turn declares
    /// them ([`AgentRole::DOCUMENTED`]).
    pub(super) fn globals() -> Vec<&'static str> {
        AgentRole::DOCUMENTED
            .mounts()
            .iter()
            .filter_map(|mount| mount.global())
            .collect()
    }

    /// The code tool every session of an agent's conversation offers,
    /// declared as [`AgentRole::DOCUMENTED`] mounts, whoever started the
    /// turn, so the definition never differs between sessions. Its
    /// description declares, in brief, the modules loaded up front, indexes
    /// those the documentation tool loads and the installed services, then
    /// lists the user's connected services.
    pub(super) fn definition(
        services: &InstalledServices,
        connected_services: &ConnectedServices,
    ) -> Tool {
        let mounts = AgentRole::DOCUMENTED.mounts();
        let mounts = &*mounts;
        let upfront: Vec<_> = modules(mounts, Load::Upfront)
            .map(|module| module.brief_source())
            .collect();
        let mut description = format!("{}\n\n{}", upfront.join("\n\n"), module_index(mounts));
        for section in [services.section(), connected_services.section()] {
            if !section.is_empty() {
                description.push('\n');
                description.push_str(section);
            }
        }
        Tool {
            name: "code-tool",
            description: description.into(),
            fields: vec![
                Field {
                    name: "title",
                    description: "A few words saying what this script does, shown to the user, \
                        e.g. `Listing overdue tasks`.",
                    kind: FieldKind::String,
                    required: true,
                },
                Field {
                    name: "code",
                    description: CODE,
                    kind: FieldKind::String,
                    required: true,
                },
                done_field(),
            ],
        }
    }

    /// What a call with `arguments` shows while it runs: its script's
    /// title, when the arguments carry one.
    pub(super) fn activity(arguments: &str) -> Option<Activity> {
        Self::parse(arguments)
            .ok()
            .map(|call| Activity::Running(call.title))
    }

    /// The model's arguments, or the failure to hand back to it.
    pub(super) fn parse(arguments: &str) -> Result<Self, ToolCallStatus> {
        serde_json::from_str(arguments).map_err(|e| ToolCallStatus::Failure(e.to_string()))
    }

    /// Stores a conversation's or a fork's run as a `codeExecution` message
    /// in the agent's feed, marked with the fork's `run_id`, and returns the
    /// message's id, or none when it could not be stored.
    pub(super) async fn record(
        self,
        database: &DatabaseContext,
        agent_id: i64,
        run_id: Option<i64>,
        outcome: Result<&str, &ScriptError>,
    ) -> Option<i64> {
        let outcome = match outcome {
            Ok(output) => AgentCodeOutcome::Succeeded {
                result: output.to_string(),
            },
            Err(ScriptError::Stopped) => AgentCodeOutcome::Cancelled,
            Err(e) => AgentCodeOutcome::Failed {
                error: e.to_string(),
            },
        };
        let execution = AgentCodeExecution {
            title: self.title,
            code: self.code,
            outcome,
        };
        match database
            .send(CreateAgentMessageCommand {
                agent_id,
                run_id,
                run_description: None,
                sender: IntelligenceAgentMessageSender::Agent,
                content: AgentMessageContent::CodeExecution(execution),
                reply_to: None,
            })
            .await
        {
            Ok(message) => Some(message.id),
            Err(e) => {
                log_error!(
                    Module::Intelligence,
                    "Agent {agent_id}: could not record a code execution: {e:?}"
                );
                None
            }
        }
    }

    /// `output`, the script's, as its tool result keeps it: its first
    /// [`OUTPUT_CHARS`] and a note of how to read the rest from `message_id`,
    /// the run's recorded message. Whole when the run was not recorded, as
    /// nothing could read the rest.
    pub(super) fn capped(output: String, message_id: Option<i64>) -> String {
        match message_id {
            Some(id) => excerpt(&output, id, OUTPUT_CHARS),
            None => output,
        }
    }
}

#[cfg(test)]
mod tests;
