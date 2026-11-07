//! Requests about activities: the long-running work Cohesion shows the user.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

/// Lists the activities still running or waiting, then the `limit` that
/// ended most recently, newest first.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListActivitiesRequest {
    pub limit: i64,
}
