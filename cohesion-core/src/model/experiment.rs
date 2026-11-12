//! Experiments: features still being tried, which each daemon's owner turns
//! on or off.

use cohesion_macros::{Fields, Model, TsWire};
use serde::{Deserialize, Serialize};

/// An experiment a daemon can turn on. Add a variant to offer one, with its
/// arms in [`Experiment::key`], [`Experiment::title`] and
/// [`Experiment::description`] and its place in [`Experiment::ALL`]; delete
/// the variant to discontinue it, and the compiler points at every place
/// that checked it. A discontinued experiment's stored switch is ignored
/// and removed by the next change to any switch.
///
/// There are none yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Experiment {}

impl Experiment {
    /// Every experiment, in the order Settings lists them.
    pub const ALL: &'static [Experiment] = &[];

    /// The stable name its switch is stored and sent under: camelCase.
    pub fn key(self) -> &'static str {
        match self {}
    }

    /// What Settings calls it.
    pub fn title(self) -> &'static str {
        match self {}
    }

    /// One sentence on what it changes, for Settings.
    pub fn description(self) -> &'static str {
        match self {}
    }

    /// The experiment named `key`, if it still exists.
    pub fn from_key(key: &str) -> Option<Experiment> {
        Self::ALL.iter().copied().find(|e| e.key() == key)
    }
}

/// An experiment a daemon's owner turned on; a row exists only while it is on.
#[derive(Clone, Serialize, Deserialize, Model, Fields)]
#[model(table = "enabled_experiments", backup = Included)]
pub struct EnabledExperiment {
    pub id: i64,
    /// [`Experiment::key`]. Unique.
    pub key: String,
}

/// An experiment and whether it is on, as Settings › Experiments lists it.
#[derive(Clone, Debug, Serialize, Deserialize, TsWire)]
pub struct ExperimentSetting {
    /// [`Experiment::key`].
    pub key: String,
    pub title: String,
    pub description: String,
    pub enabled: bool,
}
