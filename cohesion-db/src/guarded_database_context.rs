//! The database handle scripts hold: read commands always run, write commands
//! only in the areas the sandbox was given, except to the script's own
//! store, and credentials come from the credential manager.

use std::sync::Arc;

use crate::command_error::DatabaseCommandResult;
use crate::commands::SetStorageEntryCommand;
use crate::credential_manager::CredentialManager;
use crate::database_command::{Area, DatabaseCommand, Read, Write, WriteArea};
use crate::database_context::DatabaseContext;
use crate::database_event::DatabaseEventTrace;

/// A database handle for scripts: reads always, writes only to the
/// [`WriteArea`]s the sandbox was given. Private fields keep the raw handle
/// out of reach.
#[derive(Debug, Clone)]
pub struct GuardedDatabaseContext {
    database: DatabaseContext,
    areas: Arc<[WriteArea]>,
    extension: Option<Arc<String>>,
}

impl GuardedDatabaseContext {
    pub(crate) fn new(database: DatabaseContext, areas: Arc<[WriteArea]>) -> Self {
        Self {
            database,
            areas,
            extension: None,
        }
    }

    /// The same handle, attributing its writes to the extension `identifier`
    /// so that extension's own hooks don't fire for them.
    pub fn for_extension(mut self, identifier: Arc<String>) -> Self {
        self.extension = Some(identifier);
        self
    }

    /// The script identifier of the extension this handle belongs to, if
    /// any.
    pub fn extension(&self) -> Option<&Arc<String>> {
        self.extension.as_ref()
    }

    /// The same handle, writing only those of `areas` it already could.
    pub fn narrowed(&self, areas: &[WriteArea]) -> Self {
        Self {
            areas: areas
                .iter()
                .copied()
                .filter(|a| self.may_write(*a))
                .collect(),
            ..self.clone()
        }
    }

    /// Whether write commands of `area` run through this handle.
    pub fn may_write(&self, area: WriteArea) -> bool {
        self.areas.contains(&area)
    }

    /// The same handle without write access.
    pub fn read_only(&self) -> Self {
        Self {
            areas: Arc::new([]),
            ..self.clone()
        }
    }

    /// Runs a read command and waits for its result. Panics if called from
    /// inside an async runtime.
    pub fn send_read_blocking<C: DatabaseCommand<Access = Read>>(
        &self,
        cmd: C,
    ) -> DatabaseCommandResult<C::Response> {
        self.database.send_blocking(cmd)
    }

    /// The credentials the daemon keeps for extensions' accounts, whether or
    /// not this guard may write: a refresh is stored either way.
    pub fn credentials(&self) -> CredentialManager<'_> {
        self.database.credentials()
    }

    /// Writes to a script's own store, whether or not this guard may write:
    /// the store is private to its owner, so a read-only script keeps its
    /// state too.
    pub fn set_storage_blocking(&self, cmd: SetStorageEntryCommand) -> DatabaseCommandResult<()> {
        self.database.send_blocking(cmd)
    }

    /// `trace`, attributed to this guard's extension if it has one.
    fn attributed(&self, trace: DatabaseEventTrace) -> DatabaseEventTrace {
        match &self.extension {
            Some(identifier) => trace.from_extension(identifier.clone()),
            None => trace,
        }
    }

    /// Runs a write command and waits for its result. `None` when the guard
    /// may not write the command's area.
    pub async fn send_write<A: Area, C: DatabaseCommand<Access = Write<A>>>(
        &self,
        cmd: C,
    ) -> Option<DatabaseCommandResult<C::Response>> {
        if !self.may_write(A::AREA) {
            return None;
        }
        let trace = self.attributed(DatabaseEventTrace::default());
        Some(self.database.send_traced(cmd, trace).await)
    }

    /// Runs a write command whose events continue `trace`, attributed to this
    /// guard's extension if it has one, and waits for its result. `None` when
    /// the guard may not write the command's area.
    pub fn send_write_blocking_traced<A: Area, C: DatabaseCommand<Access = Write<A>>>(
        &self,
        cmd: C,
        trace: DatabaseEventTrace,
    ) -> Option<DatabaseCommandResult<C::Response>> {
        let trace = self.attributed(trace);
        self.may_write(A::AREA)
            .then(|| self.database.send_blocking_traced(cmd, trace))
    }
}

#[cfg(test)]
mod tests;
