//! Glue between the storage-agnostic value layer in cohesion-core and rusqlite.

use cohesion_core::value::{Row, RowError, Value};
use rusqlite::types::{ToSqlOutput, ValueRef};

/// Reads columns of a rusqlite row as [`Value`]s, so `Model::from_row` can parse it.
pub(crate) struct SqlRow<'a, 'stmt>(pub &'a rusqlite::Row<'stmt>);

impl Row for SqlRow<'_, '_> {
    fn get(&self, column: &str) -> Result<Value, RowError> {
        let value = self.0.get_ref(column).map_err(|err| match err {
            rusqlite::Error::InvalidColumnName(name) => RowError::MissingColumn(name),
            other => RowError::Backend(other.to_string()),
        })?;
        Ok(match value {
            ValueRef::Null => Value::Null,
            ValueRef::Integer(i) => Value::Integer(i),
            ValueRef::Real(r) => Value::Real(r),
            ValueRef::Text(bytes) => Value::Text(
                String::from_utf8(bytes.to_vec())
                    .map_err(|err| RowError::Backend(format!("column {column:?}: {err}")))?,
            ),
            ValueRef::Blob(bytes) => Value::Blob(bytes.to_vec()),
        })
    }
}

/// A borrowed [`Value`] bound as a statement parameter, so binding copies nothing.
pub(crate) struct ParamRef<'a>(pub &'a Value);

impl rusqlite::ToSql for ParamRef<'_> {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::Borrowed(value_ref(self.0)))
    }
}

/// `value` as rusqlite sees it, without copying text or blobs.
fn value_ref(value: &Value) -> ValueRef<'_> {
    match value {
        Value::Null => ValueRef::Null,
        Value::Integer(i) => ValueRef::Integer(*i),
        Value::Real(r) => ValueRef::Real(*r),
        Value::Text(text) => ValueRef::Text(text.as_bytes()),
        Value::Blob(bytes) => ValueRef::Blob(bytes),
    }
}
