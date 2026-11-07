use crate::changed::{Changed, Invalidates};
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, SettleActivitiesCommand,
    SettleActorSessionsCommand, SettleAgentActivityCommand, SettleAgentAsksCommand,
    SettleAgentThreadMarksCommand, Write,
};

/// Leaves the database as a stopped daemon should: no work marked as
/// running, since none is. [`DatabaseContext::close`](crate::DatabaseContext::close)
/// runs it as the database's last write, and the daemon's start runs it
/// again for a daemon that stopped without closing (a crash, `kill -9`).
/// A new cleanup is one more `ctx.run` here; all run in one transaction.
pub struct ShutdownCleanupCommand;
impl DatabaseCommand for ShutdownCleanupCommand {
    type Response = ();
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<()>> {
        ctx.run(SettleAgentThreadMarksCommand)?;
        ctx.run(SettleAgentActivityCommand)?;
        ctx.run(SettleActivitiesCommand)?;
        ctx.run(SettleActorSessionsCommand)?;
        ctx.run(SettleAgentAsksCommand)?;
        Ok(().invalidating_nothing())
    }
}
