//! One result row as a backend hands it over, and reading a typed column from it.

use std::fmt;

use super::column_value::Value;
use super::value_conversion::{FromValue, ValueError};

/// One result row, however the backend stores it.
pub trait Row {
    fn get(&self, column: &str) -> Result<Value, RowError>;
}

/// Why a model could not be read from a [`Row`].
#[derive(Debug)]
pub enum RowError {
    MissingColumn(String),
    Value { column: String, error: ValueError },
    Backend(String),
}

impl fmt::Display for RowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingColumn(column) => write!(f, "missing column {column:?}"),
            Self::Value { column, error } => write!(f, "column {column:?}: {error}"),
            Self::Backend(message) => write!(f, "backend error: {message}"),
        }
    }
}
impl std::error::Error for RowError {}

/// Reads `column` from `row` as a `T`; what `#[derive(Model)]` calls per field.
pub fn read_column<T: FromValue>(row: &dyn Row, column: &str) -> Result<T, RowError> {
    T::from_value(row.get(column)?).map_err(|error| RowError::Value {
        column: column.to_string(),
        error,
    })
}
