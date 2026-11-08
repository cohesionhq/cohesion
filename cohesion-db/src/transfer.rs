//! Moving a daemon's data to another daemon. [`export_database`] copies the
//! whole database, every table and every image, while the daemon runs;
//! [`import_database`] puts such a copy in place of a stopped daemon's
//! database. Only the database moves: the server key and settings are files
//! beside it, so the daemon imported into keeps its own, and the clients the
//! copy holds, which paired with the other server's key, are revoked.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use cohesion_core::private_files::open_private;
use rusqlite::{Connection, ErrorCode, OpenFlags};

use crate::command_error::CommandError;
use crate::commands::RevokeEveryClientCommand;
use crate::container::Container;
use crate::database_context::DatabaseContext;
use crate::database_error::DatabaseError;
use crate::secrets::Secrets;

/// Why a database couldn't be exported or imported.
#[derive(Debug)]
pub enum TransferError {
    /// The file to read doesn't exist.
    Missing(PathBuf),
    /// The file to write already exists.
    Exists(PathBuf),
    /// The file is an SQLite database no Cohesion daemon wrote.
    NotCohesion(PathBuf),
    /// Something else has the database open: a daemon still running on it.
    InUse(PathBuf),
    /// Revoking the imported clients failed.
    Revoke(CommandError),
    /// Reading or writing a database failed.
    Database(DatabaseError),
}

impl fmt::Display for TransferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(path) => write!(f, "{} doesn’t exist.", path.display()),
            Self::Exists(path) => write!(
                f,
                "{} already exists. Choose a new file name.",
                path.display()
            ),
            Self::NotCohesion(path) => write!(
                f,
                "{} isn’t a Cohesion export. Make one with cohesiond export.",
                path.display()
            ),
            Self::InUse(path) => write!(
                f,
                "cohesiond is running on {}. Stop it, then import again.",
                path.display()
            ),
            Self::Revoke(error) => write!(f, "Couldn’t revoke the imported clients: {error:?}"),
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

/// Copies the database at `database`, as it stands, to a new private file
/// at `file` that stands alone, and returns its size in bytes. The copy is
/// one read transaction, so it is consistent while a daemon writes.
pub fn export_database(database: &Path, file: &Path) -> Result<u64, TransferError> {
    if file.exists() {
        return Err(TransferError::Exists(file.to_path_buf()));
    }
    let source = open_read_only(database)?;
    copy_into(&source, file)?;
    Ok(std::fs::metadata(file)?.len())
}

/// Replaces the database at `database` with the export at `file`: brought
/// up to this build's schema, every client revoked, and work it caught
/// running settled, as a daemon's start would. Refuses while a daemon has
/// `database` open. Nothing of the old database is left; the files beside
/// it (the server key, the settings) are not touched.
pub async fn import_database(file: &Path, database: &Path) -> Result<(), TransferError> {
    let snapshot = open_read_only(file)?;
    let applied: i64 = snapshot
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|error| match error.sqlite_error_code() {
            Some(ErrorCode::NotADatabase) => TransferError::NotCohesion(file.to_path_buf()),
            _ => error.into(),
        })?;
    if applied == 0 {
        return Err(TransferError::NotCohesion(file.to_path_buf()));
    }
    release(database)?;
    let staging = beside(database, ".import");
    remove_if_present(&staging)?;
    let prepared = match copy_into(&snapshot, &staging) {
        Ok(()) => {
            drop(snapshot);
            prepare(&staging).await
        }
        Err(error) => Err(error),
    };
    if let Err(error) = prepared {
        remove_if_present(&staging)?;
        return Err(error);
    }
    for stale in ["-wal", "-shm"] {
        remove_if_present(&beside(database, stale))?;
    }
    std::fs::rename(&staging, database)?;
    Ok(())
}

/// Opens the database at `path` for reading only.
fn open_read_only(path: &Path) -> Result<Connection, TransferError> {
    if !path.exists() {
        return Err(TransferError::Missing(path.to_path_buf()));
    }
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

/// Takes `database`, if there is one, out of WAL mode, which SQLite allows
/// only while no other connection has it open: the one way to tell that no
/// daemon is running on it. Its WAL is folded in and removed on the way.
fn release(database: &Path) -> Result<(), TransferError> {
    if !database.exists() {
        return Ok(());
    }
    let connection = Connection::open(database)?;
    connection.busy_timeout(Duration::ZERO)?;
    connection
        .query_row("PRAGMA journal_mode = DELETE", [], |_| Ok(()))
        .map_err(|error| match error.sqlite_error_code() {
            Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => {
                TransferError::InUse(database.to_path_buf())
            }
            _ => error.into(),
        })
}

/// Migrates the copy at `staging`, revokes its clients and settles the work
/// it caught running, then leaves it as one file.
async fn prepare(staging: &Path) -> Result<(), TransferError> {
    let mut container = Container::open(staging)?;
    container.migrate()?;
    let db = DatabaseContext::new(container, Secrets::new(), ());
    let revoked = db.send(RevokeEveryClientCommand).await;
    // Closing runs the cleanup a daemon's start would.
    db.close().await;
    revoked.map_err(TransferError::Revoke)?;
    Connection::open(staging)?.query_row("PRAGMA journal_mode = DELETE", [], |_| Ok(()))?;
    Ok(())
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
