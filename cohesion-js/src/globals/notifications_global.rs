use cohesion_db::{GuardedDatabaseContext, SendNotificationCommand};
use rquickjs::{Ctx, JsLifetime, class::Trace};

use cohesion_macros::js_global;

use super::global::Global;
use crate::traced_command::write;

#[derive(Trace, JsLifetime)]
#[rquickjs::class]
pub struct NotificationsGlobal {
    #[qjs(skip_trace)]
    database: GuardedDatabaseContext,
}

/// Sends the user notifications, which extensions hooked to
/// `notification:send` push to their devices. An extension needs the
/// `notifications` permission.
///
/// Cohesion keeps no copy: a notification nothing is hooked to goes nowhere.
/// An agent's messages may already notify the user (an extension can push
/// every `agent:message`), so an agent sends a notification only for
/// something that needs the user's attention now, never to repeat a message
/// it sent.
///
/// ```js
/// notifications.send("Rent is due", "The rent task is due today.");
/// ```
#[js_global("notifications")]
#[rquickjs::methods]
impl NotificationsGlobal {
    /// Sends the user a notification with a short `title` and a `message`.
    pub fn send<'js>(&self, ctx: Ctx<'js>, title: String, message: String) -> rquickjs::Result<()> {
        write(
            &ctx,
            &self.database,
            SendNotificationCommand { title, message },
        )
    }
}

impl Global for NotificationsGlobal {
    fn install<'a>(
        context: &Ctx<'a>,
        database: GuardedDatabaseContext,
    ) -> Result<(), rquickjs::Error> {
        context
            .globals()
            .set("notifications", NotificationsGlobal { database })?;
        Ok(())
    }
}
