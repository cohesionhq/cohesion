//! [`ServerError`], why a command failed, as clients see it, and
//! [`ShuttingDown`], why new work is refused.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

/// Why a command failed; `field` names the offending input when there is one.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ServerError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

/// The kind of failure, for clients that branch on it.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TsWire)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    BadRequest,
    ValidationFailed,
    InternalError,
    UpstreamError,
    NotFound,
    Unauthorized,
}

impl ServerError {
    /// An error with no offending field.
    fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            field: None,
        }
    }

    /// The request was malformed or could not be carried out as asked.
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::BadRequest, message)
    }

    /// Input the caller can fix, optionally attributed to `field`.
    pub fn validation(field: Option<String>, message: impl Into<String>) -> Self {
        Self {
            field,
            ..Self::new(ErrorCode::ValidationFailed, message)
        }
    }

    /// The client may not do this: it was revoked.
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unauthorized, message)
    }

    /// Something went wrong on the server; details stay in its logs.
    pub fn internal() -> Self {
        Self::new(ErrorCode::InternalError, "An internal error occurred.")
    }
    /// Something went wrong on the server, but we can share it with the client.
    pub fn upstream(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::UpstreamError, message)
    }
}

/// New work refused because the daemon is shutting down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShuttingDown;

impl std::fmt::Display for ShuttingDown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Cohesion is shutting down. Try again once it has restarted.")
    }
}

impl std::error::Error for ShuttingDown {}

impl From<ShuttingDown> for ServerError {
    fn from(shutting_down: ShuttingDown) -> Self {
        Self::bad_request(shutting_down.to_string())
    }
}
