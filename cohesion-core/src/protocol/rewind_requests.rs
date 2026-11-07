//! Requests about Rewind's kept versions: reading them and restoring from
//! them. No request changes or removes a version; a restore keeps a new one.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

use crate::model::VersionTarget;

/// Lists an item's kept versions, newest first, without their text. A deleted
/// item still has its history.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListVersionsRequest {
    pub workspace_id: i64,
    pub item: VersionTarget,
}

/// Fetches one kept version.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct GetVersionRequest {
    pub workspace_id: i64,
    pub version_id: i64,
}

/// Puts an item's text back to a kept version's (a task's status too),
/// bringing the item back if it was deleted, and keeps the result as a new
/// version. A deleted journal entry is refused while its day has another.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RestoreVersionRequest {
    pub workspace_id: i64,
    pub version_id: i64,
}

/// Lists the workspace's deleted items whose deletion Rewind still has, most
/// recently deleted first.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListDeletedItemsRequest {
    pub workspace_id: i64,
}
