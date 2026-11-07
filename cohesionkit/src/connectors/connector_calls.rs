//! Calling an extension's sign-in (`oauth`) and import (`connector`)
//! functions on its scoped worker, typed by the connector contract, and the
//! import calls made on behalf of an account, which match on what serves
//! it: an extension's account looks its credential up first, and each call
//! checks, as it is made, that the extension still holds the grants the
//! function needs; an orphaned account reaches nothing.

use cohesion_core::log::Module;
use cohesion_core::log_error;
use cohesion_core::model::{
    AccountDescription, AccountHost, ConnectorAccount, ConnectorDefinition, ConnectorResource,
    ConnectorResourceFilters, ConnectorResourcePreview, Credential, Extension,
    ExtensionPermissionKind, OrganizationUnit, PkceChallenge, ServiceDescription,
};
use cohesion_core::protocol::{RedactedConnectorAccount, ServerError};
use cohesion_db::{DatabaseContext, GetConnectorAccountCommand, UpsertConnectorAccountCommand};
use cohesion_js::ScriptArgument;

use crate::connectors::credential_refresh::fresh_credential;
use crate::extension_manager::{ExtensionError, ExtensionFn, ExtensionManager, script_fault};

/// The connector `extension` defines: its identifier, version and icon, and
/// what its `serviceDescription()` returns.
pub(crate) fn definition(
    extension: &Extension,
    service: ServiceDescription,
) -> ConnectorDefinition {
    ConnectorDefinition {
        identifier: extension.script_identifier.clone(),
        icon: extension.icon.clone(),
        version: extension.version,
        description: service,
    }
}

/// `serviceDescription()`: the service the extension signs in to.
pub(crate) struct ServiceDescriptionFn;
impl ExtensionFn for ServiceDescriptionFn {
    const NAME: &'static str = "serviceDescription";
    const REQUIRES: ExtensionPermissionKind = ExtensionPermissionKind::OAuth;
    type Output = ServiceDescription;
    fn arguments(self) -> Vec<ScriptArgument> {
        Vec::new()
    }
}

/// `authenticationURL(pkce)`: the sign-in page, which the sign-in's PKCE
/// challenge goes into.
pub(crate) struct AuthenticationUrlFn {
    pub(crate) pkce: PkceChallenge,
}
impl ExtensionFn for AuthenticationUrlFn {
    const NAME: &'static str = "authenticationURL";
    const REQUIRES: ExtensionPermissionKind = ExtensionPermissionKind::OAuth;
    type Output = String;
    fn arguments(self) -> Vec<ScriptArgument> {
        vec![json_argument(&self.pkce)]
    }
}

/// `completeAuthentication(code, verifier)`: the credential `code` is
/// exchanged for, with the sign-in's PKCE verifier when the daemon still
/// holds it.
struct CompleteAuthenticationFn {
    code: String,
    verifier: Option<String>,
}
impl ExtensionFn for CompleteAuthenticationFn {
    const NAME: &'static str = "completeAuthentication";
    const REQUIRES: ExtensionPermissionKind = ExtensionPermissionKind::OAuth;
    type Output = Credential;
    fn arguments(self) -> Vec<ScriptArgument> {
        let mut arguments = vec![ScriptArgument::String(self.code)];
        arguments.extend(self.verifier.map(ScriptArgument::String));
        arguments
    }
}

/// `getAccountDescription(credential)`: which account `credential` belongs to.
struct AccountDescriptionFn<'a> {
    credential: &'a Credential,
}
impl ExtensionFn for AccountDescriptionFn<'_> {
    const NAME: &'static str = "getAccountDescription";
    const REQUIRES: ExtensionPermissionKind = ExtensionPermissionKind::OAuth;
    type Output = AccountDescription;
    fn arguments(self) -> Vec<ScriptArgument> {
        vec![json_argument(self.credential)]
    }
}

/// `accountAdded(account)`: the user added the account, or signed in to it
/// again, so the extension can set up what it needs, such as an endpoint
/// that signs in with it. Optional.
struct AccountAddedFn {
    account: AccountDescription,
}
impl ExtensionFn for AccountAddedFn {
    const NAME: &'static str = "accountAdded";
    const REQUIRES: ExtensionPermissionKind = ExtensionPermissionKind::OAuth;
    type Output = ();
    fn arguments(self) -> Vec<ScriptArgument> {
        vec![json_argument(&self.account)]
    }
}

/// `accountRemoved(account)`: the account was removed, so the extension
/// can undo what it set up for it. Optional.
struct AccountRemovedFn {
    account: AccountDescription,
}
impl ExtensionFn for AccountRemovedFn {
    const NAME: &'static str = "accountRemoved";
    const REQUIRES: ExtensionPermissionKind = ExtensionPermissionKind::OAuth;
    type Output = ();
    fn arguments(self) -> Vec<ScriptArgument> {
        vec![json_argument(&self.account)]
    }
}

/// `getOrganizationUnits(credential)`: the parts the account groups its
/// items into.
struct OrganizationUnitsFn<'a> {
    credential: &'a Credential,
}
impl ExtensionFn for OrganizationUnitsFn<'_> {
    const NAME: &'static str = "getOrganizationUnits";
    const REQUIRES: ExtensionPermissionKind = ExtensionPermissionKind::Connector;
    type Output = Vec<OrganizationUnit>;
    fn arguments(self) -> Vec<ScriptArgument> {
        vec![json_argument(self.credential)]
    }
}

/// `listResources(filters, credential)`: the items offered for import.
struct ListResourcesFn<'a> {
    filters: &'a ConnectorResourceFilters,
    credential: &'a Credential,
}
impl ExtensionFn for ListResourcesFn<'_> {
    const NAME: &'static str = "listResources";
    const REQUIRES: ExtensionPermissionKind = ExtensionPermissionKind::Connector;
    type Output = Vec<ConnectorResourcePreview>;
    fn arguments(self) -> Vec<ScriptArgument> {
        vec![json_argument(self.filters), json_argument(self.credential)]
    }
}

/// `getResource(identifier, credential)`: the whole item.
struct GetResourceFn<'a> {
    identifier: String,
    credential: &'a Credential,
}
impl ExtensionFn for GetResourceFn<'_> {
    const NAME: &'static str = "getResource";
    const REQUIRES: ExtensionPermissionKind = ExtensionPermissionKind::Connector;
    type Output = ConnectorResource;
    fn arguments(self) -> Vec<ScriptArgument> {
        vec![
            ScriptArgument::String(self.identifier),
            json_argument(self.credential),
        ]
    }
}

/// How an account's items are reached: through the extension serving it,
/// with the account's credential.
pub(crate) struct Connection {
    extension: Extension,
    credential: Credential,
}

impl Connection {
    /// The parts the account groups its items into.
    pub(crate) async fn organization_units(
        &self,
        extensions: &ExtensionManager,
    ) -> Result<Vec<OrganizationUnit>, ServerError> {
        let function = OrganizationUnitsFn {
            credential: &self.credential,
        };
        call_connector(extensions, &self.extension, function).await
    }

    /// The items the account offers for import, narrowed by `filters`.
    pub(crate) async fn list(
        &self,
        extensions: &ExtensionManager,
        filters: ConnectorResourceFilters,
    ) -> Result<Vec<ConnectorResourcePreview>, ServerError> {
        let function = ListResourcesFn {
            filters: &filters,
            credential: &self.credential,
        };
        call_connector(extensions, &self.extension, function).await
    }

    /// The whole item `identifier`, failing when it comes back without a
    /// `kindIdentifier`.
    pub(crate) async fn fetch(
        &self,
        extensions: &ExtensionManager,
        identifier: String,
    ) -> Result<ConnectorResource, ServerError> {
        let function = GetResourceFn {
            identifier,
            credential: &self.credential,
        };
        let resource = call_connector(extensions, &self.extension, function).await?;
        if resource.kind_identifier.is_empty() {
            return Err(ServerError::upstream(format!(
                "{}: {} returned a resource without a kindIdentifier.",
                self.extension.name,
                GetResourceFn::NAME
            )));
        }
        Ok(resource)
    }
}

/// How `account`'s items are reached. An extension's account needs its
/// extension granted `connector` and `oauth`, and its credential, refreshed
/// first if it is about to expire. An orphaned account is reached by
/// nothing.
pub(crate) async fn connect(
    extensions: &ExtensionManager,
    database: &DatabaseContext,
    account: &ConnectorAccount,
) -> Result<Connection, ServerError> {
    match &account.host {
        AccountHost::Extension { extension_id, .. } => {
            let extension = extensions
                .granted(*extension_id, ExtensionPermissionKind::Connector)
                .await?;
            let credential = fresh_credential(extensions, database, &extension, account).await?;
            Ok(Connection {
                extension,
                credential,
            })
        }
        AccountHost::Orphaned => Err(ServerError::validation(
            None,
            format!(
                "{} is no longer connected; move its resources to another account.",
                account.description
            ),
        )),
    }
}

/// Finishes an extension's sign-in with `code`: exchanges it for a
/// credential, asks the extension who the account is and what service it is,
/// stores the account and tells the extension with its `accountAdded`. What
/// `accountAdded` throws fails the sign-in, though the account stays added,
/// so the user sees why what it sets up is missing.
pub(crate) async fn complete_connector_authentication(
    extensions: &ExtensionManager,
    database: &DatabaseContext,
    extension_id: i64,
    code: String,
    verifier: Option<String>,
) -> Result<RedactedConnectorAccount, ServerError> {
    let extension = extensions
        .granted(extension_id, ExtensionPermissionKind::OAuth)
        .await?;
    let complete = CompleteAuthenticationFn { code, verifier };
    let credential = call_connector(extensions, &extension, complete).await?;
    let described = AccountDescriptionFn {
        credential: &credential,
    };
    let account = call_connector(extensions, &extension, described).await?;
    let service = call_connector(extensions, &extension, ServiceDescriptionFn).await?;
    let added = database
        .send(UpsertConnectorAccountCommand {
            extension_id: extension.id,
            connector: definition(&extension, service),
            identifier: account.identifier.clone(),
            description: account.description.clone(),
            credential,
        })
        .await?;
    extensions
        .notify(extension.id, AccountAddedFn { account })
        .await
        .map_err(|e| connector_error(&extension, AccountAddedFn::NAME, e))?;
    Ok(added)
}

/// Tells the extension that served `account`, which was just removed, with
/// its `accountRemoved`. The account is gone whatever the extension does,
/// so a failure is logged rather than returned, and an extension no longer
/// granted `oauth` isn't told.
pub(crate) async fn account_removed(
    extensions: &ExtensionManager,
    account: &RedactedConnectorAccount,
) {
    let AccountHost::Extension {
        extension_id,
        identifier,
    } = &account.host
    else {
        return;
    };
    let removed = AccountRemovedFn {
        account: AccountDescription {
            identifier: identifier.clone(),
            description: account.description.clone(),
        },
    };
    match extensions.notify(*extension_id, removed).await {
        Ok(()) | Err(ExtensionError::NotGranted(_)) => {}
        Err(e) => log_error!(
            Module::Extensions,
            "{}: {} failed: {e}",
            account.connector.title,
            AccountRemovedFn::NAME
        ),
    }
}

/// How account `account_id`'s items are reached.
async fn connected_account(
    extensions: &ExtensionManager,
    database: &DatabaseContext,
    account_id: i64,
) -> Result<Connection, ServerError> {
    let account = database
        .send(GetConnectorAccountCommand { account_id })
        .await?;
    connect(extensions, database, &account).await
}

/// The organization units of account `account_id`: what its extension's
/// `getOrganizationUnits` returns, such as its calendars.
pub(crate) async fn account_organization_units(
    extensions: &ExtensionManager,
    database: &DatabaseContext,
    account_id: i64,
) -> Result<Vec<OrganizationUnit>, ServerError> {
    connected_account(extensions, database, account_id)
        .await?
        .organization_units(extensions)
        .await
}

/// The items account `account_id` offers for import: what its extension's
/// `listResources` returns, narrowed by `filters`.
pub(crate) async fn account_items(
    extensions: &ExtensionManager,
    database: &DatabaseContext,
    account_id: i64,
    filters: ConnectorResourceFilters,
) -> Result<Vec<ConnectorResourcePreview>, ServerError> {
    connected_account(extensions, database, account_id)
        .await?
        .list(extensions, filters)
        .await
}

/// Account `account_id`'s whole item `identifier` (see
/// [`Connection::fetch`]).
pub(crate) async fn fetch_resource(
    extensions: &ExtensionManager,
    database: &DatabaseContext,
    account_id: i64,
    identifier: String,
) -> Result<ConnectorResource, ServerError> {
    connected_account(extensions, database, account_id)
        .await?
        .fetch(extensions, identifier)
        .await
}

/// Passes `value` to a script as a plain object.
pub(super) fn json_argument(value: &impl serde::Serialize) -> ScriptArgument {
    ScriptArgument::Json(serde_json::to_value(value).expect("contract types serialize"))
}

/// Calls `extension`'s global `function`, provided it still holds what
/// `function` requires, failing as [`connector_error`] says.
pub(crate) async fn call_connector<F: ExtensionFn>(
    extensions: &ExtensionManager,
    extension: &Extension,
    function: F,
) -> Result<F::Output, ServerError> {
    extensions
        .call(extension.id, function)
        .await
        .map_err(|e| connector_error(extension, F::NAME, e))
}

/// What a failed call to `extension`'s `function` tells the caller: a
/// failure the script caused is the user's to see; anything else stays in
/// the log.
pub(super) fn connector_error(
    extension: &Extension,
    function: &str,
    error: ExtensionError,
) -> ServerError {
    match error {
        ExtensionError::Script(e) => script_fault(&extension.name, function, e, |message| {
            format!(
                "{}: {function} didn't return the expected shape: {message}",
                extension.name
            )
        })
        .map_or_else(ServerError::internal, ServerError::upstream),
        e => e.into(),
    }
}
