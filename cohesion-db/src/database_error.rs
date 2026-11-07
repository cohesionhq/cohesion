use cohesion_core::value::RowError;

/// Why a database operation failed.
#[derive(Debug)]
pub enum DatabaseError {
    /// The worker thread that owns the connection has stopped.
    Closed,
    /// SQLite rejected or failed a statement.
    Sql(rusqlite::Error),
    /// A schema migration could not be applied.
    Migration {
        name: &'static str,
        error: rusqlite::Error,
    },
    /// The database records more migrations than this build has: it was
    /// written by a newer build, or predates a squash it wasn't moved onto.
    UnknownSchema { applied: usize, known: usize },
    /// The database file or its directory could not be created.
    Storage(std::io::Error),
    /// A row came back that doesn't match the model's columns.
    Row(RowError),
}
impl DatabaseError {
    /// Whether a UNIQUE constraint refused the statement: the value a
    /// command wrote is already taken.
    pub fn is_unique_violation(&self) -> bool {
        matches!(
            self,
            DatabaseError::Sql(rusqlite::Error::SqliteFailure(error, _))
                if error.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
        )
    }
}
impl std::fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatabaseError::Closed => write!(f, "database connection closed"),
            DatabaseError::Sql(err) => write!(f, "sql error: {err}"),
            DatabaseError::Migration { name, error } => {
                write!(f, "migration {name} failed: {error}")
            }
            DatabaseError::UnknownSchema { applied, known } => write!(
                f,
                "the database has applied {applied} migrations, but this build knows {known}"
            ),
            DatabaseError::Storage(err) => write!(f, "cannot create the database file: {err}"),
            DatabaseError::Row(err) => write!(f, "unreadable row: {err}"),
        }
    }
}
impl std::error::Error for DatabaseError {}
impl From<rusqlite::Error> for DatabaseError {
    fn from(value: rusqlite::Error) -> Self {
        DatabaseError::Sql(value)
    }
}
impl From<std::io::Error> for DatabaseError {
    fn from(value: std::io::Error) -> Self {
        DatabaseError::Storage(value)
    }
}
impl From<RowError> for DatabaseError {
    fn from(value: RowError) -> Self {
        DatabaseError::Row(value)
    }
}
