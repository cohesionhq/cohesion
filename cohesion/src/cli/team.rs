use clap::Subcommand;
use cohesion_core::model::IntelligenceAgentTeam;
use cohesion_core::protocol::{
    CreateAgentTeamRequest, DeleteAgentTeamRequest, ListAgentTeamsRequest,
    ListIntelligenceAgentsRequest, RenameAgentTeamRequest,
};

use super::{table, terminal_io::confirmed};
use crate::{cli_context::Context, cli_error::CliError, cli_macros::row, name_lookup::find};

#[derive(Subcommand)]
pub enum TeamCommand {
    /// Create a team; put agents on it with `cohesion agent update <agent> --team <team>`.
    Create {
        /// What to call the team.
        title: String,
    },
    /// Rename a team.
    Rename {
        /// Team id or title (titles match case-insensitively).
        team: String,
        /// What to call it now.
        title: String,
    },
    /// Delete a team, after asking for confirmation. The team's agents won't be
    /// deleted, but its memories will.
    Delete {
        /// Team id or title.
        team: String,
        /// Don't ask for confirmation.
        #[arg(short = 'y', long)]
        yes: bool,
    },
}

pub async fn run(ctx: &Context, verb: Option<TeamCommand>) -> Result<(), CliError> {
    match verb {
        None => list(ctx).await,
        Some(TeamCommand::Create { title }) => create(ctx, title).await,
        Some(TeamCommand::Rename { team, title }) => rename(ctx, team, title).await,
        Some(TeamCommand::Delete { team, yes }) => delete(ctx, team, yes).await,
    }
}

/// The team whose id or title is `query`.
pub async fn find_team(ctx: &Context, query: &str) -> Result<IntelligenceAgentTeam, CliError> {
    let teams = ctx.call(ListAgentTeamsRequest).await?;
    find(&teams, query).cloned()
}

/// Teams with the names of the agents on them.
async fn list(ctx: &Context) -> Result<(), CliError> {
    let teams = ctx.call(ListAgentTeamsRequest).await?;
    let agents = ctx.call(ListIntelligenceAgentsRequest).await?;
    table::list(
        &teams,
        "No teams.\nCreate one with `cohesion team create <title>`.",
        |team| {
            let members = agents
                .iter()
                .filter(|agent| agent.team_id == Some(team.id))
                .map(|agent| agent.agent_name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            row![>team.id, team.title, members]
        },
    );
    Ok(())
}

async fn create(ctx: &Context, title: String) -> Result<(), CliError> {
    let team = ctx.call(CreateAgentTeamRequest { title }).await?;
    println!("Created {}.", describe(&team));
    Ok(())
}

async fn rename(ctx: &Context, team: String, title: String) -> Result<(), CliError> {
    let found = find_team(ctx, &team).await?;
    let renamed = ctx
        .call(RenameAgentTeamRequest {
            team_id: found.id,
            title,
        })
        .await?;
    println!("Renamed {} to {}.", describe(&found), describe(&renamed));
    Ok(())
}

async fn delete(ctx: &Context, team: String, yes: bool) -> Result<(), CliError> {
    let found = find_team(ctx, &team).await?;
    let question = format!(
        "Delete {}? The team's agents won't be deleted, but its memories will.",
        describe(&found)
    );
    if !confirmed(&question, yes)? {
        return Ok(());
    }
    let deleted = ctx
        .call(DeleteAgentTeamRequest { team_id: found.id })
        .await?;
    println!("Deleted {}.", describe(&deleted));
    Ok(())
}

/// `team "title" (id)`, as messages name a team.
fn describe(team: &IntelligenceAgentTeam) -> String {
    format!("team \"{}\" ({})", team.title, team.id)
}
