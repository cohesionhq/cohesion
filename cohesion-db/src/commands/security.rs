use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{PairingOpener, SecurityEvent, SecurityEventKind, SecuritySettings};
use cohesion_core::protocol::{
    GetSecuritySettingsRequest, ListSecurityEventsRequest, SecurityEventPage,
    UpdateSecuritySettingsRequest,
};
use cohesion_core::{Client, FromRequest};

use crate::changed::{Changed, Invalidates};
use crate::commands::page::page_size;
use crate::retention::days_in_secs;
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, Found, Read, ReadableDatabase,
    SendNotificationCommand, Write,
};

/// The security settings' one row.
#[derive(FromRequest)]
#[request(GetSecuritySettingsRequest)]
pub struct GetSecuritySettingsCommand {}
impl DatabaseCommand for GetSecuritySettingsCommand {
    type Response = SecuritySettings;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<SecuritySettings> {
        ctx.db.select().one().found()
    }
}

/// Replaces the security settings, recording that they changed.
#[derive(FromRequest)]
#[request(UpdateSecuritySettingsRequest)]
pub struct UpdateSecuritySettingsCommand {
    pub pairing_opened_by: PairingOpener,
    pub alert_new_client: bool,
    pub revoke_inactive_after_days: Option<i64>,
}
impl DatabaseCommand for UpdateSecuritySettingsCommand {
    type Response = SecuritySettings;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<SecuritySettings>> {
        let settings: SecuritySettings = ctx
            .db
            .update()
            .set(SecuritySettings::PAIRING_OPENED_BY, self.pairing_opened_by)
            .set(SecuritySettings::ALERT_NEW_CLIENT, self.alert_new_client)
            .set(
                SecuritySettings::REVOKE_INACTIVE_AFTER_DAYS,
                self.revoke_inactive_after_days,
            )
            .returning_one()
            .found()?;
        record(ctx, SecurityEventKind::SettingsChanged, None, None)?;
        Ok(settings.invalidating([Invalidation::Security]))
    }
}

/// One page of the security activity, newest first.
#[derive(FromRequest)]
#[request(ListSecurityEventsRequest)]
pub struct ListSecurityEventPageCommand {
    pub offset: u64,
    pub limit: Option<u64>,
}
impl DatabaseCommand for ListSecurityEventPageCommand {
    type Response = SecurityEventPage;
    type Access = Read;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Read>,
    ) -> DatabaseCommandResult<SecurityEventPage> {
        Ok(SecurityEventPage {
            total: ctx.db.select::<SecurityEvent>().count()?,
            events: ctx
                .db
                .select()
                .order("created_at DESC, id DESC")
                .limit(page_size(self.limit))
                .offset(self.offset)
                .all()?,
        })
    }
}

/// Records something that happened at the security boundary, about
/// `client` if any, from `address` if it came over the network.
pub struct RecordSecurityEventCommand {
    pub kind: SecurityEventKind,
    pub client: Option<Client>,
    pub address: Option<String>,
}
impl DatabaseCommand for RecordSecurityEventCommand {
    type Response = ();
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<()>> {
        ctx.db
            .insert::<SecurityEvent>()
            .set(SecurityEvent::CREATED_AT, ctx.now)
            .set(SecurityEvent::KIND, self.kind)
            .set(SecurityEvent::CLIENT_ID, self.client.as_ref().map(|c| c.id))
            .set(
                SecurityEvent::CLIENT_NAME,
                self.client.map(|client| client.client_name),
            )
            .set(SecurityEvent::ADDRESS, self.address)
            .execute()?;
        Ok(().invalidating([Invalidation::Security]))
    }
}

/// How long security activity is kept, in days.
const SECURITY_EVENT_RETENTION_DAYS: i64 = 30;

/// Forgets security activity older than [`SECURITY_EVENT_RETENTION_DAYS`].
pub struct DeleteOldSecurityEventsCommand;
impl DatabaseCommand for DeleteOldSecurityEventsCommand {
    type Response = ();
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<()>> {
        let deleted = ctx
            .db
            .delete::<SecurityEvent>()
            .filter("created_at < ?")
            .bind(ctx.now - days_in_secs(SECURITY_EVENT_RETENTION_DAYS))
            .execute()?;
        Ok(().invalidating((deleted > 0).then_some(Invalidation::Security)))
    }
}

/// Records a security event inline, as part of the command running.
pub(crate) fn record(
    ctx: &mut CommandContext<'_>,
    kind: SecurityEventKind,
    client: Option<&Client>,
    address: Option<String>,
) -> DatabaseCommandResult<()> {
    ctx.run(RecordSecurityEventCommand {
        kind,
        client: client.cloned(),
        address,
    })
}

/// Sends the notification `message`, titled `title`, when `wanted` by the
/// security settings (`alert` picks which setting).
pub(crate) fn alert(
    ctx: &mut CommandContext<'_>,
    alert: fn(&SecuritySettings) -> bool,
    title: &str,
    message: String,
) -> DatabaseCommandResult<()> {
    let settings = GetSecuritySettingsCommand {}.handle(&mut ctx.as_read())?;
    if alert(&settings) {
        ctx.run(SendNotificationCommand {
            title: title.to_string(),
            message,
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
