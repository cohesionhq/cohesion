//! The one way to a connector account's credential: read for the extension
//! that serves the account only, and refreshed under a lock per account so
//! one refresh token is never spent twice.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use cohesion_core::model::Credential;

use crate::command_error::CommandError;
use crate::commands::{
    FailConnectorCredentialCommand, GetConnectorCredentialCommand, UpdateConnectorCredentialCommand,
};
use crate::database_context::DatabaseContext;

/// A lock per account id, shared by every clone of a [`DatabaseContext`].
#[derive(Debug, Default)]
pub(crate) struct CredentialLocks(Mutex<HashMap<i64, Arc<Mutex<()>>>>);

impl CredentialLocks {
    /// Account `account_id`'s lock. Never removed, so every caller of an id
    /// shares one.
    fn account(&self, account_id: i64) -> Arc<Mutex<()>> {
        self.0
            .lock()
            .unwrap()
            .entry(account_id)
            .or_default()
            .clone()
    }
}

thread_local! {
    /// Whether this thread is running a refresh, during which asking for a
    /// credential would wait on itself.
    static REFRESHING: Cell<bool> = const { Cell::new(false) };
}

/// Marks this thread as refreshing until dropped, even by a panic.
#[must_use = "the thread stops counting as refreshing when this is dropped"]
struct Refreshing;
impl Refreshing {
    fn start() -> Self {
        REFRESHING.set(true);
        Self
    }
}
impl Drop for Refreshing {
    fn drop(&mut self) {
        REFRESHING.set(false);
    }
}

/// Why [`CredentialManager::fresh`] handed out no credential.
#[derive(Debug)]
pub enum CredentialError<E> {
    /// Reading or storing it failed, or the extension doesn't serve the
    /// account.
    Command(CommandError),
    /// Asked for while this thread was refreshing a credential.
    Reentrant,
    /// The refresh failed; nothing was stored.
    Refresh(E),
}

impl<E> From<CommandError> for CredentialError<E> {
    fn from(error: CommandError) -> Self {
        Self::Command(error)
    }
}

/// Hands out connector accounts' credentials. Callers that find the same
/// credential expiring take turns: the first refreshes it, the rest get what
/// it stored.
#[derive(Clone, Copy)]
pub struct CredentialManager<'a> {
    database: &'a DatabaseContext,
    locks: &'a CredentialLocks,
}

impl<'a> CredentialManager<'a> {
    pub(crate) fn new(database: &'a DatabaseContext, locks: &'a CredentialLocks) -> Self {
        Self { database, locks }
    }

    /// The credential of account `account_id`, provided extension
    /// `extension_id` serves it. Within a minute of its expiry it is first
    /// replaced by what `refresh` returns for it, which is stored. A refresh
    /// that fails flags the account as needing to be signed in again; a
    /// credential stored later clears the flag. Blocks, so
    /// call it from a script worker, never from inside an async runtime.
    pub fn fresh<E>(
        &self,
        extension_id: i64,
        account_id: i64,
        refresh: impl FnOnce(&Credential) -> Result<Credential, E>,
    ) -> Result<Credential, CredentialError<E>> {
        if REFRESHING.get() {
            return Err(CredentialError::Reentrant);
        }
        let lock = self.locks.account(account_id);
        // The lock guards no data, so one a panicking refresh poisoned is
        // still sound.
        let _turn = lock.lock().unwrap_or_else(PoisonError::into_inner);
        let credential = self.database.send_blocking(GetConnectorCredentialCommand {
            extension_id,
            account_id,
        })?;
        let now = crate::database_context::unix_now();
        if !credential.needs_refresh(now) {
            return Ok(credential);
        }
        let refreshed = {
            let _refreshing = Refreshing::start();
            match refresh(&credential) {
                Ok(refreshed) => refreshed,
                Err(e) => {
                    // The failure the caller gets matters more than a failed
                    // flag write, which the next failed refresh repeats.
                    let _ = self.database.send_blocking(FailConnectorCredentialCommand {
                        extension_id,
                        account_id,
                    });
                    return Err(CredentialError::Refresh(e));
                }
            }
        };
        Ok(self
            .database
            .send_blocking(UpdateConnectorCredentialCommand {
                extension_id,
                account_id,
                credential: refreshed,
            })?)
    }
}

#[cfg(test)]
mod tests;
