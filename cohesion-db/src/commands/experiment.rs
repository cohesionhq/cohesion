use cohesion_core::FromRequest;
use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{EnabledExperiment, Experiment, ExperimentSetting};
use cohesion_core::protocol::{ListExperimentsRequest, SetExperimentRequest};

use crate::changed::{Changed, Invalidates};
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, DatabaseError, Filtered, Read,
    ReadableDatabase, ValidationError, Write,
};

/// Whether `experiment` is on. The check a caller makes before running an
/// experimental path.
pub fn experiment_enabled<'c>(
    db: impl ReadableDatabase<'c>,
    experiment: Experiment,
) -> Result<bool, DatabaseError> {
    Ok(db
        .select::<EnabledExperiment>()
        .filter_eq(EnabledExperiment::KEY, experiment.key().to_string())
        .one()?
        .is_some())
}

/// Every experiment, in order, with whether it is on.
fn settings<'c>(db: impl ReadableDatabase<'c>) -> Result<Vec<ExperimentSetting>, DatabaseError> {
    let on: Vec<String> = db
        .select::<EnabledExperiment>()
        .all()?
        .into_iter()
        .map(|row| row.key)
        .collect();
    Ok(Experiment::ALL
        .iter()
        .map(|experiment| ExperimentSetting {
            key: experiment.key().to_string(),
            title: experiment.title().to_string(),
            description: experiment.description().to_string(),
            enabled: on.iter().any(|key| key == experiment.key()),
        })
        .collect())
}

/// Every experiment and whether it is on.
#[derive(FromRequest)]
#[request(ListExperimentsRequest)]
pub struct ListExperimentsCommand {}
impl DatabaseCommand for ListExperimentsCommand {
    type Response = Vec<ExperimentSetting>;
    type Access = Read;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Read>,
    ) -> DatabaseCommandResult<Vec<ExperimentSetting>> {
        Ok(settings(ctx.db)?)
    }
}

/// Turns an experiment on or off. The switches of experiments that no longer
/// exist are removed with it.
#[derive(FromRequest)]
#[request(SetExperimentRequest)]
pub struct SetExperimentCommand {
    pub key: String,
    pub enabled: bool,
}
impl DatabaseCommand for SetExperimentCommand {
    type Response = Vec<ExperimentSetting>;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Vec<ExperimentSetting>>> {
        let Some(experiment) = Experiment::from_key(&self.key) else {
            return Err(ValidationError::in_field(
                "key",
                format!("There’s no experiment named “{}”.", self.key),
            )
            .into());
        };
        let kept: Vec<String> = Experiment::ALL
            .iter()
            .map(|e| e.key().to_string())
            .collect();
        ctx.db
            .delete::<EnabledExperiment>()
            .filter_not_in_values("key", kept)
            .execute()?;
        match self.enabled {
            true => {
                ctx.db
                    .insert::<EnabledExperiment>()
                    .set(EnabledExperiment::KEY, experiment.key().to_string())
                    .on_conflict_update(&["key"])
                    .execute()?;
            }
            false => {
                ctx.db
                    .delete::<EnabledExperiment>()
                    .filter_eq(EnabledExperiment::KEY, experiment.key().to_string())
                    .execute()?;
            }
        }
        Ok(settings(ctx.db.read())?.invalidating([Invalidation::Experiments]))
    }
}

#[cfg(test)]
mod tests;
