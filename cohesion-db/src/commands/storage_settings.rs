use cohesion_core::FromRequest;
use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::StorageSettings;
use cohesion_core::protocol::{
    DatabaseSize, GetDatabaseSizeRequest, GetStorageSettingsRequest, UpdateStorageSettingsRequest,
};

use crate::changed::{Changed, Invalidates};
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, Found, Read, ReadableDatabase, Write,
};

/// The storage settings' one row.
#[derive(FromRequest)]
#[request(GetStorageSettingsRequest)]
pub struct GetStorageSettingsCommand {}
impl DatabaseCommand for GetStorageSettingsCommand {
    type Response = StorageSettings;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<StorageSettings> {
        ctx.db.select().one().found()
    }
}

/// Replaces the storage settings. The next cleanup applies the retentions;
/// memories past a lowered limit stay until forgotten.
#[derive(FromRequest)]
#[request(UpdateStorageSettingsRequest)]
pub struct UpdateStorageSettingsCommand {
    pub thread_retention_days: i64,
    pub actor_session_retention_days: i64,
    pub rewind_retention_days: i64,
    pub deleted_item_retention_days: i64,
    pub memory_limit: i64,
}
impl DatabaseCommand for UpdateStorageSettingsCommand {
    type Response = StorageSettings;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<StorageSettings>> {
        let settings: StorageSettings = ctx
            .db
            .update()
            .set(
                StorageSettings::THREAD_RETENTION_DAYS,
                self.thread_retention_days,
            )
            .set(
                StorageSettings::ACTOR_SESSION_RETENTION_DAYS,
                self.actor_session_retention_days,
            )
            .set(
                StorageSettings::REWIND_RETENTION_DAYS,
                self.rewind_retention_days,
            )
            .set(
                StorageSettings::DELETED_ITEM_RETENTION_DAYS,
                self.deleted_item_retention_days,
            )
            .set(StorageSettings::MEMORY_LIMIT, self.memory_limit)
            .returning_one()
            .found()?;
        Ok(settings.invalidating([Invalidation::Storage]))
    }
}

/// How large the database file is, and how much of it is free.
#[derive(FromRequest)]
#[request(GetDatabaseSizeRequest)]
pub struct GetDatabaseSizeCommand {}
impl DatabaseCommand for GetDatabaseSizeCommand {
    type Response = DatabaseSize;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<DatabaseSize> {
        Ok(ctx.db.container().size()?)
    }
}

#[cfg(test)]
mod tests;
