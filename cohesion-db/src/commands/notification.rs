use cohesion_core::invalidation::Invalidation;

use crate::changed::{Changed, Invalidates};
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, DatabaseEventKind, Notifications, Write,
};

/// Sends the user a notification. It stores nothing and runs no SQL, so it
/// takes no lock: it raises `notification:send` for the extensions that push
/// it to the user's devices, and an invalidation carrying it for clients to
/// show.
pub struct SendNotificationCommand {
    pub title: String,
    pub message: String,
}
impl DatabaseCommand for SendNotificationCommand {
    type Response = ();
    type Access = Write<Notifications>;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Write<Notifications>>,
    ) -> DatabaseCommandResult<Changed<()>> {
        ctx.events
            .send(ctx.trace.event(DatabaseEventKind::NotificationSent {
                title: self.title.clone(),
                message: self.message.clone(),
            }));
        Ok(().invalidating([Invalidation::Notification {
            title: self.title,
            message: self.message,
        }]))
    }
}
