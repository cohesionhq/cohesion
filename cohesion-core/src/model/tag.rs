use cohesion_macros::{Fields, Model, TsWire};
use serde::{Deserialize, Serialize};

use crate::value::{FromValue, Value, ValueError};

/// A name a workspace puts on its tasks, to group them across folders.
#[derive(Clone, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "tags", backup = Included)]
pub struct Tag {
    /// Unique, stable identifier.
    pub id: i64,
    /// Id of the workspace the tag belongs to.
    pub workspace_id: i64,
    /// Unique, ignoring case, among the workspace's tags.
    pub name: String,
    /// Position among the workspace's tags, first to last.
    pub sort_order: i64,
    /// In Unix seconds.
    pub created_at: i64,
}

/// One tag put on one task. The table's foreign keys hold both to the same
/// workspace, and take the row with either.
#[derive(Clone, Serialize, Deserialize, Model)]
#[model(table = "task_tags", backup = Included)]
pub struct TaskTag {
    pub id: i64,
    pub workspace_id: i64,
    pub tag_id: i64,
    pub task_id: i64,
}

/// The ids of the tags on a task; read from a JSON array.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TsWire)]
#[serde(transparent)]
pub struct TagIds(pub Vec<i64>);

impl FromValue for TagIds {
    fn from_value(value: Value) -> Result<Self, ValueError> {
        let text = String::from_value(value)?;
        serde_json::from_str(&text)
            .map(Self)
            .map_err(|e| ValueError::Invalid(e.to_string()))
    }
}

/// Which tags the items a listing covers must carry, and which they must
/// not: an item is listed when it has every tag in `with` and none in
/// `without`. Empty lists filter nothing.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TsWire)]
pub struct TagFilter {
    pub with: Vec<i64>,
    pub without: Vec<i64>,
}

impl FromValue for TagFilter {
    fn from_value(value: Value) -> Result<Self, ValueError> {
        let text = String::from_value(value)?;
        serde_json::from_str(&text).map_err(|e| ValueError::Invalid(e.to_string()))
    }
}
