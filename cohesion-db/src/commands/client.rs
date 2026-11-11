use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::SecurityEventKind;
use cohesion_core::protocol::{DeleteClientRequest, RenameClientRequest};
use cohesion_core::{Client, ClientKeyKind, FromRequest, LocalIdentity};

use super::security::{alert, record};
use crate::changed::{Changed, Invalidates};
use crate::retention::days_in_secs;
use crate::{
    CommandContext, CommandError, DatabaseCommand, DatabaseCommandResult, DatabaseEventKind,
    Filtered, Found, GetSecuritySettingsCommand, Read, ReadableDatabase, ValidationError, Write,
};

/// Every client that has not been deleted, by name. The clients list adds
/// whether each is online.
pub struct ListClientsCommand {}
impl DatabaseCommand for ListClientsCommand {
    type Response = Vec<Client>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Vec<Client>> {
        Ok(ctx
            .db
            .select_live()
            .order("client_name COLLATE NOCASE, id")
            .all()?)
    }
}

/// The live client whose certificate has this fingerprint, if any.
pub struct LookupClientByFingerprintCommand {
    pub fingerprint: String,
}
impl DatabaseCommand for LookupClientByFingerprintCommand {
    type Response = Option<Client>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Option<Client>> {
        Ok(ctx
            .db
            .select_live()
            .filter("key_kind = ?")
            .bind(ClientKeyKind::Certificate)
            .filter("key = ?")
            .bind(self.fingerprint)
            .one()?)
    }
}

/// Live client `id`; `NotFound` once it is deleted.
pub struct GetClientCommand {
    pub id: i64,
}
impl DatabaseCommand for GetClientCommand {
    type Response = Client;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Client> {
        ctx.db
            .select_live()
            .filter("id = ?")
            .bind(self.id)
            .one()
            .found()
    }
}

/// Trusts the certificate with `fingerprint`, presented from `address`,
/// provided the open pairing window's setup key is given; the key is spent
/// by the registration. A certificate already trusted answers its client. A
/// revoked certificate registers as a new client, since only live rows are
/// unique.
pub struct RegisterClientCommand {
    pub setup_key: String,
    pub fingerprint: String,
    pub client_name: String,
    pub address: String,
}
impl DatabaseCommand for RegisterClientCommand {
    type Response = Client;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Client>> {
        if !ctx.secrets.verify_setup_key(&self.setup_key) {
            return Err(CommandError::Unauthorized);
        }
        let client_name = valid_name(&self.client_name)?;
        let existing = LookupClientByFingerprintCommand {
            fingerprint: self.fingerprint.clone(),
        };
        let client = match existing.handle(&mut ctx.as_read())? {
            Some(client) => client.invalidating_nothing(),
            None => insert(
                ctx,
                ClientKeyKind::Certificate,
                self.fingerprint,
                client_name,
                Some(self.address),
            )?,
        };
        ctx.secrets.end_pairing();
        Ok(client)
    }
}

/// The client presenting `identity` on the Unix socket, whose file
/// permissions trust whoever connects: the one with its key, or a new one
/// named `identity.name` the first time the key is seen. A key whose client
/// was deleted is refused, so revoking a local client lasts until it makes a
/// new key.
pub struct IdentifyLocalClientCommand {
    pub identity: LocalIdentity,
}
impl DatabaseCommand for IdentifyLocalClientCommand {
    type Response = Client;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Client>> {
        let key = self.identity.fingerprint();
        let known: Option<Client> = ctx
            .db
            .select()
            .filter_eq(Client::KEY_KIND, ClientKeyKind::Local)
            .filter_eq(Client::KEY, &key)
            .one()?;
        match known {
            Some(client) if client.deleted => Err(ValidationError::new(
                "This client was revoked; run `cohesion setup --local` to connect again.",
            )
            .into()),
            Some(client) => Ok(client.invalidating_nothing()),
            None => {
                let name = valid_name(&self.identity.name)?;
                insert(ctx, ClientKeyKind::Local, key, name, None)
            }
        }
    }
}

/// Renames client `id`.
#[derive(FromRequest)]
#[request(RenameClientRequest)]
pub struct RenameClientCommand {
    pub id: i64,
    pub client_name: String,
}
impl DatabaseCommand for RenameClientCommand {
    type Response = Client;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Client>> {
        let client_name = valid_name(&self.client_name)?;
        let client: Client = ctx
            .db
            .update_live()
            .set(Client::CLIENT_NAME, client_name)
            .filter_eq(Client::ID, self.id)
            .returning_one()
            .found()?;
        record(ctx, SecurityEventKind::ClientRenamed, Some(&client), None)?;
        Ok(client.invalidating([Invalidation::Clients]))
    }
}

/// `name` trimmed, refused when nothing is left.
fn valid_name(name: &str) -> DatabaseCommandResult<&str> {
    match name.trim() {
        "" => Err(ValidationError::new("Client name can’t be empty.").into()),
        name => Ok(name),
    }
}

/// A new client identified by `key`, a fingerprint of `kind`, pairing
/// from `address` (`None` on this computer); recorded, and announced when
/// the settings ask. The announcement leaves the address out: behind a
/// router that forwards the port, it can be the router's.
fn insert(
    ctx: &mut CommandContext<'_>,
    kind: ClientKeyKind,
    key: String,
    client_name: &str,
    address: Option<String>,
) -> DatabaseCommandResult<Changed<Client>> {
    let client: Client = ctx
        .db
        .insert()
        .set(Client::KEY_KIND, kind)
        .set(Client::KEY, key)
        .set(Client::CLIENT_NAME, client_name)
        .set(Client::CREATED_AT, Some(ctx.now))
        .set(Client::LAST_SEEN_AT, Some(ctx.now))
        .set(Client::LAST_ADDRESS, address.clone())
        .returning_one()
        .found()?;
    let message = format!("{} paired with Cohesion.", client.client_name);
    record(ctx, SecurityEventKind::ClientPaired, Some(&client), address)?;
    alert(
        ctx,
        |settings| settings.alert_new_client,
        "New client paired",
        message,
    )?;
    Ok(client.invalidating([Invalidation::Clients]))
}

/// Soft-deletes a client, so its key is no longer trusted. The schema
/// soft-deletes the actors it offered, which keep their sessions.
#[derive(FromRequest)]
#[request(DeleteClientRequest)]
pub struct DeleteClientCommand {
    pub id: i64,
}
impl DatabaseCommand for DeleteClientCommand {
    type Response = Client;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Client>> {
        revoke(ctx, self.id, SecurityEventKind::ClientRevoked)
    }
}

/// Revokes every live client but `keep`, the one asking, as
/// [`DeleteClientCommand`] does each; answers the clients revoked.
pub struct RevokeOtherClientsCommand {
    pub keep: i64,
}
impl DatabaseCommand for RevokeOtherClientsCommand {
    type Response = Vec<Client>;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Vec<Client>>> {
        let others: Vec<i64> = ctx
            .db
            .select_id::<Client>()
            .live()
            .filter("id <> ?")
            .bind(self.keep)
            .all()?;
        revoke_all(ctx, others, SecurityEventKind::ClientRevoked)
    }
}

/// Revokes every live client, as [`DeleteClientCommand`] does each, then
/// pairs `clients` again as they were. An import runs it on the database it
/// brings in: that database's clients paired with another server's key, and
/// `clients`, the ones of the database it replaces, with this server's.
pub struct ReplaceClientsCommand {
    pub clients: Vec<Client>,
}
impl DatabaseCommand for ReplaceClientsCommand {
    type Response = ();
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<()>> {
        let every: Vec<i64> = ctx.db.select_id::<Client>().live().all()?;
        let revoked = revoke_all(ctx, every, SecurityEventKind::ClientRevoked)?;
        for client in self.clients {
            ctx.db
                .insert::<Client>()
                .set(Client::KEY_KIND, client.key_kind)
                .set(Client::KEY, client.key)
                .set(Client::CLIENT_NAME, client.client_name)
                .set(Client::CREATED_AT, client.created_at)
                .set(Client::LAST_SEEN_AT, client.last_seen_at)
                .set(Client::LAST_ADDRESS, client.last_address)
                .execute()?;
        }
        Ok(revoked.map(|_| ()))
    }
}

/// Revokes the clients that haven't connected for as long as the security
/// settings allow, if they set a limit, except the `online` ones, whose
/// connection may have lasted that long. A client never seen since pairing
/// was recorded counts from when it paired, and one with neither is left.
/// Answers the clients revoked.
pub struct RevokeInactiveClientsCommand {
    pub online: Vec<i64>,
}
impl DatabaseCommand for RevokeInactiveClientsCommand {
    type Response = Vec<Client>;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Vec<Client>>> {
        let settings = GetSecuritySettingsCommand {}.handle(&mut ctx.as_read())?;
        let Some(days) = settings.revoke_inactive_after_days else {
            return Ok(Vec::new().invalidating_nothing());
        };
        let inactive: Vec<i64> = ctx
            .db
            .select_id::<Client>()
            .live()
            .filter("coalesce(last_seen_at, created_at) < ?")
            .bind(ctx.now.saturating_sub(days_in_secs(days)))
            .filter_not_in_values("id", self.online)
            .all()?;
        revoke_all(ctx, inactive, SecurityEventKind::InactiveClientRevoked)
    }
}

/// Notes that client `id` connected now, from `address` (`None` on this
/// computer).
pub struct ClientSeenCommand {
    pub id: i64,
    pub address: Option<String>,
}
impl DatabaseCommand for ClientSeenCommand {
    type Response = ();
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<()>> {
        ctx.db
            .update_live::<Client>()
            .set(Client::LAST_SEEN_AT, Some(ctx.now))
            .set(Client::LAST_ADDRESS, self.address)
            .filter_eq(Client::ID, self.id)
            .execute()?;
        Ok(().invalidating([Invalidation::Clients]))
    }
}

/// Soft-deletes client `id`, recording why as `kind`, and raises
/// `client:revoke` with it.
fn revoke(
    ctx: &mut CommandContext<'_>,
    id: i64,
    kind: SecurityEventKind,
) -> DatabaseCommandResult<Changed<Client>> {
    let client: Client = ctx
        .db
        .update()
        .set_true(Client::DELETED)
        .filter_eq(Client::ID, id)
        .returning_one()
        .found()?;
    record(ctx, kind, Some(&client), None)?;
    ctx.events
        .send(ctx.trace.event(DatabaseEventKind::ClientRevoked {
            client_id: client.id,
            name: client.client_name.clone(),
        }));
    Ok(client.invalidating([Invalidation::Clients]))
}

/// Revokes each of `ids` as [`revoke`] does, answering them in order.
fn revoke_all(
    ctx: &mut CommandContext<'_>,
    ids: Vec<i64>,
    kind: SecurityEventKind,
) -> DatabaseCommandResult<Changed<Vec<Client>>> {
    let mut clients = Vec::new();
    let mut changed = ().invalidating_nothing();
    for id in ids {
        changed = changed.merge(revoke(ctx, id, kind)?.map(|client| clients.push(client)));
    }
    Ok(changed.map(|()| clients))
}

#[cfg(test)]
mod tests;
