//! `use_skill`: runs one of the skills the user saved for the agent, or for
//! its team, that the agent may use itself. Every thread has it, a turn an
//! event started included: a skill is only a prompt, so it grants nothing
//! the thread couldn't do already.

use cohesion_core::log::Module;
use cohesion_core::log_error;
use cohesion_core::model::{AgentMessageContent, AgentSkill, IntelligenceAgentMessageSender};
use cohesion_db::{CreateAgentMessageCommand, DatabaseContext};
use serde::Deserialize;

use super::agent_tool::{AgentTool, Seat};
use super::conversation_session::usable_skills;
use crate::tool_definition::{Field, FieldKind, Tool};
use crate::tools::ToolFuture;
use crate::transcript_item::ToolCallStatus;

#[derive(Deserialize)]
struct UseSkillCall {
    name: String,
}

/// Hands the agent the prompt of one of its skills, by name, and shows that
/// it used it in the feed, marked with the fork's run when a fork does.
pub(super) struct UseSkillTool {
    pub database: DatabaseContext,
    pub agent_id: i64,
}

impl AgentTool for UseSkillTool {
    fn definition(&self) -> Tool {
        Tool {
            name: "use_skill",
            description: "Runs one of your skills: instructions the user saved for a task \
                they ask for often. Returns the skill's prompt, which you then carry out as if \
                the user had sent it. Your skills are listed in your status, with any change \
                since under `# Skills changed`. Use one when the user asks for it or the work \
                matches its description."
                .into(),
            fields: vec![Field {
                name: "name",
                description: "The skill's name, as listed.",
                kind: FieldKind::String,
                required: true,
            }],
        }
    }

    fn call<'a>(&'a self, seat: &'a Seat, arguments: &'a str) -> ToolFuture<'a> {
        Box::pin(async move {
            let call: UseSkillCall = match serde_json::from_str(arguments) {
                Ok(call) => call,
                Err(e) => return ToolCallStatus::Failure(e.to_string()),
            };
            let skills = match usable_skills(&self.database, self.agent_id).await {
                Ok(skills) => skills,
                Err(e) => return ToolCallStatus::Failure(format!("could not read skills: {e:?}")),
            };
            let Some(skill) = find(&skills, &call.name) else {
                return ToolCallStatus::Failure(unknown(&skills, &call.name));
            };
            post(&self.database, self.agent_id, seat.fork_run_id(), skill).await;
            ToolCallStatus::Success(format!(
                "Skill \"{}\":\n{}\n\nCarry it out now.",
                skill.name, skill.prompt
            ))
        })
    }
}

/// The skill named `name`, ignoring case and the spaces around it.
fn find<'a>(skills: &'a [AgentSkill], name: &str) -> Option<&'a AgentSkill> {
    let name = name.trim();
    skills
        .iter()
        .find(|skill| skill.name.eq_ignore_ascii_case(name))
}

/// Why no skill named `name` ran, naming the ones the agent has.
fn unknown(skills: &[AgentSkill], name: &str) -> String {
    match skills {
        [] => "You have no skills to use.".to_string(),
        skills => {
            let names = skills
                .iter()
                .map(|skill| format!("\"{}\"", skill.name))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "You have no skill named \"{}\". Your skills: {names}.",
                name.trim()
            )
        }
    }
}

/// Posts a `usedSkill` line naming `skill` to the agent's feed, marked with
/// `run_id` when a fork used it. The prompt is already returned, so a
/// failure here is only logged.
async fn post(database: &DatabaseContext, agent_id: i64, run_id: Option<i64>, skill: &AgentSkill) {
    let result = database
        .send(CreateAgentMessageCommand {
            agent_id,
            run_id,
            run_description: None,
            sender: IntelligenceAgentMessageSender::Agent,
            content: AgentMessageContent::UsedSkill {
                name: skill.name.clone(),
            },
            reply_to: None,
        })
        .await;
    if let Err(e) = result {
        log_error!(
            Module::Intelligence,
            "Agent {agent_id}: could not post the skill it used: {e:?}"
        );
    }
}

#[cfg(test)]
mod tests;
