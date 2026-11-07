//! An image's bytes: a BLOB in the database, base64 in JSON.

use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::ts_wire::TsWire;
use crate::value::{FromValue, IntoValue, Value, ValueError};

/// An image's bytes. Base64 only ever exists on the wire and in what a
/// provider is sent, so the one rule for it lives here, and the database,
/// which would pay a third again for it, holds the bytes.
#[derive(Clone, PartialEq, Eq)]
pub struct ImageBytes(Vec<u8>);

impl ImageBytes {
    /// The image base64-encoded `text` holds, or why it holds none.
    pub fn from_base64(text: &str) -> Result<Self, base64::DecodeError> {
        STANDARD.decode(text).map(Self)
    }

    /// The image's bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }

    /// The image, base64-encoded.
    pub fn to_base64(&self) -> String {
        STANDARD.encode(&self.0)
    }
}

impl std::fmt::Debug for ImageBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ImageBytes({} bytes)", self.0.len())
    }
}

impl Serialize for ImageBytes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_base64())
    }
}

impl<'de> Deserialize<'de> for ImageBytes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::from_base64(&text).map_err(serde::de::Error::custom)
    }
}

impl TsWire for ImageBytes {
    fn ts_type() -> String {
        "string".to_string()
    }
}

impl IntoValue for ImageBytes {
    fn into_value(self) -> Value {
        Value::Blob(self.0)
    }
}

impl FromValue for ImageBytes {
    fn from_value(value: Value) -> Result<Self, ValueError> {
        match value {
            Value::Blob(bytes) => Ok(Self(bytes)),
            other => Err(ValueError::wrong_type("blob", &other)),
        }
    }
}
