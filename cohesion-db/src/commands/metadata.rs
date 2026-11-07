//! Script-attached key/value pairs on an item ([`MetadataOwner`]). Lookups by
//! metadata are `Select::with_metadata` (pairs) and
//! `Select::with_metadata_keys` (keys alone) on each model's list command.

use cohesion_core::model::{Metadata, MetadataOwner};

use crate::changed::{Changed, Invalidates};
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, Filtered, Read, ReadableDatabase, Write,
};

/// The value stored under `key` on an item, if any.
pub struct GetMetadataCommand {
    pub owner: MetadataOwner,
    pub key: String,
}
impl DatabaseCommand for GetMetadataCommand {
    type Response = Option<String>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Option<String>> {
        let metadata: Option<Metadata> = ctx
            .db
            .select()
            .filter_group(self.owner)
            .filter_eq(Metadata::KEY, self.key)
            .one()?;
        Ok(metadata.map(|metadata| metadata.value))
    }
}

/// Stores `value` under `key` on an item, replacing any earlier value; `None`
/// removes the key.
pub struct SetMetadataCommand {
    pub owner: MetadataOwner,
    pub key: String,
    pub value: Option<String>,
}
impl DatabaseCommand for SetMetadataCommand {
    type Response = ();
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<()>> {
        ctx.db
            .delete::<Metadata>()
            .filter_group(self.owner)
            .filter_eq(Metadata::KEY, self.key.clone())
            .execute()?;
        if let Some(value) = self.value {
            ctx.db
                .insert::<Metadata>()
                .set_group(self.owner)
                .set(Metadata::KEY, self.key)
                .set(Metadata::VALUE, value)
                .execute()?;
        }
        Ok(().invalidating_nothing())
    }
}

#[cfg(test)]
mod tests;
