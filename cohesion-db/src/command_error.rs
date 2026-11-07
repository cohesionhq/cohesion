//! How a command fails, and how each failure reaches the client.

use cohesion_core::Named;
use cohesion_core::protocol::{ErrorCode, ServerError};
use rusqlite::ffi;

use crate::database_error::DatabaseError;

/// Why a command did not produce a response.
#[derive(Debug)]
pub enum CommandError {
    /// The entity asked for, or one it refers to, does not exist.
    NotFound(&'static str),
    /// The entity could not be created or moved because the entity doesn't exist.
    InvalidReference,
    /// The caller may not run this command.
    Unauthorized,
    /// The input was rejected; the caller can fix it.
    Validation(ValidationError),
    /// Anything else; details stay on the server.
    Database(DatabaseError),
}

/// What every [`DatabaseCommand::handle`](crate::DatabaseCommand::handle) returns.
pub type DatabaseCommandResult<T> = Result<T, CommandError>;

/// Input the caller can fix, optionally naming the offending field.
#[derive(Debug)]
pub struct ValidationError {
    pub field: Option<String>,
    pub message: String,
}
impl ValidationError {
    /// A validation failure attributed to `field`.
    pub fn in_field(field: &'static str, message: String) -> Self {
        ValidationError {
            field: Some(field.to_string()),
            message,
        }
    }
    /// A validation failure not tied to one field.
    pub fn new(message: &'static str) -> Self {
        ValidationError {
            field: None,
            message: message.to_string(),
        }
    }
}
impl From<ValidationError> for CommandError {
    fn from(value: ValidationError) -> Self {
        Self::Validation(value)
    }
}
/// Constraint violations become errors the caller can act on; everything
/// else is an internal `Database` error.
impl From<DatabaseError> for CommandError {
    fn from(value: DatabaseError) -> Self {
        let DatabaseError::Sql(rusqlite::Error::SqliteFailure(
            ffi::Error {
                code: ffi::ErrorCode::ConstraintViolation,
                extended_code,
            },
            _,
        )) = &value
        else {
            return Self::Database(value);
        };
        match *extended_code {
            // A trigger refuses a row that refers to something the schema
            // rules out, such as a tag on another workspace's item.
            ffi::SQLITE_CONSTRAINT_FOREIGNKEY | ffi::SQLITE_CONSTRAINT_TRIGGER => {
                Self::InvalidReference
            }
            ffi::SQLITE_CONSTRAINT_NOTNULL => {
                ValidationError::new("A required value is missing.").into()
            }
            ffi::SQLITE_CONSTRAINT_UNIQUE | ffi::SQLITE_CONSTRAINT_PRIMARYKEY => {
                ValidationError::new("A value must be unique but already exists.").into()
            }
            ffi::SQLITE_CONSTRAINT_CHECK => ValidationError::new("A value is out of range.").into(),
            _ => Self::Database(value),
        }
    }
}

/// How a failed command is reported to a client. Database details stay on
/// the server; the client only learns that something went wrong.
impl From<CommandError> for ServerError {
    fn from(value: CommandError) -> Self {
        match value {
            CommandError::NotFound(missing) => ServerError {
                code: ErrorCode::NotFound,
                message: format!("The requested {} wasn’t found.", missing),
                field: None,
            },
            CommandError::InvalidReference => ServerError {
                code: ErrorCode::NotFound,
                message: "Something this refers to doesn’t exist.".to_string(),
                field: None,
            },
            CommandError::Unauthorized => ServerError {
                code: ErrorCode::Unauthorized,
                message: "Not authorized.".to_string(),
                field: None,
            },
            CommandError::Validation(error) => ServerError {
                code: ErrorCode::ValidationFailed,
                message: error.message,
                field: error.field,
            },
            CommandError::Database(_) => ServerError {
                code: ErrorCode::InternalError,
                message: "An internal error occurred.".to_string(),
                field: None,
            },
        }
    }
}

/// Runs a full-text search, turning SQLite's refusal of a malformed FTS5
/// query (`re-auth` reads as the column filter `auth`, an unbalanced quote)
/// into a validation error naming the fix, instead of an internal error.
pub fn searched<T>(result: Result<T, DatabaseError>) -> DatabaseCommandResult<T> {
    match result {
        Err(DatabaseError::Sql(rusqlite::Error::SqliteFailure(
            ffi::Error {
                code: ffi::ErrorCode::Unknown,
                ..
            },
            Some(detail),
        ))) if is_query_syntax_error(&detail) => Err(ValidationError {
            field: Some("query".to_string()),
            message: format!(
                "The search query isn’t valid ({detail}). Characters such as `-`, `:`, `.` or an unbalanced quote are search syntax. To search for the text itself, wrap it in double quotes."
            ),
        }
        .into()),
        other => Ok(other?),
    }
}

/// Whether SQLite's message for a generic error is FTS5 refusing the query
/// text. Every other failure, such as a missing table or a locked database,
/// shares the code and stays internal, its message on the server.
fn is_query_syntax_error(detail: &str) -> bool {
    [
        "no such column:",
        "unterminated string",
        "fts5:",
        "unknown special query",
    ]
    .iter()
    .any(|prefix| detail.starts_with(prefix))
}

/// Converts a missing row into `CommandError::NotFound`, so single-row
/// queries can be unwrapped with `?`.
pub trait Found<T> {
    /// The row, or `CommandError::NotFound` when there was none.
    fn found(self) -> DatabaseCommandResult<T>;
}
impl<T: Named> Found<T> for Result<Option<T>, DatabaseError> {
    fn found(self) -> DatabaseCommandResult<T> {
        self?.ok_or(CommandError::NotFound(T::NOUN))
    }
}
