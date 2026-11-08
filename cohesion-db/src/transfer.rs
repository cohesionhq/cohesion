//! Moving a daemon's data to another daemon, through clients. An export is
//! the whole database, every table and image, copied while the daemon runs
//! and read out in parts; an import is written in parts, then staged: a copy
//! of it beside the database, brought up to this build's schema, holding
//! this server's clients in place of its own. The daemon puts the staged
//! copy in place once it has shut down ([`StagedImport::replace`]), then
//! starts again. Only the database moves: the server key and settings are
//! files beside it, so the daemon imported into keeps its own.
//!
//! Exports and imports under way are files in a `transfers` directory beside
//! the database, which [`clear_transfers`] empties when the daemon starts.

use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use cohesion_core::Client;
use cohesion_core::private_files::{create_private_dir, open_private};
use cohesion_core::protocol::{
    DatabaseExport, DatabaseExportPart, DatabaseImport, TransferBytes, TransferId,
};
use rusqlite::{Connection, ErrorCode, OpenFlags};

use crate::command_error::CommandError;
use crate::commands::{ListClientsCommand, ReplaceClientsCommand};
use crate::container::Container;
use crate::database_context::DatabaseContext;
use crate::database_error::DatabaseError;
use crate::secrets::{Secrets, random_string};

/// The most of an export one read answers: 8 MB, a third again in base64.
const PART: usize = 8 * 1024 * 1024;

/// Why an export or import failed.
#[derive(Debug)]
pub enum TransferError {
    /// No export or import has this id: it never began, was read to the
    /// end, was finished, or the daemon restarted.
    Unknown,
    /// A part of an import didn't start where the import ends.
    WrongOffset { holds: u64 },
    /// What was imported isn't a database a Cohesion daemon exported.
    NotCohesion,
    /// Something else still has the database open.
    InUse,
    /// Reading the clients, or putting this server's in the import, failed.
    Command(CommandError),
    /// Reading or writing a file failed.
    Database(DatabaseError),
}

impl fmt::Display for TransferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown => write!(f, "There’s no export or import with that id."),
            Self::WrongOffset { holds } => {
                write!(
                    f,
                    "The import holds {holds} bytes, so the next part starts there."
                )
            }
            Self::NotCohesion => write!(f, "The import isn’t a Cohesion export."),
            Self::InUse => write!(f, "Something else has the database open."),
            Self::Command(error) => write!(f, "{error:?}"),
            Self::Database(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for TransferError {}

impl From<DatabaseError> for TransferError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}
impl From<rusqlite::Error> for TransferError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error.into())
    }
}
impl From<std::io::Error> for TransferError {
    fn from(error: std::io::Error) -> Self {
        Self::Database(error.into())
    }
}

/// An import made ready beside the database, to replace it once the daemon
/// has closed it.
#[derive(Debug)]
pub struct StagedImport {
    staging: PathBuf,
    database: PathBuf,
}

impl StagedImport {
    /// Puts the import in place of the database. Refuses while anything has
    /// the database open, so call it once the daemon has closed its own.
    pub fn replace(self) -> Result<(), TransferError> {
        release(&self.database)?;
        for stale in ["-wal", "-shm"] {
            remove_if_present(&beside(&self.database, stale))?;
        }
        std::fs::rename(&self.staging, &self.database)?;
        Ok(())
    }
}

/// Removes the exports and imports left in the `transfers` directory beside
/// `database`: no id names them once the daemon that made them has stopped.
pub fn clear_transfers(database: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(transfers(database)) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

impl DatabaseContext {
    /// Copies the whole database, as it stands, to a new export. The copy is
    /// one read transaction on a connection of its own, so it is consistent
    /// and commands carry on while it runs.
    pub async fn export(&self) -> Result<DatabaseExport, TransferError> {
        let database = self.database_file()?;
        blocking(move || {
            create_private_dir(&transfers(&database))?;
            let id = new_id();
            let file = export_file(&database, &id);
            copy_into(&open_read_only(&database)?, &file)?;
            Ok(DatabaseExport {
                id,
                bytes: std::fs::metadata(&file)?.len() as i64,
            })
        })
        .await
    }

    /// The part of export `id` from `offset`, at most [`PART`] bytes. The
    /// part that reaches the end removes the export.
    pub async fn read_export(
        &self,
        id: TransferId,
        offset: i64,
    ) -> Result<DatabaseExportPart, TransferError> {
        let file = export_file(&self.database_file()?, &id);
        blocking(move || {
            let mut export = File::open(&file).map_err(unknown_if_missing)?;
            let length = export.metadata()?.len();
            let offset = u64::try_from(offset).unwrap_or(0).min(length);
            export.seek(SeekFrom::Start(offset))?;
            let mut data = Vec::new();
            export.take(PART as u64).read_to_end(&mut data)?;
            if offset + data.len() as u64 >= length {
                remove_if_present(&file)?;
            }
            Ok(DatabaseExportPart {
                data: TransferBytes(data),
            })
        })
        .await
    }

    /// Starts an import: an empty file for [`write_import`](Self::write_import).
    pub async fn begin_import(&self) -> Result<DatabaseImport, TransferError> {
        let database = self.database_file()?;
        blocking(move || {
            create_private_dir(&transfers(&database))?;
            let id = new_id();
            open_private(&import_file(&database, &id))?;
            Ok(DatabaseImport { id })
        })
        .await
    }

    /// Adds `data` to import `id`, which must hold `offset` bytes already.
    pub async fn write_import(
        &self,
        id: TransferId,
        offset: i64,
        data: TransferBytes,
    ) -> Result<(), TransferError> {
        let file = import_file(&self.database_file()?, &id);
        blocking(move || {
            let mut import = OpenOptions::new()
                .append(true)
                .open(&file)
                .map_err(unknown_if_missing)?;
            let holds = import.metadata()?.len();
            if i64::try_from(holds).ok() != Some(offset) {
                return Err(TransferError::WrongOffset { holds });
            }
            import.write_all(&data.0)?;
            Ok(())
        })
        .await
    }

    /// Stages import `id` to replace this database, holding this database's
    /// clients in place of its own, and removes the import. The daemon
    /// carries on until it shuts down to put the staged copy in place.
    pub async fn stage_import(&self, id: TransferId) -> Result<StagedImport, TransferError> {
        let database = self.database_file()?;
        let file = import_file(&database, &id);
        if !file.exists() {
            return Err(TransferError::Unknown);
        }
        let clients = self
            .send(ListClientsCommand {})
            .await
            .map_err(TransferError::Command)?;
        let staged = stage(file.clone(), database, clients).await;
        remove_if_present(&file)?;
        staged
    }

    fn database_file(&self) -> Result<PathBuf, TransferError> {
        self.file.as_deref().cloned().ok_or_else(|| {
            DatabaseError::Storage(std::io::Error::other("the database has no file")).into()
        })
    }
}

/// Copies `file` to a staging file beside `database`, migrates it, puts
/// `clients` in place of its own and settles the work it caught running,
/// as a daemon's start would. A failure leaves no staging file.
async fn stage(
    file: PathBuf,
    database: PathBuf,
    clients: Vec<Client>,
) -> Result<StagedImport, TransferError> {
    let staging = beside(&database, ".import");
    let prepared = prepare(file, staging.clone(), clients).await;
    if let Err(error) = prepared {
        remove_if_present(&staging)?;
        return Err(error);
    }
    Ok(StagedImport { staging, database })
}

async fn prepare(
    file: PathBuf,
    staging: PathBuf,
    clients: Vec<Client>,
) -> Result<(), TransferError> {
    let copy = staging.clone();
    let container = blocking(move || {
        let snapshot = open_read_only(&file)?;
        let applied: i64 = snapshot
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|error| match error.sqlite_error_code() {
                Some(ErrorCode::NotADatabase) => TransferError::NotCohesion,
                _ => error.into(),
            })?;
        if applied == 0 {
            return Err(TransferError::NotCohesion);
        }
        remove_if_present(&copy)?;
        copy_into(&snapshot, &copy)?;
        let mut container = Container::open(&copy)?;
        container.migrate()?;
        Ok(container)
    })
    .await?;
    let db = DatabaseContext::new(container, Secrets::new(), ());
    let replaced = db.send(ReplaceClientsCommand { clients }).await;
    // Closing runs the cleanup a daemon's start would.
    db.close().await;
    replaced.map_err(TransferError::Command)?;
    blocking(move || {
        Connection::open(&staging)?.query_row("PRAGMA journal_mode = DELETE", [], |_| Ok(()))?;
        Ok(())
    })
    .await
}

/// Runs `work` where blocking is allowed.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, TransferError> + Send + 'static,
) -> Result<T, TransferError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| DatabaseError::Storage(std::io::Error::other(error)))?
}

fn new_id() -> TransferId {
    TransferId::try_from(random_string(32)).expect("letters and digits make an id")
}

fn transfers(database: &Path) -> PathBuf {
    database
        .parent()
        .unwrap_or(Path::new("."))
        .join("transfers")
}

fn export_file(database: &Path, id: &TransferId) -> PathBuf {
    transfers(database).join(format!("{}.export", id.as_str()))
}

fn import_file(database: &Path, id: &TransferId) -> PathBuf {
    transfers(database).join(format!("{}.import", id.as_str()))
}

fn unknown_if_missing(error: std::io::Error) -> TransferError {
    match error.kind() {
        std::io::ErrorKind::NotFound => TransferError::Unknown,
        _ => error.into(),
    }
}

/// Opens the database at `path` for reading only.
fn open_read_only(path: &Path) -> Result<Connection, TransferError> {
    Ok(Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?)
}

/// Writes everything `source` holds to a new private file at `file`, in
/// the rollback journal, so the file is all there is of it. A failed copy
/// leaves no file.
fn copy_into(source: &Connection, file: &Path) -> Result<(), TransferError> {
    // `VACUUM INTO` takes an empty file as it finds it, so the mode holds.
    open_private(file)?;
    let copied = source
        .execute("VACUUM INTO ?1", [file.to_string_lossy()])
        .map_err(TransferError::from)
        .and_then(|_| {
            Connection::open(file)?.query_row("PRAGMA journal_mode = DELETE", [], |_| Ok(()))?;
            Ok(())
        });
    if copied.is_err() {
        remove_if_present(file)?;
    }
    copied
}

/// Takes `database` out of WAL mode, which SQLite allows only while no
/// other connection has it open; its WAL is folded in and removed.
fn release(database: &Path) -> Result<(), TransferError> {
    let connection = Connection::open(database)?;
    connection.busy_timeout(Duration::ZERO)?;
    connection
        .query_row("PRAGMA journal_mode = DELETE", [], |_| Ok(()))
        .map_err(|error| match error.sqlite_error_code() {
            Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => TransferError::InUse,
            _ => error.into(),
        })
}

/// `path` with `suffix` added to its file name.
fn beside(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

fn remove_if_present(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests;
