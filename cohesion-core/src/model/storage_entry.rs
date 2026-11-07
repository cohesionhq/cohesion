use cohesion_macros::{ColumnGroup, Model};
use serde::{Deserialize, Serialize};

/// One key/value pair in the private store of an extension or an agent,
/// which scripts reach through the `storage` global.
#[derive(Clone, Serialize, Deserialize, Model)]
#[model(table = "storage_entries", backup = Included)]
pub struct StorageEntry {
    /// Unique, stable identifier.
    pub id: i64,
    /// Whose store it is in.
    #[model(group)]
    pub owner: StorageOwner,
    /// Unique per store.
    pub key: String,
    /// The value as JSON.
    pub value: String,
}

/// Whose store a pair is in. Real foreign keys, so a store goes with its owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ColumnGroup)]
pub enum StorageOwner {
    Extension { extension_id: i64 },
    Agent { agent_id: i64 },
}
