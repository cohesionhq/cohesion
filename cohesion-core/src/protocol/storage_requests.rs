//! Requests about what the daemon keeps: the storage settings, the size of
//! its database, and copying the database out or replacing it.

use base64::{Engine, engine::general_purpose::STANDARD};
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

/// How large the database file is, and what fills it.
#[derive(Serialize, Deserialize, Clone, TsWire)]
pub struct DatabaseSize {
    /// The whole file, in bytes: its pages, without the `-wal` file beside it.
    pub bytes: i64,
    /// What of it is free pages, in bytes: space SQLite reuses before the
    /// file grows, given back once the cleanup vacuums.
    pub free_bytes: i64,
    /// What each area's tables and their indexes take up, largest first and
    /// [`StorageArea::Other`] last. With `free_bytes` they add up to `bytes`.
    pub areas: Vec<StorageAreaSize>,
}

/// One area's share of the database file.
#[derive(Serialize, Deserialize, Clone, TsWire)]
pub struct StorageAreaSize {
    pub area: StorageArea,
    /// The pages its tables and indexes take up, in bytes.
    pub bytes: i64,
}

/// A part of what Cohesion keeps, as Settings › Storage breaks the
/// database's size down.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TsWire)]
#[serde(rename_all = "camelCase")]
pub enum StorageArea {
    /// Agents' messages, the model threads behind them, their runs and
    /// the images in their messages.
    AgentChats,
    /// Actor sessions, their messages, the images in them, and their
    /// search index.
    ActorSessions,
    /// Documents, journal entries, tasks, their folders, tags and saved
    /// views.
    DocumentsAndTasks,
    /// Feeds, their articles and the corpus's search index.
    FeedsAndArticles,
    /// Resources and their kinds.
    Resources,
    /// Agents' and teams' memories.
    Memories,
    /// Rewind's earlier versions of tasks, documents and journal entries.
    RewindVersions,
    /// Everything else: the workspace search index, metadata, contacts,
    /// calendars, workspaces, settings, extensions and their storage, actors,
    /// connectors, models and spend records.
    Other,
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

/// Names one export or import under way on the daemon: letters and digits
/// only, so it can name a file there.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(try_from = "String", into = "String")]
pub struct TransferId(String);

impl TransferId {
    /// The id, as it names the transfer's file.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for TransferId {
    type Error = &'static str;
    fn try_from(id: String) -> Result<Self, Self::Error> {
        match !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric()) {
            true => Ok(Self(id)),
            false => Err("a transfer id is 1 to 64 letters and digits"),
        }
    }
}

impl From<TransferId> for String {
    fn from(id: TransferId) -> Self {
        id.0
    }
}

impl crate::ts_wire::TsWire for TransferId {
    fn ts_type() -> String {
        "string".to_string()
    }
}

/// Part of a file moving between a client and the daemon: bytes, base64 on
/// the wire.
#[derive(Clone, PartialEq, Eq)]
pub struct TransferBytes(pub Vec<u8>);

impl std::fmt::Debug for TransferBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TransferBytes({} bytes)", self.0.len())
    }
}

impl Serialize for TransferBytes {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&STANDARD.encode(&self.0))
    }
}

impl<'de> Deserialize<'de> for TransferBytes {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        STANDARD
            .decode(text)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

impl crate::ts_wire::TsWire for TransferBytes {
    fn ts_type() -> String {
        "string".to_string()
    }
}

/// Copies the whole database, every table and image, to a new file on the
/// daemon's computer, for another daemon to import. Read it with
/// `readDatabaseExport`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ExportDatabaseRequest;

/// An export the daemon wrote, waiting to be read.
#[derive(Serialize, Deserialize, Clone, TsWire)]
pub struct DatabaseExport {
    pub id: TransferId,
    /// How large it is, in bytes.
    pub bytes: i64,
}

/// Answers the part of export `id` that starts at `offset`. The part that
/// reaches the end removes the export from the daemon's computer.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ReadDatabaseExportRequest {
    pub id: TransferId,
    pub offset: i64,
}

/// Part of an export.
#[derive(Serialize, Deserialize, Clone, TsWire)]
pub struct DatabaseExportPart {
    pub data: TransferBytes,
}

/// Starts an import: an empty file on the daemon's computer that
/// `writeDatabaseImport` fills and `finishDatabaseImport` puts in place.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct BeginDatabaseImportRequest;

/// An import under way.
#[derive(Serialize, Deserialize, Clone, TsWire)]
pub struct DatabaseImport {
    pub id: TransferId,
}

/// Adds `data` to import `id`. `offset` is how much it holds already, so a
/// part sent twice or out of order is refused.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct WriteDatabaseImportRequest {
    pub id: TransferId,
    pub offset: i64,
    pub data: TransferBytes,
}

/// Replaces everything in the database with import `id`, then restarts the
/// daemon. The daemon keeps its key and the clients paired with it; the
/// clients the import holds are revoked.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct FinishDatabaseImportRequest {
    pub id: TransferId,
}
