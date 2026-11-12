//! Requests about experiments: listing them and turning one on or off.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

/// Answers every experiment and whether it is on.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListExperimentsRequest;

/// Turns the experiment named `key` on or off. Refused for a key no
/// experiment has.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetExperimentRequest {
    pub key: String,
    pub enabled: bool,
}
