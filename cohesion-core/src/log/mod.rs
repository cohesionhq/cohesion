//! Stateless logging: every line says which module emitted it, and nothing
//! else is carried between calls. Records fan out to the [`Adapter`]s of a
//! globally installed [`Logger`]; with none installed, logging is a no-op.
//!
//! ```ignore
//! cohesion_core::log_info!(Module::Http, "{caller} opened document {}", id);
//! ```

mod console;
mod level;
mod logger;
mod module;
mod record;
mod time;

pub use console::ConsoleAdapter;
pub use level::{Level, ParseLevelError};
pub use logger::{Logger, debug, error, info, log, warn};
pub use module::Module;
pub use record::{Adapter, Record};
pub use time::Timestamp;
