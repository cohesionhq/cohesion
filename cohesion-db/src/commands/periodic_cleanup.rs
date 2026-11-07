use crate::changed::{Changed, Invalidates};
use crate::commands::intelligence_agent_message::BlankOldCodeResultsCommand;
use crate::commands::rewind::{DeleteOldVersionsCommand, PurgeDeletedItemsCommand};
use crate::{
    BlankOldThreadMessagesCommand, CommandContext, DatabaseCommand, DatabaseCommandResult,
    DeleteIdleThreadsCommand, DeleteOldActorSessionsCommand, DeleteOldSecurityEventsCommand,
    GetStorageSettingsCommand, Write,
};

/// Removes what has outlived its use: model threads gone idle, the contents
/// of old thread messages and script results, finished actor sessions and their
/// images, old security activity, old Rewind versions and items deleted long
/// ago, kept as long as the storage settings say, security activity for its
/// own fixed time. The daemon runs it on start and every hour after, then
/// vacuums if it freed enough
/// ([`DatabaseContext::vacuum_if_reclaimable`](crate::DatabaseContext)).
/// A new cleanup is one more `ctx.run` here; all run in one transaction.
pub struct PeriodicCleanupCommand;
impl DatabaseCommand for PeriodicCleanupCommand {
    type Response = ();
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<()>> {
        let settings = GetStorageSettingsCommand {}.handle(&mut ctx.as_read())?;
        let threads = settings.thread_retention_days;
        ctx.run(DeleteIdleThreadsCommand { days: threads })?;
        ctx.run(BlankOldThreadMessagesCommand { days: threads })?;
        ctx.run(BlankOldCodeResultsCommand { days: threads })?;
        ctx.run(DeleteOldActorSessionsCommand {
            days: settings.actor_session_retention_days,
        })?;
        ctx.run(DeleteOldSecurityEventsCommand)?;
        ctx.run(DeleteOldVersionsCommand {
            days: settings.rewind_retention_days,
        })?;
        ctx.run(PurgeDeletedItemsCommand {
            days: settings
                .deleted_item_retention_days
                .max(settings.rewind_retention_days),
        })?;
        Ok(().invalidating_nothing())
    }
}

#[cfg(test)]
mod tests;
