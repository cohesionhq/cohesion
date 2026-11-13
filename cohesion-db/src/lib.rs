//! SQLite persistence: the connection (`Container`) with its query builders
//! and migrations, the command trait and how commands fail, the worker channel
//! that runs them (`DatabaseContext`), the events they raise and the
//! `InvalidationWatch` that waits in the server for what they change, the commands
//! that read and write models, the `WorkTracker` that counts the daemon's
//! work in flight so a shutdown can wait for it. Each command declares `Read` or
//! `Write` access; `Access` picks the view of the connection its context
//! holds (`ReadContainer` or `WriteContainer`), and reads on either come from
//! `ReadableDatabase`, so a `Read` command's context cannot write. Scripts hold
//! a `GuardedDatabaseContext`, which runs read commands always and write
//! commands only when given write access, except writes to the script's own
//! key/value store, which always run. Connector credentials are out of
//! every command's reach but the `CredentialManager`'s, which hands one only
//! to the extension serving its account and refreshes it once at a time.
//! `DatabaseContext::export` and the import methods beside it move a whole
//! database from one daemon to another.

// Lets `#[derive(FromRequest)]`'s `::cohesion_db::...` paths resolve inside this crate.
extern crate self as cohesion_db;

mod backup;
mod changed;
mod command_error;
pub mod commands;
mod container;
mod credential_manager;
mod database_command;
mod database_context;
mod database_error;
mod database_event;
mod db_macros;
mod guarded_database_context;
mod invalidation_watch;
mod migrations;
pub mod query;
mod retention;
mod sandbox_key;
mod secrets;
mod sql_value;
mod storage_areas;
#[cfg(test)]
mod test_support;
mod transfer;
mod work_tracker;

pub use changed::{Changed, Invalidates};
pub use command_error::{CommandError, DatabaseCommandResult, Found, ValidationError, searched};
pub use commands::*;
pub use container::{Container, ReadContainer, WriteContainer};
pub use credential_manager::{CredentialError, CredentialManager};
pub use database_command::{
    Access, AgentSettings, Area, CommandContext, DatabaseCommand, DbRequest, IntelligenceSettings,
    Notifications, Read, UserData, Write, WriteArea,
};
pub use database_context::DatabaseContext;
pub use database_error::DatabaseError;
pub use database_event::{
    DatabaseEvent, DatabaseEventKind, DatabaseEventListener, DatabaseEventTrace, EventContext,
};
pub use guarded_database_context::GuardedDatabaseContext;
pub use invalidation_watch::{InvalidationWatch, Watched};
pub use query::{Delete, Filtered, Insert, ReadableDatabase, Select, Update};
pub use sandbox_key::SandboxKey;
pub use secrets::Secrets;
pub use transfer::{StagedImport, TransferError, clear_transfers};
pub use work_tracker::{PendingWork, WorkGuard, WorkTracker};
