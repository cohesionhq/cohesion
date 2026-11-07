use cohesion_db::{
    GetExtensionCommand, GetExtensionQuery, GuardedDatabaseContext,
    ListExtensionConnectorAccountsCommand,
};
use rquickjs::{Ctx, Exception, JsLifetime, class::Trace};

use cohesion_macros::js_global;

use super::global::Global;
use crate::entities::account::JsAccount;
use crate::traced_command::read;

#[derive(Trace, JsLifetime)]
#[rquickjs::class]
pub struct AccountsGlobal {
    #[qjs(skip_trace)]
    database: GuardedDatabaseContext,
}

/// The accounts the user added for this extension through its sign-in
/// functions, each with its credential. Needs the `oauth` permission.
///
/// ```js
/// for (const account of accounts.all()) {
///   const { authorization } = account.credential();
///   network.fetch({ url: "https://api.example.com/me", method: NetworkMethod.get,
///                   headers: { Authorization: "Bearer " + authorization } });
/// }
/// ```
#[js_global("accounts")]
#[rquickjs::methods]
impl AccountsGlobal {
    /// Every account the user added for this extension, oldest first. Other
    /// extensions' accounts are never listed.
    pub fn all<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Vec<JsAccount>> {
        let Some(identifier) = self.database.extension() else {
            return Err(Exception::throw_message(
                &ctx,
                "`accounts` is only available to extensions.",
            ));
        };
        let extension = read(
            &ctx,
            &self.database,
            GetExtensionCommand {
                query: GetExtensionQuery::ScriptIdentifier(identifier.to_string()),
            },
        )?;
        let accounts = read(
            &ctx,
            &self.database,
            ListExtensionConnectorAccountsCommand {
                extension_id: extension.id,
            },
        )?;
        Ok(accounts
            .into_iter()
            .map(|account| JsAccount::new(account, self.database.clone()))
            .collect())
    }
}

impl Global for AccountsGlobal {
    fn install<'a>(
        context: &Ctx<'a>,
        database: GuardedDatabaseContext,
    ) -> Result<(), rquickjs::Error> {
        context
            .globals()
            .set("accounts", AccountsGlobal { database })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
