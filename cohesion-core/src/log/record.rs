//! [`Record`], one log event, and the [`Adapter`]s that receive it.

use std::fmt;
use std::time::SystemTime;

use super::level::Level;
use super::module::Module;

/// One log event as handed to adapters.
pub struct Record<'a> {
    /// Severity.
    pub level: Level,
    /// Subsystem the line comes from.
    pub module: Module,
    /// The formatted message, rendered lazily by each adapter.
    pub message: fmt::Arguments<'a>,
    /// When the record was created.
    pub time: SystemTime,
}

/// A destination for log records (console, file, remote sink, ...).
pub trait Adapter: Send + Sync + 'static {
    /// Handles one record; adapters do their own level filtering.
    fn log(&self, record: &Record<'_>);
}
