//! The private key/value stores of extensions and agents, one per
//! [`SandboxKey`]. Scripts reach their own through the `storage` global,
//! which names the sandbox itself, so no script can reach another's.

use cohesion_core::model::{Extension, StorageEntry, StorageOwner};

use crate::changed::{Changed, Invalidates};
use crate::{
    CommandContext, CommandError, DatabaseCommand, DatabaseCommandResult, Filtered, Read,
    ReadableDatabase, SandboxKey, ValidationError, Write,
};

/// The most bytes of JSON one value in a store may hold.
pub const MAX_STORAGE_VALUE_BYTES: usize = 1024 * 1024;
/// The most bytes of keys and values one store may hold in all.
pub const MAX_STORAGE_BYTES: usize = 16 * 1024 * 1024;

/// Who owns `sandbox`'s store: its agent, or the installed extension with its
/// script identifier, if there is one.
fn owner<'c>(
    db: impl ReadableDatabase<'c>,
    sandbox: &SandboxKey,
) -> DatabaseCommandResult<Option<StorageOwner>> {
    Ok(match sandbox {
        SandboxKey::Agent(agent_id) => Some(StorageOwner::Agent {
            agent_id: *agent_id,
        }),
        SandboxKey::Extension(identifier) => {
            let extension: Option<i64> = db
                .select_column::<Extension, i64>("id")
                .filter("script_identifier = ?")
                .bind(identifier.clone())
                .one()?;
            extension.map(|extension_id| StorageOwner::Extension { extension_id })
        }
    })
}

/// The JSON stored under `key` in `sandbox`'s store, if any.
pub struct GetStorageEntryCommand {
    pub sandbox: SandboxKey,
    pub key: String,
}
impl DatabaseCommand for GetStorageEntryCommand {
    type Response = Option<String>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Option<String>> {
        let Some(owner) = owner(ctx.db, &self.sandbox)? else {
            return Ok(None);
        };
        let entry: Option<StorageEntry> = ctx
            .db
            .select()
            .filter_group(owner)
            .filter_eq(StorageEntry::KEY, self.key)
            .one()?;
        Ok(entry.map(|entry| entry.value))
    }
}

/// Every key in `sandbox`'s store, in order.
pub struct ListStorageKeysCommand {
    pub sandbox: SandboxKey,
}
impl DatabaseCommand for ListStorageKeysCommand {
    type Response = Vec<String>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Vec<String>> {
        let Some(owner) = owner(ctx.db, &self.sandbox)? else {
            return Ok(Vec::new());
        };
        let entries: Vec<StorageEntry> = ctx.db.select().filter_group(owner).order("key").all()?;
        Ok(entries.into_iter().map(|entry| entry.key).collect())
    }
}

/// Stores the JSON `value` under `key` in `sandbox`'s store, replacing any
/// earlier value; `None` removes the key. Refused (as a validation error)
/// when the value is over [`MAX_STORAGE_VALUE_BYTES`] or the store would
/// pass [`MAX_STORAGE_BYTES`].
pub struct SetStorageEntryCommand {
    pub sandbox: SandboxKey,
    pub key: String,
    pub value: Option<String>,
}
impl DatabaseCommand for SetStorageEntryCommand {
    type Response = ();
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<()>> {
        let owner = owner(ctx.db.read(), &self.sandbox)?.ok_or_else(|| {
            ValidationError::new("Only an installed extension or an agent has a store.")
        })?;
        match self.value {
            Some(value) => {
                if value.len() > MAX_STORAGE_VALUE_BYTES {
                    return Err(CommandError::Validation(ValidationError::new(
                        "A stored value may be at most 1 MiB.",
                    )));
                }
                let others: Vec<StorageEntry> = ctx
                    .db
                    .select()
                    .filter_group(owner)
                    .filter("key != ?")
                    .bind(self.key.clone())
                    .all()?;
                let stored: usize = others
                    .iter()
                    .map(|entry| entry.key.len() + entry.value.len())
                    .sum();
                if stored + self.key.len() + value.len() > MAX_STORAGE_BYTES {
                    return Err(CommandError::Validation(ValidationError::new(
                        "Storage is full: it holds at most 16 MiB in all. Remove what you no longer need.",
                    )));
                }
                ctx.db
                    .delete::<StorageEntry>()
                    .filter_group(owner)
                    .filter_eq(StorageEntry::KEY, self.key.clone())
                    .execute()?;
                ctx.db
                    .insert::<StorageEntry>()
                    .set_group(owner)
                    .set(StorageEntry::KEY, self.key)
                    .set(StorageEntry::VALUE, value)
                    .execute()?;
            }
            None => {
                ctx.db
                    .delete::<StorageEntry>()
                    .filter_group(owner)
                    .filter_eq(StorageEntry::KEY, self.key)
                    .execute()?;
            }
        }
        Ok(().invalidating_nothing())
    }
}

#[cfg(test)]
mod tests;
