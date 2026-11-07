//! `Agent` in scripts.

use cohesion_core::model::IntelligenceAgent;
use cohesion_db::SetAgentTeamCommand;
use cohesion_macros::js_entity;
use rquickjs::Ctx;

/// An intelligence agent, as `cohesion.agents()` and `cohesion.agent(id)`
/// return it, with the name, color, emoji, personality, verbosity, role and
/// directive the user gave it, and the team it is on (`teamId`).
///
/// An `AgentMessage`'s `agentId` names the agent that sent it, so
/// `onAgentMessage` can look up who is talking.
///
/// Link one in a message as `[Scout](cohesion://agent/3)`, its name and `id`;
/// the user opens it with a tap.
#[js_entity(IntelligenceAgent, js = "Agent")]
impl JsAgent {
    /// Puts the agent on team `teamId`, or on none with `null`; its pin
    /// stays as it was. Throws if no team has that id. Not available in
    /// read-only contexts.
    #[qjs(rename = "setTeam")]
    pub fn set_team<'js>(&mut self, ctx: Ctx<'js>, team_id: Option<i64>) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            SetAgentTeamCommand {
                agent_id: self.inner.id,
                team_id,
            },
        )?;
        Ok(())
    }
}
