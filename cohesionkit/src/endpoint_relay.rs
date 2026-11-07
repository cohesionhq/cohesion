//! How agents' sessions reach an endpoint that signs in with an extension's
//! account: by the account's credential, which the extension renews.

use cohesion_core::model::{AccountHost, ExtensionPermissionKind};
use cohesion_core::protocol::ServerError;
use cohesion_db::{DatabaseContext, GetConnectorAccountCommand};
use cohesion_intelligence::{AccountCredentials, CredentialFuture, IntelligenceError};

use crate::connectors::fresh_credential;
use crate::extension_manager::ExtensionManager;

/// The agents' [`AccountCredentials`].
pub(crate) struct EndpointRelay {
    database: DatabaseContext,
    extensions: ExtensionManager,
}

impl EndpointRelay {
    pub(crate) fn new(database: DatabaseContext, extensions: ExtensionManager) -> Self {
        Self {
            database,
            extensions,
        }
    }

    /// Account `account_id`'s authorization, refreshed first by the
    /// extension serving it, which must still hold `oauth`.
    async fn fresh_authorization(&self, account_id: i64) -> Result<String, ServerError> {
        let account = self
            .database
            .send(GetConnectorAccountCommand { account_id })
            .await?;
        let AccountHost::Extension { extension_id, .. } = account.host else {
            return Err(ServerError::validation(
                None,
                format!("{} is no longer signed in.", account.description),
            ));
        };
        let extension = self
            .extensions
            .granted(extension_id, ExtensionPermissionKind::OAuth)
            .await?;
        let credential =
            fresh_credential(&self.extensions, &self.database, &extension, &account).await?;
        Ok(credential.authorization)
    }
}

impl AccountCredentials for EndpointRelay {
    fn authorization(&self, account_id: i64) -> CredentialFuture<'_> {
        Box::pin(async move {
            self.fresh_authorization(account_id)
                .await
                .map_err(|error| IntelligenceError::AccountCredential(error.message))
        })
    }
}
