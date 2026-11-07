//! `Account` in scripts.

use cohesion_core::model::{AccountHost, ConnectorAccount, Credential};
use cohesion_db::{CreateAccountEndpointCommand, CredentialError};
use cohesion_macros::js_entity;
use rquickjs::{Ctx, Exception, Function, Value};

use super::intelligence_endpoint::JsIntelligenceEndpoint;
use crate::conversion::plain;
use crate::globals::AccountEndpointOptions;
use crate::traced_command::throw_command_error;

/// An account the user added for this extension, from `accounts.all()`.
/// `identifier` and `description` are what
/// `getAccountDescription(credential)` returned. `createdAt` is in Unix
/// seconds.
#[js_entity(ConnectorAccount, js = "Account")]
impl JsAccount {
    /// The account's identifier at the service, unique per extension.
    #[qjs(get, rename = "identifier")]
    pub fn identifier(&self) -> String {
        match &self.inner.host {
            AccountHost::Extension { identifier, .. } => identifier.clone(),
            AccountHost::Orphaned => {
                unreachable!("`accounts.all()` lists only its extension's accounts")
            }
        }
    }

    /// The account's credential, to reach the service with. Within 60
    /// seconds of its `expiresAt`, this first calls this extension's
    /// `completeRefresh(credential)` and stores what it returns, so the
    /// credential returned is current; a refresh already under way elsewhere
    /// is waited for instead. Throws whatever `completeRefresh` throws, and
    /// an `Error` when it isn't defined or calls `credential()` itself.
    #[ts(returns = "Credential")]
    pub fn credential<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        let extension_id = self
            .inner
            .host
            .extension_id()
            .expect("`accounts.all()` lists only its extension's accounts");
        let credential = self
            .db
            .credentials()
            .fresh(extension_id, self.inner.id, |credential| {
                complete_refresh(&ctx, credential)
            })
            .map_err(|e| match e {
                CredentialError::Command(e) => throw_command_error(&ctx, e),
                CredentialError::Reentrant => Exception::throw_message(
                    &ctx,
                    "`credential()` can't be called while `completeRefresh` runs.",
                ),
                CredentialError::Refresh(e) => e,
            })?;
        plain(&ctx, &credential)
    }

    /// Creates an intelligence endpoint that signs in with this account:
    /// each request it sends carries the account's credential, renewed by
    /// `completeRefresh` first as `credential()` renews it. It starts with
    /// no models; add them with `addModel`. Removing the account, or
    /// uninstalling this extension, deletes it. Throws if the name is
    /// taken. Needs the `intelligence` permission.
    #[qjs(rename = "createEndpoint")]
    pub fn create_endpoint<'js>(
        &self,
        ctx: Ctx<'js>,
        options: AccountEndpointOptions,
    ) -> rquickjs::Result<JsIntelligenceEndpoint> {
        let endpoint = self.write(
            &ctx,
            CreateAccountEndpointCommand {
                account_id: self.inner.id,
                name: options.name,
                provider_identifier: options.identifier,
                dialect: options.dialect,
                base_url: options.base_url,
                usage_url: options.usage_url,
                cache_minutes: options.cache_minutes,
            },
        )?;
        Ok(JsIntelligenceEndpoint::new(endpoint, self.db.clone()))
    }
}

/// What the script's own global `completeRefresh(credential)` returns for
/// `credential`.
fn complete_refresh(ctx: &Ctx<'_>, credential: &Credential) -> rquickjs::Result<Credential> {
    let Some(refresh) = ctx
        .globals()
        .get::<_, Option<Function>>("completeRefresh")?
    else {
        return Err(Exception::throw_message(
            ctx,
            "script does not define a function named `completeRefresh`",
        ));
    };
    let fresh: Value = refresh.call((plain(ctx, credential)?,))?;
    let json = ctx
        .json_stringify(fresh)?
        .map(|json| json.to_string())
        .transpose()?
        .unwrap_or_default();
    serde_json::from_str(&json).map_err(|e| {
        Exception::throw_type(
            ctx,
            &format!("completeRefresh didn't return a Credential: {e}"),
        )
    })
}
