use std::collections::{HashMap, HashSet};

use cohesion_core::Named;
use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{
    AccountHost, Connector, ConnectorAccount, ConnectorDefinition, Credential, ExtensionPermission,
    ExtensionPermissionKind, Resource,
};
use cohesion_core::protocol::{
    AcceptedResponse, DeleteConnectorAccountRequest, ListConnectorAccountsRequest,
    MigrateResourcesRequest, RedactedConnectorAccount,
};
use cohesion_core::{FromRequest, Model, Projection};

use crate::changed::{Changed, Invalidates};
use crate::commands::connector::upsert_connector;
use crate::{
    CommandContext, CommandError, DatabaseCommand, DatabaseCommandResult, Filtered, Found, Read,
    ReadableDatabase, Write,
};

/// Every connector account, without credentials, by description and then
/// by connector title (both case-insensitive).
#[derive(FromRequest)]
#[request(ListConnectorAccountsRequest)]
pub struct ListConnectorAccountsCommand {}
impl DatabaseCommand for ListConnectorAccountsCommand {
    type Response = Vec<RedactedConnectorAccount>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        let accounts = ListConnectorAccountRowsCommand {}.handle(ctx)?;
        let redaction = Redaction::load(ctx.db, &accounts)?;
        let mut accounts: Vec<_> = accounts
            .into_iter()
            .map(|account| redaction.redact(account))
            .collect();
        accounts.sort_by_cached_key(|account| {
            (
                account.description.to_lowercase(),
                account.connector.title.to_lowercase(),
                account.id,
            )
        });
        Ok(accounts)
    }
}

/// Whether an extension is granted `connector`, read with its permission
/// row (an extension without one isn't).
#[derive(Projection)]
#[projection(of = ExtensionPermission)]
struct ConnectorGrant {
    extension_id: i64,
    enabled: bool,
}

/// A connector account's whole row, credential included, for the commands
/// that write it: `ConnectorAccount` leaves the credential out so nothing
/// else can reach it.
#[derive(Model)]
#[model(table = "connector_accounts", backup = Included)]
#[allow(dead_code)] // Written through the builders; never read back whole.
struct AccountRow {
    id: i64,
    connector_id: i64,
    #[model(group)]
    host: AccountHost,
    description: Option<String>,
    authorization: Option<String>,
    refresh: Option<String>,
    expires_at: Option<i64>,
    credential_failed_at: Option<i64>,
}

/// The credential columns of an extension's account.
#[derive(Projection)]
#[projection(of = AccountRow)]
struct StoredCredential {
    authorization: Option<String>,
    refresh: Option<String>,
    expires_at: Option<i64>,
    credential_failed_at: Option<i64>,
}

/// What a client sees of some accounts besides their rows: each one's
/// connector, and whether each extension serving one is granted
/// `connector`, loaded once per command rather than once per account.
struct Redaction {
    connectors: HashMap<i64, Connector>,
    granted: HashSet<i64>,
}
impl Redaction {
    fn load<'c>(
        db: impl ReadableDatabase<'c>,
        accounts: &[ConnectorAccount],
    ) -> DatabaseCommandResult<Self> {
        let connectors = db
            .select::<Connector>()
            .filter_in_values("id", accounts.iter().map(|account| account.connector_id))
            .all()?
            .into_iter()
            .map(|connector| (connector.id, connector))
            .collect();
        let granted = db
            .select_as::<ConnectorGrant>()
            .filter("permission = ?")
            .bind(ExtensionPermissionKind::Connector)
            .filter_in_values(
                "extension_id",
                accounts
                    .iter()
                    .filter_map(|account| account.host.extension_id()),
            )
            .all()?
            .into_iter()
            .filter(|grant| grant.enabled)
            .map(|grant| grant.extension_id)
            .collect();
        Ok(Self {
            connectors,
            granted,
        })
    }

    /// What a client sees of `account`: no credential, its connector, and
    /// whether its items can be imported: an extension's when the extension
    /// is granted `connector`, an orphan's never.
    fn redact(&self, account: ConnectorAccount) -> RedactedConnectorAccount {
        let importable = match account.host {
            AccountHost::Extension { extension_id, .. } => self.granted.contains(&extension_id),
            AccountHost::Orphaned => false,
        };
        // The foreign key guarantees the connector's row.
        let connector = self.connectors[&account.connector_id].clone();
        RedactedConnectorAccount::new(account, connector, importable)
    }
}

/// `account` as a client sees it.
fn redacted<'c>(
    db: impl ReadableDatabase<'c>,
    account: ConnectorAccount,
) -> DatabaseCommandResult<RedactedConnectorAccount> {
    let redaction = Redaction::load(db, std::slice::from_ref(&account))?;
    Ok(redaction.redact(account))
}

/// Every connector account, oldest first.
pub struct ListConnectorAccountRowsCommand {}
impl DatabaseCommand for ListConnectorAccountRowsCommand {
    type Response = Vec<ConnectorAccount>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx
            .db
            .select::<ConnectorAccount>()
            .order("connector_accounts.id")
            .all()?)
    }
}

/// The accounts added for extension `extension_id`, oldest first.
pub struct ListExtensionConnectorAccountsCommand {
    pub extension_id: i64,
}
impl DatabaseCommand for ListExtensionConnectorAccountsCommand {
    type Response = Vec<ConnectorAccount>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx
            .db
            .select::<ConnectorAccount>()
            .filter("connector_accounts.extension_id = ?")
            .bind(self.extension_id)
            .order("connector_accounts.id")
            .all()?)
    }
}

pub struct GetConnectorAccountCommand {
    pub account_id: i64,
}
impl DatabaseCommand for GetConnectorAccountCommand {
    type Response = ConnectorAccount;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        ctx.db
            .select::<ConnectorAccount>()
            .filter_eq(ConnectorAccount::ID, self.account_id)
            .one()
            .found()
    }
}

/// The connector account `account_id` is one of.
pub struct GetAccountConnectorCommand {
    pub account_id: i64,
}
impl DatabaseCommand for GetAccountConnectorCommand {
    type Response = Connector;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        let account = GetConnectorAccountCommand {
            account_id: self.account_id,
        }
        .handle(ctx)?;
        ctx.db
            .select::<Connector>()
            .filter_eq(Connector::ID, account.connector_id)
            .one()
            .found()
    }
}

/// Adds an account of the extension's connector, storing `connector` as
/// its definition when it is new or newer, or replaces the description and
/// credential of the account the extension already has with this
/// identifier.
pub struct UpsertConnectorAccountCommand {
    pub extension_id: i64,
    pub connector: ConnectorDefinition,
    pub identifier: String,
    pub description: String,
    pub credential: Credential,
}
impl DatabaseCommand for UpsertConnectorAccountCommand {
    type Response = RedactedConnectorAccount;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let upserted = upsert_connector(ctx, self.connector)?;
        upserted.and_then(|connector| {
            // The model leaves the credential out, so the row model writes it.
            ctx.db
                .insert::<AccountRow>()
                .set(AccountRow::CONNECTOR_ID, connector.id)
                .set_group(AccountHost::Extension {
                    extension_id: self.extension_id,
                    identifier: self.identifier.clone(),
                })
                .set(AccountRow::DESCRIPTION, self.description)
                .set(AccountRow::AUTHORIZATION, self.credential.authorization)
                .set(AccountRow::REFRESH, self.credential.refresh)
                .set(AccountRow::EXPIRES_AT, self.credential.expires_at)
                .set(AccountRow::CREDENTIAL_FAILED_AT, None::<i64>)
                .on_conflict_update(&["extension_id", "identifier"])
                .execute()?;
            let account = ctx
                .db
                .select::<ConnectorAccount>()
                .filter("connector_accounts.extension_id = ? AND connector_accounts.identifier = ?")
                .bind(self.extension_id)
                .bind(self.identifier)
                .one()
                .found()?;
            Ok(redacted(ctx.db, account)?.invalidating([Invalidation::ConnectorAccounts]))
        })
    }
}

/// The credential stored for account `account_id`, provided extension
/// `extension_id` serves it. Only [`CredentialManager`](crate::CredentialManager)
/// runs this.
pub(crate) struct GetConnectorCredentialCommand {
    pub extension_id: i64,
    pub account_id: i64,
}
impl DatabaseCommand for GetConnectorCredentialCommand {
    type Response = Credential;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        stored_credential(ctx.db, self.extension_id, self.account_id)
    }
}

/// Stores a refreshed credential for account `account_id`, provided
/// extension `extension_id` serves it, keeping the refresh token when the
/// new credential has none, and returns what is now stored. Only
/// [`CredentialManager`](crate::CredentialManager) runs this.
pub(crate) struct UpdateConnectorCredentialCommand {
    pub extension_id: i64,
    pub account_id: i64,
    pub credential: Credential,
}
impl DatabaseCommand for UpdateConnectorCredentialCommand {
    type Response = Credential;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let was_failed = stored_row(ctx.db, self.extension_id, self.account_id)?
            .credential_failed_at
            .is_some();
        let mut update = ctx
            .db
            .update::<AccountRow>()
            .set(AccountRow::AUTHORIZATION, self.credential.authorization)
            .set(AccountRow::EXPIRES_AT, self.credential.expires_at)
            .set_null(AccountRow::CREDENTIAL_FAILED_AT);
        if let Some(refresh) = self.credential.refresh {
            update = update.set(AccountRow::REFRESH, refresh);
        }
        update
            .filter("id = ? AND extension_id = ?")
            .bind(self.account_id)
            .bind(self.extension_id)
            .execute()?;
        // Clients show the flag, so clearing one is a change they refetch.
        Ok(
            stored_credential(ctx.db, self.extension_id, self.account_id)?
                .invalidating(was_failed.then_some(Invalidation::ConnectorAccounts)),
        )
    }
}

/// Flags account `account_id`, provided extension `extension_id` serves it,
/// as needing to be signed in again: its credential's refresh failed. An
/// account already flagged keeps its first time. Only
/// [`CredentialManager`](crate::CredentialManager) runs this.
pub(crate) struct FailConnectorCredentialCommand {
    pub extension_id: i64,
    pub account_id: i64,
}
impl DatabaseCommand for FailConnectorCredentialCommand {
    type Response = AcceptedResponse;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let row = stored_row(ctx.db, self.extension_id, self.account_id)?;
        if row.credential_failed_at.is_some() {
            return Ok(AcceptedResponse {}.invalidating_nothing());
        }
        ctx.db
            .update::<AccountRow>()
            .set_now(AccountRow::CREDENTIAL_FAILED_AT)
            .filter("id = ? AND extension_id = ?")
            .bind(self.account_id)
            .bind(self.extension_id)
            .execute()?;
        Ok(AcceptedResponse {}.invalidating([Invalidation::ConnectorAccounts]))
    }
}

/// Account `account_id`'s credential columns, provided extension
/// `extension_id` serves it.
fn stored_row<'c>(
    db: impl ReadableDatabase<'c>,
    extension_id: i64,
    account_id: i64,
) -> DatabaseCommandResult<StoredCredential> {
    db.select_as::<StoredCredential>()
        .filter("id = ? AND extension_id = ?")
        .bind(account_id)
        .bind(extension_id)
        .one()?
        .ok_or(CommandError::NotFound(ConnectorAccount::NOUN))
}

/// Account `account_id`'s credential, provided extension `extension_id`
/// serves it.
fn stored_credential<'c>(
    db: impl ReadableDatabase<'c>,
    extension_id: i64,
    account_id: i64,
) -> DatabaseCommandResult<Credential> {
    let stored = stored_row(db, extension_id, account_id)?;
    Ok(Credential {
        // The table's check gives every extension's account a credential.
        authorization: stored
            .authorization
            .expect("an extension's account has a credential"),
        refresh: stored.refresh,
        expires_at: stored.expires_at,
    })
}

/// Removes an account; the resources imported from it cascade with it.
#[derive(FromRequest)]
#[request(DeleteConnectorAccountRequest)]
pub struct DeleteConnectorAccountCommand {
    pub account_id: i64,
}
impl DatabaseCommand for DeleteConnectorAccountCommand {
    type Response = RedactedConnectorAccount;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let account = GetConnectorAccountCommand {
            account_id: self.account_id,
        }
        .handle(&mut ctx.as_read())?;
        let account = redacted(ctx.db, account)?;
        let workspaces: Vec<i64> = ctx
            .db
            .select_column::<Resource, i64>("workspace_id")
            .distinct()
            .filter("account_id = ?")
            .bind(self.account_id)
            .all()?;
        ctx.db
            .delete::<ConnectorAccount>()
            .filter_eq(ConnectorAccount::ID, self.account_id)
            .execute()?;
        // An endpoint signed in with the account goes with it.
        Ok(account
            .invalidating([Invalidation::ConnectorAccounts, Invalidation::Intelligence])
            .and(workspaces.into_iter().map(Invalidation::Resources)))
    }
}

/// Moves every resource of account `from_account_id` to account
/// `to_account_id`, in every workspace; the schema refuses an account of
/// another connector. A resource whose item the target already holds in
/// the same workspace is dropped instead: they are the same item, and the
/// target's copy stays.
#[derive(FromRequest)]
#[request(MigrateResourcesRequest)]
pub struct MigrateResourcesCommand {
    pub from_account_id: i64,
    pub to_account_id: i64,
}
impl DatabaseCommand for MigrateResourcesCommand {
    type Response = AcceptedResponse;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let held: HashSet<(i64, String)> = ctx
            .db
            .select::<Resource>()
            .filter_eq(Resource::ACCOUNT_ID, self.to_account_id)
            .all()?
            .into_iter()
            .map(|resource| (resource.workspace_id, resource.identifier))
            .collect();
        let moving = ctx
            .db
            .select::<Resource>()
            .filter_eq(Resource::ACCOUNT_ID, self.from_account_id)
            .all()?;
        let workspaces: HashSet<i64> = moving
            .iter()
            .map(|resource| resource.workspace_id)
            .collect();
        let duplicates = moving
            .into_iter()
            .filter(|resource| held.contains(&(resource.workspace_id, resource.identifier.clone())))
            .map(|resource| resource.id);
        ctx.db
            .delete::<Resource>()
            .filter_in_values("id", duplicates)
            .execute()?;
        ctx.db
            .update::<Resource>()
            .set(Resource::ACCOUNT_ID, self.to_account_id)
            .filter_eq(Resource::ACCOUNT_ID, self.from_account_id)
            .execute()?;
        Ok(AcceptedResponse {}.invalidating(workspaces.into_iter().map(Invalidation::Resources)))
    }
}

#[cfg(test)]
mod tests;
