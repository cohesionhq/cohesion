use cohesion_core::TextEnum;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, TextEnum)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum TimerUnit {
    Second,
    Minute,
    Hour,
}
