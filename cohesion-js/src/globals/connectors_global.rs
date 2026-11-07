use cohesion_db::{GuardedDatabaseContext, ListConnectorAccountRowsCommand, ListConnectorsCommand};
use rquickjs::function::Opt;
use rquickjs::{Ctx, Exception, JsLifetime, class::Trace};

use cohesion_macros::js_global;

use super::global::Global;
use crate::connector_access::current_connectors;
use crate::entities::connector_account::JsConnectorAccount;
use crate::traced_command::read;

#[derive(Trace, JsLifetime)]
#[rquickjs::class]
pub struct ConnectorsGlobal {
    #[qjs(skip_trace)]
    database: GuardedDatabaseContext,
}

/// The accounts the user added at connected services, listed under
/// "Connected services" in the code tool's description, and the live services
/// behind them. Their items are normally already in Cohesion as resources: look
/// with `workspace.resources()` and `workspace.search()` first. Reaching a
/// service through an account is a last resort, for an item that is not
/// imported yet: it is slow and reads the service directly. For a one-off
/// question, `fetch()` the item and answer from it. Import only an item the
/// user will keep coming back to, and only as the user decides: in a
/// conversation, say which item and workspace and why, and import once the
/// user agrees; in a turn your cue started `import` throws, so ask the user
/// first. Exists only in an agent's sandbox.
#[js_global("connectors")]
#[rquickjs::methods]
impl ConnectorsGlobal {
    /// Every account the user added, clients' and orphans included, oldest
    /// first; with a connector identifier such as `"com.cohesion.notion"`,
    /// only that connector's accounts.
    pub fn accounts<'js>(
        &self,
        ctx: Ctx<'js>,
        connector: Opt<String>,
    ) -> rquickjs::Result<Vec<JsConnectorAccount>> {
        current_connectors(&ctx)?;
        let accounts = read(&ctx, &self.database, ListConnectorAccountRowsCommand {})?;
        let connector_id = match connector.0 {
            Some(identifier) => {
                let connectors = read(&ctx, &self.database, ListConnectorsCommand {})?;
                Some(
                    connectors
                        .into_iter()
                        .find(|connector| connector.identifier.eq_ignore_ascii_case(&identifier))
                        .map(|connector| connector.id),
                )
            }
            None => None,
        };
        Ok(accounts
            .into_iter()
            .filter(|account| {
                connector_id.is_none_or(|connector_id| Some(account.connector_id) == connector_id)
            })
            .map(|account| JsConnectorAccount::new(account, self.database.clone()))
            .collect())
    }

    /// The account with this id; throws a `ReferenceError` if there is none.
    pub fn account<'js>(&self, ctx: Ctx<'js>, id: i64) -> rquickjs::Result<JsConnectorAccount> {
        current_connectors(&ctx)?;
        let accounts = read(&ctx, &self.database, ListConnectorAccountRowsCommand {})?;
        accounts
            .into_iter()
            .find(|account| account.id == id)
            .map(|account| JsConnectorAccount::new(account, self.database.clone()))
            .ok_or_else(|| Exception::throw_reference(&ctx, &format!("no account with id {id}")))
    }
}

impl Global for ConnectorsGlobal {
    fn install<'a>(
        context: &Ctx<'a>,
        database: GuardedDatabaseContext,
    ) -> Result<(), rquickjs::Error> {
        context
            .globals()
            .set("connectors", ConnectorsGlobal { database })?;
        Ok(())
    }
}
