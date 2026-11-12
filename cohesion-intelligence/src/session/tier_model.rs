//! Which endpoint model a tier's session runs on, and why a session could
//! not be built.

use cohesion_core::model::IntelligenceTier;
use cohesion_db::{
    CommandError, DatabaseCommand, DatabaseContext, GetAgentModelCommand,
    GetIntelligenceDefaultCommand, TierModel,
};

/// Why a session could not be built: a database command failed (reading its
/// tier's default, creating its thread), its tier has no default model, or
/// the thread it forks holds an item it can't read back.
#[derive(Debug)]
pub enum SessionConstructionError {
    Command(CommandError),
    /// The tier has no default model.
    NoDefaultModel,
}

/// The endpoint model `tier`'s default points to, with its endpoint, read now
/// so a changed default applies to the next session.
pub async fn tier_model(
    database: &DatabaseContext,
    tier: IntelligenceTier,
) -> Result<TierModel, SessionConstructionError> {
    model(database, GetIntelligenceDefaultCommand { tier }).await
}

/// The endpoint model agent `agent_id`'s conversation runs on: its own model,
/// or the `agent` tier's default when it has none. Read now, so a changed
/// model applies to the next session.
pub async fn agent_model(
    database: &DatabaseContext,
    agent_id: i64,
) -> Result<TierModel, SessionConstructionError> {
    model(database, GetAgentModelCommand { agent_id }).await
}

async fn model<C>(
    database: &DatabaseContext,
    command: C,
) -> Result<TierModel, SessionConstructionError>
where
    C: DatabaseCommand<Response = TierModel> + Send + 'static,
{
    database.send(command).await.map_err(|err| match err {
        CommandError::NotFound(_) => SessionConstructionError::NoDefaultModel,
        _ => SessionConstructionError::Command(err),
    })
}
