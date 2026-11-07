//! The built-in adapter that writes human-readable lines to the terminal.

use std::io::Write;

use super::level::Level;
use super::record::{Adapter, Record};
use super::time::Timestamp;

/// Writes records at or above `min_level`: Debug/Info to stdout, Warn/Error to stderr.
pub struct ConsoleAdapter {
    min_level: Level,
}

impl ConsoleAdapter {
    /// Creates a console adapter that drops records below `min_level`.
    pub fn new(min_level: Level) -> Self {
        Self { min_level }
    }

    /// Whether a record at `level` passes this adapter's filter.
    pub fn should_log(&self, level: Level) -> bool {
        level >= self.min_level
    }

    /// Renders one line (without trailing newline), e.g.
    /// `2026-09-29T14:03:12Z INFO  [http] message`.
    pub fn format(record: &Record<'_>) -> String {
        format!(
            "{} {:<5} [{}] {}",
            Timestamp(record.time),
            record.level,
            record.module,
            record.message
        )
    }
}

impl Adapter for ConsoleAdapter {
    fn log(&self, record: &Record<'_>) {
        if !self.should_log(record.level) {
            return;
        }
        let mut line = Self::format(record);
        line.push('\n');
        // Write in one call so concurrent lines don't interleave; ignore closed pipes.
        let _ = if record.level >= Level::Warn {
            std::io::stderr().lock().write_all(line.as_bytes())
        } else {
            std::io::stdout().lock().write_all(line.as_bytes())
        };
    }
}
