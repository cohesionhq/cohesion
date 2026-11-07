//! Conversions between Rust values and stored [`Value`]s, and why one can fail.

use std::fmt;

use super::column_value::Value;

/// Converts a Rust value into a [`Value`] to store. Consumes the value, so
/// owned text and blobs move into the [`Value`] without copying; references
/// are accepted and cloned.
pub trait IntoValue {
    fn into_value(self) -> Value;
}

/// Converts a stored [`Value`] back into a Rust value.
pub trait FromValue: Sized {
    fn from_value(value: Value) -> Result<Self, ValueError>;
}

/// Why a [`Value`] could not become the requested type.
#[derive(Debug)]
pub enum ValueError {
    WrongType {
        expected: &'static str,
        found: &'static str,
    },
    Invalid(String),
}

impl ValueError {
    /// A [`ValueError::WrongType`] for `value`, which was expected to be `expected`.
    pub fn wrong_type(expected: &'static str, value: &Value) -> Self {
        Self::WrongType {
            expected,
            found: value.type_name(),
        }
    }
}

impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongType { expected, found } => write!(f, "expected {expected}, found {found}"),
            Self::Invalid(message) => write!(f, "invalid value: {message}"),
        }
    }
}
impl std::error::Error for ValueError {}

impl IntoValue for Value {
    fn into_value(self) -> Value {
        self
    }
}
impl FromValue for Value {
    fn from_value(value: Value) -> Result<Self, ValueError> {
        Ok(value)
    }
}

impl IntoValue for i64 {
    fn into_value(self) -> Value {
        Value::Integer(self)
    }
}
impl FromValue for i64 {
    fn from_value(value: Value) -> Result<Self, ValueError> {
        match value {
            Value::Integer(i) => Ok(i),
            other => Err(ValueError::wrong_type("integer", &other)),
        }
    }
}

impl IntoValue for i32 {
    fn into_value(self) -> Value {
        Value::Integer(i64::from(self))
    }
}
impl FromValue for i32 {
    fn from_value(value: Value) -> Result<Self, ValueError> {
        let i = i64::from_value(value)?;
        i32::try_from(i).map_err(|_| ValueError::Invalid(format!("{i} does not fit in an i32")))
    }
}

impl IntoValue for f64 {
    fn into_value(self) -> Value {
        Value::Real(self)
    }
}
impl FromValue for f64 {
    fn from_value(value: Value) -> Result<Self, ValueError> {
        match value {
            Value::Real(r) => Ok(r),
            // SQLite hands back whole-number reals as integers.
            Value::Integer(i) => Ok(i as f64),
            other => Err(ValueError::wrong_type("real", &other)),
        }
    }
}

/// Stored as `Integer` 0 or 1.
impl IntoValue for bool {
    fn into_value(self) -> Value {
        Value::Integer(i64::from(self))
    }
}
impl FromValue for bool {
    fn from_value(value: Value) -> Result<Self, ValueError> {
        match value {
            Value::Integer(i) => Ok(i != 0),
            other => Err(ValueError::wrong_type("integer", &other)),
        }
    }
}

impl IntoValue for &str {
    fn into_value(self) -> Value {
        Value::Text(self.to_string())
    }
}
impl IntoValue for String {
    fn into_value(self) -> Value {
        Value::Text(self)
    }
}
impl FromValue for String {
    fn from_value(value: Value) -> Result<Self, ValueError> {
        match value {
            Value::Text(text) => Ok(text),
            other => Err(ValueError::wrong_type("text", &other)),
        }
    }
}

impl IntoValue for Vec<u8> {
    fn into_value(self) -> Value {
        Value::Blob(self)
    }
}
impl FromValue for Vec<u8> {
    fn from_value(value: Value) -> Result<Self, ValueError> {
        match value {
            Value::Blob(bytes) => Ok(bytes),
            other => Err(ValueError::wrong_type("blob", &other)),
        }
    }
}

/// `None` is stored as `Null`.
impl<T: IntoValue> IntoValue for Option<T> {
    fn into_value(self) -> Value {
        match self {
            Some(value) => value.into_value(),
            None => Value::Null,
        }
    }
}
impl<T: FromValue> FromValue for Option<T> {
    fn from_value(value: Value) -> Result<Self, ValueError> {
        match value {
            Value::Null => Ok(None),
            other => T::from_value(other).map(Some),
        }
    }
}

/// Borrowed values are cloned; pass by value to avoid the copy.
impl<T: IntoValue + Clone> IntoValue for &T {
    fn into_value(self) -> Value {
        self.clone().into_value()
    }
}

#[cfg(test)]
mod tests;
