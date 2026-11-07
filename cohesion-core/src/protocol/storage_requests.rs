//! Requests about what the daemon keeps: the storage settings and the size
//! of its database.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

/// Answers the storage settings.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct GetStorageSettingsRequest;

/// Replaces the storage settings.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct UpdateStorageSettingsRequest {
    /// Each at least one.
    pub thread_retention_days: i64,
    pub actor_session_retention_days: i64,
    pub rewind_retention_days: i64,
    pub deleted_item_retention_days: i64,
    pub memory_limit: i64,
}

/// Answers how large the database file is.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct GetDatabaseSizeRequest;

/// How large the database file is.
#[derive(Serialize, Deserialize, Clone, TsWire)]
pub struct DatabaseSize {
    /// The whole file, in bytes.
    pub bytes: i64,
    /// What of it is free pages, in bytes: space SQLite reuses before the
    /// file grows, given back once the cleanup vacuums.
    pub free_bytes: i64,
}

/// Writes a backup of the database to a new file on the daemon's computer:
/// everything but the images in actor sessions and agent messages.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct BackupDatabaseRequest;

/// A backup the daemon wrote.
#[derive(Serialize, Deserialize, Clone, TsWire)]
pub struct DatabaseBackup {
    /// Where it is, on the daemon's computer.
    pub path: String,
    /// How large it is, in bytes.
    pub bytes: i64,
}
