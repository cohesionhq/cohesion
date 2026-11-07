//! `Team` in scripts.

use cohesion_core::model::IntelligenceAgentTeam;
use cohesion_db::{DeleteAgentTeamCommand, ListIntelligenceAgentsCommand, RenameAgentTeamCommand};
use cohesion_macros::js_entity;
use rquickjs::Ctx;

use super::agent::JsAgent;

/// A team of agents: a named group whose members see each other under
/// `# Your team` in their prompts and share the team's memories.
///
/// Get them from `cohesion.teams()` or `cohesion.team(id)`, make one with
/// `cohesion.createTeam(title)`, and put an agent on one with
/// `Agent.setTeam(teamId)`. An agent is on at most one team; pinning an agent
/// is apart from its team.
#[js_entity(IntelligenceAgentTeam, js = "Team")]
impl JsTeam {
    /// The agents on the team, as `cohesion.agents()` orders them.
    pub fn agents<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Vec<JsAgent>> {
        let agents = self.read(&ctx, ListIntelligenceAgentsCommand {})?;
        Ok(agents
            .into_iter()
            .filter(|agent| agent.team_id == Some(self.inner.id))
            .map(|agent| JsAgent::new(agent, self.db.clone()))
            .collect())
    }

    /// Changes the title, which is unique among teams, ignoring case. An
    /// empty or taken title throws. Not available in read-only contexts.
    pub fn rename<'js>(&mut self, ctx: Ctx<'js>, title: String) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            RenameAgentTeamCommand {
                team_id: self.inner.id,
                title,
            },
        )?;
        Ok(())
    }

    /// Deletes the team: its agents stay, on no team, and its memories are
    /// forgotten. Not available in read-only contexts.
    pub fn delete<'js>(&mut self, ctx: Ctx<'js>) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            DeleteAgentTeamCommand {
                team_id: self.inner.id,
            },
        )?;
        Ok(())
    }
}
