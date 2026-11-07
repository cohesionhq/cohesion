//! How long the daemon keeps what nothing needs again, and how many
//! memories it keeps: the storage settings.

use cohesion_macros::{Model, TsWire};
use serde::{Deserialize, Serialize};

/// The daemon's storage settings; there is always exactly one row. The
/// hourly cleanup applies the retentions, and remembering applies the
/// memory limit. Every value is at least one.
#[derive(Clone, Debug, Serialize, Deserialize, Model, TsWire)]
#[model(table = "storage_settings", backup = Included)]
pub struct StorageSettings {
    pub id: i64,
    /// A model thread nothing has been added to for this many days is
    /// deleted, and a thread message or a script's result in an agent's chat
    /// this old loses its contents.
    pub thread_retention_days: i64,
    /// An actor session resolved this many days ago is deleted, with its images;
    /// one never resolved stays.
    pub actor_session_retention_days: i64,
    /// A kept version of a task or document last changed this many days ago
    /// is deleted, and with the last of them a deleted item's way back.
    pub rewind_retention_days: i64,
    /// A deleted task, document or journal entry is removed for good this many
    /// days after it was deleted, or once Rewind's retention has passed if that
    /// is longer.
    pub deleted_item_retention_days: i64,
    /// The most memories each scope keeps: an agent its own, a team its
    /// own, and every agent's together. Lowering it forgets nothing; past
    /// it, remembering another in that scope is refused.
    pub memory_limit: i64,
}
