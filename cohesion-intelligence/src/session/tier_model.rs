//! Which endpoint model a tier's session runs on, and why a session could
//! not be built.

use cohesion_core::model::IntelligenceTier;
use cohesion_db::{CommandError, DatabaseContext, GetIntelligenceDefaultCommand, TierModel};

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
    database
        .send(GetIntelligenceDefaultCommand { tier })
        .await
        .map_err(|err| match err {
            CommandError::NotFound(_) => SessionConstructionError::NoDefaultModel,
            _ => SessionConstructionError::Command(err),
        })
}
