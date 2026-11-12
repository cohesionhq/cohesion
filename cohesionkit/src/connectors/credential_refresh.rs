//! Reaching an account's credential through the database's credential
//! manager, refreshed through `completeRefresh` before it expires.

use cohesion_core::model::{ConnectorAccount, Credential, Extension, ExtensionPermissionKind};
use cohesion_core::protocol::ServerError;
use cohesion_db::{CredentialError, DatabaseContext};
use cohesion_js::ScriptArgument;

use crate::connectors::connector_calls::{connector_error, json_argument};
use crate::extension_manager::{ExtensionError, ExtensionFn, ExtensionManager};

/// `completeRefresh(credential)`: `credential` renewed before it expires.
struct CompleteRefreshFn<'a> {
    credential: &'a Credential,
}
impl ExtensionFn for CompleteRefreshFn<'_> {
    const NAME: &'static str = "completeRefresh";
    const REQUIRES: ExtensionPermissionKind = ExtensionPermissionKind::OAuth;
    type Output = Credential;
    fn arguments(self) -> Vec<ScriptArgument> {
        vec![json_argument(self.credential)]
    }
}

/// `account`'s credential, refreshed and stored first when it is about to
/// expire. The manager runs on `extension`'s scope, as a script's own
/// `credential()` does, so its `completeRefresh` is called where any other
/// refresh of the account would be, and never while that one holds the
/// account.
pub(crate) async fn fresh_credential(
    extensions: &ExtensionManager,
    database: &DatabaseContext,
    extension: &Extension,
    account: &ConnectorAccount,
) -> Result<Credential, ServerError> {
    let (extension_id, account_id) = (extension.id, account.id);
    // Most of the time nothing is due, and the extension's scope, which may
    // be busy running something else, needn't be waited on.
    if let Some(credential) = database
        .credentials()
        .unexpired(extension_id, account_id)
        .await?
    {
        return Ok(credential);
    }
    let database = database.clone();
    let fresh = extensions
        .scoped(
            extension.id,
            Some(CompleteRefreshFn::REQUIRES),
            move |sandbox| {
                Ok(database
                    .credentials()
                    .fresh(extension_id, account_id, |credential| {
                        CompleteRefreshFn { credential }.call_in(sandbox)
                    }))
            },
        )
        .await
        .map_err(|e| connector_error(extension, CompleteRefreshFn::NAME, e))?;
    fresh.map_err(|e| match e {
        CredentialError::Command(e) => e.into(),
        CredentialError::Refresh(e) => connector_error(
            extension,
            CompleteRefreshFn::NAME,
            ExtensionError::Script(e),
        ),
        CredentialError::Reentrant => unreachable!("a scope job starts outside any refresh"),
    })
}
