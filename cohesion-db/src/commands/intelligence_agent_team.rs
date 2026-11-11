use cohesion_core::FromRequest;
use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{IntelligenceAgent, IntelligenceAgentTeam};
use cohesion_core::protocol::{
    CreateAgentTeamRequest, DeleteAgentTeamRequest, ListAgentTeamsRequest, RenameAgentTeamRequest,
    SetAgentTeamRequest,
};

use crate::changed::{Changed, Invalidates};
use crate::commands::title::ensure_title;
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, DatabaseError, Filtered, Found, Read,
    ReadableDatabase, ValidationError, Write,
};

/// A team written, or a refusal naming the title already taken.
fn titled(
    written: Result<Option<IntelligenceAgentTeam>, DatabaseError>,
    title: &str,
) -> DatabaseCommandResult<IntelligenceAgentTeam> {
    match written {
        Err(error) if error.is_unique_violation() => Err(ValidationError::in_field(
            "title",
            format!("There's already a team named “{title}”."),
        )
        .into()),
        written => written.found(),
    }
}

/// Every team, by title.
#[derive(FromRequest)]
#[request(ListAgentTeamsRequest)]
pub struct ListAgentTeamsCommand {}
impl DatabaseCommand for ListAgentTeamsCommand {
    type Response = Vec<IntelligenceAgentTeam>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx.db.select().order("title, id").all()?)
    }
}

/// A team by id.
pub struct GetAgentTeamCommand {
    pub team_id: i64,
}
impl DatabaseCommand for GetAgentTeamCommand {
    type Response = IntelligenceAgentTeam;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        ctx.db
            .select()
            .filter_eq(IntelligenceAgentTeam::ID, self.team_id)
            .one()
            .found()
    }
}

/// Creates a team, with no agents on it yet.
#[derive(FromRequest)]
#[request(CreateAgentTeamRequest)]
pub struct CreateAgentTeamCommand {
    pub title: String,
}
impl DatabaseCommand for CreateAgentTeamCommand {
    type Response = IntelligenceAgentTeam;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let title = self.title.trim();
        ensure_title(title)?;
        let written = ctx
            .db
            .insert::<IntelligenceAgentTeam>()
            .set(IntelligenceAgentTeam::TITLE, title.to_string())
            .returning_one();
        Ok(titled(written, title)?.invalidating([Invalidation::AgentTeams]))
    }
}

/// Changes a team's title.
#[derive(FromRequest)]
#[request(RenameAgentTeamRequest)]
pub struct RenameAgentTeamCommand {
    pub team_id: i64,
    pub title: String,
}
impl DatabaseCommand for RenameAgentTeamCommand {
    type Response = IntelligenceAgentTeam;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let title = self.title.trim();
        ensure_title(title)?;
        let written = ctx
            .db
            .update::<IntelligenceAgentTeam>()
            .set(IntelligenceAgentTeam::TITLE, title.to_string())
            .filter_eq(IntelligenceAgentTeam::ID, self.team_id)
            .returning_one();
        Ok(titled(written, title)?.invalidating([Invalidation::AgentTeams]))
    }
}

/// Deletes a team. Its agents stay, on no team, and its memories and
/// skills go with it.
#[derive(FromRequest)]
#[request(DeleteAgentTeamRequest)]
pub struct DeleteAgentTeamCommand {
    pub team_id: i64,
}
impl DatabaseCommand for DeleteAgentTeamCommand {
    type Response = IntelligenceAgentTeam;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let members: Vec<i64> = ctx
            .db
            .select_column::<IntelligenceAgent, i64>("id")
            .filter_eq(IntelligenceAgent::TEAM_ID, self.team_id)
            .all()?;
        let team: IntelligenceAgentTeam = ctx
            .db
            .delete()
            .filter_eq(IntelligenceAgentTeam::ID, self.team_id)
            .returning_one()
            .found()?;
        let keys = [
            Invalidation::AgentTeams,
            Invalidation::Memories,
            Invalidation::AgentSkills,
        ]
        .into_iter()
        .chain(members.into_iter().map(Invalidation::Agent));
        Ok(team.invalidating(keys))
    }
}

/// Puts an agent on a team, or on none when `team_id` is `None`. Its pin
/// is left as it was.
#[derive(FromRequest)]
#[request(SetAgentTeamRequest)]
pub struct SetAgentTeamCommand {
    pub agent_id: i64,
    pub team_id: Option<i64>,
}
impl DatabaseCommand for SetAgentTeamCommand {
    type Response = IntelligenceAgent;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let agent: IntelligenceAgent = ctx
            .db
            .update()
            .set(IntelligenceAgent::TEAM_ID, self.team_id)
            .filter_eq(IntelligenceAgent::ID, self.agent_id)
            .returning_one()
            .found()?;
        let key = Invalidation::Agent(agent.id);
        Ok(agent.invalidating([key]))
    }
}

#[cfg(test)]
mod tests;
