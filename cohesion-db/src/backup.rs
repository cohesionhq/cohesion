//! What a backup leaves out of its copy of the database. Every model says
//! whether a backup holds its rows ([`BackupClass`]); this applies that to the
//! copy `Container::copy_for_backup` made.

use std::path::Path;

use cohesion_core::Model;
use cohesion_core::model::*;
use cohesion_core::protocol::DatabaseBackup;
use cohesion_core::stored_model::BackupClass;
use rusqlite::Connection;

use crate::database_error::DatabaseError;
use crate::db_macros::every_model;

/// The tables a backup holds no rows of.
fn left_out_tables() -> Vec<&'static str> {
    fn note<M: Model>(tables: &mut Vec<&'static str>) {
        if M::BACKUP == BackupClass::LeftOut {
            tables.push(M::TABLE);
        }
    }
    let mut tables = Vec::new();
    every_model!(note(&mut tables));
    tables
}

/// Empties the left-out tables of the copy at `file` and compacts it into
/// one file that stands alone.
pub(crate) fn finish_backup(file: &Path) -> Result<DatabaseBackup, DatabaseError> {
    let copy = Connection::open(file)?;
    copy.execute_batch("PRAGMA foreign_keys = ON;")?;
    copy.query_row("PRAGMA journal_mode = DELETE", [], |_| Ok(()))?;
    for table in left_out_tables() {
        copy.execute(&format!("DELETE FROM {table}"), [])?;
    }
    copy.execute_batch("VACUUM")?;
    drop(copy);
    Ok(DatabaseBackup {
        path: file.to_string_lossy().into_owned(),
        bytes: std::fs::metadata(file)?.len() as i64,
    })
}

#[cfg(test)]
mod tests;
