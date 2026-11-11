use cohesion_core::{
    FromRequest,
    invalidation::Invalidation,
    model::{AgentSkill, AgentSkillScope},
    protocol::{
        CreateAgentSkillRequest, DeleteAgentSkillRequest, ListAgentSkillsRequest,
        UpdateAgentSkillRequest,
    },
};

use crate::changed::{Changed, Invalidates};
use crate::{
    AgentSettings, CommandContext, DatabaseCommand, DatabaseCommandResult, DatabaseError, Filtered,
    Found, Read, ReadableDatabase, ValidationError, Write,
};

/// A skill's name, description and prompt as they are stored: trimmed, and
/// refused when the name or the prompt is empty.
struct SkillText {
    name: String,
    description: String,
    prompt: String,
}

impl SkillText {
    fn new(name: &str, description: &str, prompt: &str) -> Result<Self, ValidationError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(ValidationError::in_field(
                "name",
                "A skill needs a name.".to_string(),
            ));
        }
        let prompt = prompt.trim();
        if prompt.is_empty() {
            return Err(ValidationError::in_field(
                "prompt",
                "A skill needs a prompt.".to_string(),
            ));
        }
        Ok(Self {
            name: name.to_string(),
            description: description.trim().to_string(),
            prompt: prompt.to_string(),
        })
    }
}

/// A skill written, or a refusal naming the name already taken in its
/// scope.
fn named(
    written: Result<Option<AgentSkill>, DatabaseError>,
    name: &str,
) -> DatabaseCommandResult<AgentSkill> {
    match written {
        Err(error) if error.is_unique_violation() => Err(ValidationError::in_field(
            "name",
            format!("There's already a skill named “{name}” here."),
        )
        .into()),
        written => written.found(),
    }
}

/// Every skill, each agent's and each team's, by name.
#[derive(FromRequest)]
#[request(ListAgentSkillsRequest)]
pub struct ListAgentSkillsCommand {}
impl DatabaseCommand for ListAgentSkillsCommand {
    type Response = Vec<AgentSkill>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx.db.select().order("name, id").all()?)
    }
}

/// The skills agent `agent_id` has, by name: its own, and its team's whose
/// names none of its own take, since its own skill wins.
pub struct SkillsForAgentCommand {
    pub agent_id: i64,
}
impl DatabaseCommand for SkillsForAgentCommand {
    type Response = Vec<AgentSkill>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx
            .db
            .select()
            .filter(
                "agent_id = ? OR (team_id = (SELECT team_id FROM intelligence_agents WHERE id = ?) \
                 AND name NOT IN (SELECT name FROM agent_skills WHERE agent_id = ?))",
            )
            .bind(self.agent_id)
            .bind(self.agent_id)
            .bind(self.agent_id)
            .order("name, id")
            .all()?)
    }
}

/// Saves a skill for one agent or for every agent on a team.
#[derive(FromRequest)]
#[request(CreateAgentSkillRequest)]
pub struct CreateAgentSkillCommand {
    pub scope: AgentSkillScope,
    pub name: String,
    pub description: String,
    pub prompt: String,
    pub agent_can_use: bool,
}
impl DatabaseCommand for CreateAgentSkillCommand {
    type Response = AgentSkill;
    type Access = Write<AgentSettings>;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Write<AgentSettings>>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let text = SkillText::new(&self.name, &self.description, &self.prompt)?;
        let written = ctx
            .db
            .insert::<AgentSkill>()
            .set_group(self.scope)
            .set(AgentSkill::NAME, text.name.clone())
            .set(AgentSkill::DESCRIPTION, text.description)
            .set(AgentSkill::PROMPT, text.prompt)
            .set(AgentSkill::AGENT_CAN_USE, self.agent_can_use)
            .returning_one();
        Ok(named(written, &text.name)?.invalidating([Invalidation::AgentSkills]))
    }
}

/// Changes a skill's name, description, prompt and whether the agent may
/// use it; its scope stays.
#[derive(FromRequest)]
#[request(UpdateAgentSkillRequest)]
pub struct UpdateAgentSkillCommand {
    pub skill_id: i64,
    pub name: String,
    pub description: String,
    pub prompt: String,
    pub agent_can_use: bool,
}
impl DatabaseCommand for UpdateAgentSkillCommand {
    type Response = AgentSkill;
    type Access = Write<AgentSettings>;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Write<AgentSettings>>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let text = SkillText::new(&self.name, &self.description, &self.prompt)?;
        let written = ctx
            .db
            .update::<AgentSkill>()
            .set(AgentSkill::NAME, text.name.clone())
            .set(AgentSkill::DESCRIPTION, text.description)
            .set(AgentSkill::PROMPT, text.prompt)
            .set(AgentSkill::AGENT_CAN_USE, self.agent_can_use)
            .filter_eq(AgentSkill::ID, self.skill_id)
            .returning_one();
        Ok(named(written, &text.name)?.invalidating([Invalidation::AgentSkills]))
    }
}

/// Deletes a skill, returning it. Messages that ran it keep its prompt.
#[derive(FromRequest)]
#[request(DeleteAgentSkillRequest)]
pub struct DeleteAgentSkillCommand {
    pub skill_id: i64,
}
impl DatabaseCommand for DeleteAgentSkillCommand {
    type Response = AgentSkill;
    type Access = Write<AgentSettings>;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Write<AgentSettings>>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let skill: AgentSkill = ctx
            .db
            .delete()
            .filter_eq(AgentSkill::ID, self.skill_id)
            .returning_one()
            .found()?;
        Ok(skill.invalidating([Invalidation::AgentSkills]))
    }
}

#[cfg(test)]
mod tests;
