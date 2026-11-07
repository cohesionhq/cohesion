//! [`Logger`], the global fan-out to adapters, and the free functions that
//! log through it.

use std::fmt;
use std::sync::OnceLock;
use std::time::SystemTime;

use super::level::Level;
use super::module::Module;
use super::record::{Adapter, Record};

/// Fans records out to a set of adapters; install one globally with [`Logger::install`].
#[derive(Default)]
pub struct Logger {
    adapters: Vec<Box<dyn Adapter>>,
}

static LOGGER: OnceLock<Logger> = OnceLock::new();

impl Logger {
    /// A logger with no adapters.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an adapter.
    pub fn with(mut self, adapter: impl Adapter) -> Self {
        self.adapters.push(Box::new(adapter));
        self
    }

    /// Installs the global logger. Second call is a no-op returning `Err(self)`.
    pub fn install(self) -> Result<(), Self> {
        LOGGER.set(self)
    }

    /// Sends `record` to every adapter of this logger (installed or not).
    pub fn dispatch(&self, record: &Record<'_>) {
        for adapter in &self.adapters {
            adapter.log(record);
        }
    }
}

/// Logs `message` at `level` through the installed logger; no-op if none is installed.
pub fn log(level: Level, module: Module, message: fmt::Arguments<'_>) {
    if let Some(logger) = LOGGER.get() {
        logger.dispatch(&Record {
            level,
            module,
            message,
            time: SystemTime::now(),
        });
    }
}

/// Logs at [`Level::Debug`].
pub fn debug(module: Module, message: fmt::Arguments<'_>) {
    log(Level::Debug, module, message);
}

/// Logs at [`Level::Info`].
pub fn info(module: Module, message: fmt::Arguments<'_>) {
    log(Level::Info, module, message);
}

/// Logs at [`Level::Warn`].
pub fn warn(module: Module, message: fmt::Arguments<'_>) {
    log(Level::Warn, module, message);
}

/// Logs at [`Level::Error`].
pub fn error(module: Module, message: fmt::Arguments<'_>) {
    log(Level::Error, module, message);
}
