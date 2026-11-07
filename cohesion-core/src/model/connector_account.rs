use cohesion_macros::{ColumnGroup, Fields, Model, TsWire};
use serde::{Deserialize, Serialize};

/// One account of a connector, added through an `oauth` extension.
/// Accounts are daemon-wide; the resources imported from one belong to a
/// workspace. An extension's account has a credential, stored in the same
/// row but never read into this model: only the database's credential
/// manager hands it out.
///
/// An account outlives its extension: uninstalling it orphans the account,
/// keeping its resources until they are moved to another account of the
/// connector or the account is removed.
#[derive(Clone, Model, Fields)]
#[model(table = "connector_accounts", backup = Included)]
pub struct ConnectorAccount {
    /// Unique, stable identifier.
    pub id: i64,
    /// Id of the connector the account is one of.
    pub connector_id: i64,
    /// What serves the account. Scripts see its parts as `identifier`.
    #[model(group)]
    #[fields(skip)]
    pub host: AccountHost,
    /// Describes the account for people, such as its name or email.
    pub description: String,

    /// When the extension's `completeRefresh` last failed for the account's
    /// credential, in Unix seconds; `None` while it works. Signing in again
    /// or a refresh that succeeds clears it.
    pub credential_failed_at: Option<i64>,

    /// When the account was added, in Unix seconds.
    pub created_at: i64,
}

/// What serves an account's items. The table's checks give every row
/// exactly one of these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ColumnGroup, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AccountHost {
    /// An extension, with the account's credential.
    Extension {
        /// Id of the extension that serves the account.
        extension_id: i64,
        /// The account's identifier at the service, unique per extension.
        identifier: String,
    },
    /// Nothing any more: its extension was uninstalled. It can't be
    /// browsed or refreshed.
    Orphaned,
}

impl AccountHost {
    /// The id of the serving extension; none for any other account.
    pub fn extension_id(&self) -> Option<i64> {
        match self {
            Self::Extension { extension_id, .. } => Some(*extension_id),
            Self::Orphaned => None,
        }
    }
}
