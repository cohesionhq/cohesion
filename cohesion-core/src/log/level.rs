//! [`Level`], a record's severity, and parsing it from text.

use std::fmt;
use std::str::FromStr;

/// Severity of a record, ordered from least to most severe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Level {
    Debug,
    Info,
    #[serde(alias = "warning")]
    Warn,
    Error,
}

impl Level {
    /// Uppercase name used in log lines, e.g. `"WARN"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

/// Uppercase level name; honours width/padding flags (`{:<5}`).
impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}

/// Error returned when parsing an unknown [`Level`] name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseLevelError(pub String);

impl fmt::Display for ParseLevelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown log level '{}' (expected debug, info, warn or error)",
            self.0
        )
    }
}

impl std::error::Error for ParseLevelError {}

/// Case-insensitive: `debug`, `info`, `warn`/`warning`, `error`.
impl FromStr for Level {
    type Err = ParseLevelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "debug" => Ok(Self::Debug),
            "info" => Ok(Self::Info),
            "warn" | "warning" => Ok(Self::Warn),
            "error" => Ok(Self::Error),
            _ => Err(ParseLevelError(s.to_string())),
        }
    }
}
