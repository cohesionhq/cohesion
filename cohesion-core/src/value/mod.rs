//! A storage-agnostic value, so models can be read from and written to any
//! backend (SQLite today, the JS runtime's getters) without knowing about it.

mod column_value;
mod json;
mod row;
mod value_conversion;

pub use column_value::Value;
pub use json::Json;
pub use row::{Row, RowError, read_column};
pub use value_conversion::{FromValue, IntoValue, ValueError};
