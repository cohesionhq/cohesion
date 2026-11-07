//! A serde type stored as JSON text in one column.

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::column_value::Value;
use super::value_conversion::{FromValue, IntoValue, ValueError};

/// A `T` stored as JSON text. Write a field by passing `Json(&field)` (or
/// `Json(field)`) to `set`; a model field marked `#[model(json)]` is read
/// through it, and a stored value that is not valid JSON for `T` is a
/// [`RowError`](super::RowError) rather than a panic.
#[derive(Debug, Clone, PartialEq)]
pub struct Json<T>(pub T);

impl<T: Serialize> IntoValue for Json<T> {
    fn into_value(self) -> Value {
        // The stored types are plain derives, whose serialization can't fail.
        Value::Text(serde_json::to_string(&self.0).expect("a stored value serializes"))
    }
}

impl<T: DeserializeOwned> FromValue for Json<T> {
    fn from_value(value: Value) -> Result<Self, ValueError> {
        let text = String::from_value(value)?;
        serde_json::from_str(&text)
            .map(Json)
            .map_err(|error| ValueError::Invalid(error.to_string()))
    }
}
