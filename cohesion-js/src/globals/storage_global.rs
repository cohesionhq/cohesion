use cohesion_db::{
    GetStorageEntryCommand, GuardedDatabaseContext, ListStorageKeysCommand, SandboxKey,
    SetStorageEntryCommand,
};
use rquickjs::{Ctx, Exception, JsLifetime, Value, class::Trace};

use cohesion_macros::js_global;

use super::global::Global;
use crate::traced_command::{read, throw_command_error};

/// The sandbox whose store the running job's `storage` reaches, stored as context
/// userdata by [`Sandbox::storing`](crate::Sandbox::storing) for the job's
/// duration. Without it, `storage` reaches the sandbox's extension's store.
#[derive(JsLifetime)]
pub(crate) struct CurrentStorage(pub SandboxKey);

#[derive(Trace, JsLifetime)]
#[rquickjs::class]
pub struct StorageGlobal {
    #[qjs(skip_trace)]
    database: GuardedDatabaseContext,
}

/// Your own key/value store, kept across runs, restarts and updates: an
/// extension's or an agent's, which no other script can read or change.
/// Values are anything `JSON.stringify` accepts and come back parsed. It
/// needs no permission, and writes work even where `cohesion` is read-only,
/// such as in a cue or a turn a cue started.
///
/// ```js
/// const seen = storage.get("seen") ?? [];
/// storage.set("seen", [...seen, task.id]);
/// ```
#[js_global("storage")]
#[rquickjs::methods]
impl StorageGlobal {
    /// The value stored under `key`, or `null` if there is none.
    #[ts(returns = "unknown")]
    pub fn get<'js>(&self, ctx: Ctx<'js>, key: String) -> rquickjs::Result<Value<'js>> {
        let sandbox = sandbox(&ctx, &self.database)?;
        match read(
            &ctx,
            &self.database,
            GetStorageEntryCommand { sandbox, key },
        )? {
            Some(value) => ctx.json_parse(value),
            None => Ok(Value::new_null(ctx)),
        }
    }

    /// Stores `value` under `key`, replacing what was there. `null` (or
    /// `undefined`) removes the key.
    pub fn set<'js>(
        &self,
        ctx: Ctx<'js>,
        key: String,
        #[ts(type = "unknown")] value: Value<'js>,
    ) -> rquickjs::Result<()> {
        let sandbox = sandbox(&ctx, &self.database)?;
        let value = if value.is_null() || value.is_undefined() {
            None
        } else {
            match ctx.json_stringify(value)? {
                Some(json) => Some(json.to_string()?),
                None => {
                    return Err(Exception::throw_type(
                        &ctx,
                        "A stored value must be something `JSON.stringify` accepts.",
                    ));
                }
            }
        };
        self.database
            .set_storage_blocking(SetStorageEntryCommand {
                sandbox,
                key,
                value,
            })
            .map_err(|error| throw_command_error(&ctx, error))
    }

    /// Removes `key`; does nothing if it isn't stored.
    pub fn remove<'js>(&self, ctx: Ctx<'js>, key: String) -> rquickjs::Result<()> {
        let sandbox = sandbox(&ctx, &self.database)?;
        self.database
            .set_storage_blocking(SetStorageEntryCommand {
                sandbox,
                key,
                value: None,
            })
            .map_err(|error| throw_command_error(&ctx, error))
    }

    /// Every stored key, in alphabetical order.
    pub fn keys<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Vec<String>> {
        let sandbox = sandbox(&ctx, &self.database)?;
        read(&ctx, &self.database, ListStorageKeysCommand { sandbox })
    }
}

/// The sandbox whose store the running script reaches: the job's agent's,
/// else the sandbox's extension's.
fn sandbox(ctx: &Ctx<'_>, database: &GuardedDatabaseContext) -> rquickjs::Result<SandboxKey> {
    if let Some(current) = ctx.userdata::<CurrentStorage>() {
        return Ok(current.0.clone());
    }
    match database.extension() {
        Some(identifier) => Ok(SandboxKey::Extension(identifier.to_string())),
        None => Err(Exception::throw_message(
            ctx,
            "`storage` is only available to extensions and agents.",
        )),
    }
}

impl Global for StorageGlobal {
    fn install<'a>(
        context: &Ctx<'a>,
        database: GuardedDatabaseContext,
    ) -> Result<(), rquickjs::Error> {
        context
            .globals()
            .set("storage", StorageGlobal { database })?;
        Ok(())
    }
}
