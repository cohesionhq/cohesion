//! The key a built-in backend sends with each request.

use std::sync::Arc;

use crate::account_credentials::AccountCredentials;
use crate::intelligence_error::IntelligenceError;

/// What authorizes a built-in backend's requests: an API key the endpoint
/// stores, if it needs one, or an account's credential, asked for before
/// every request so a session outliving the credential gets its renewal.
pub enum EndpointKey {
    Stored(Option<String>),
    Account {
        account_id: i64,
        credentials: Arc<dyn AccountCredentials>,
    },
}
impl EndpointKey {
    /// The key to send now, if any.
    pub(super) async fn current(&self) -> Result<Option<String>, IntelligenceError> {
        match self {
            Self::Stored(key) => Ok(key.clone()),
            Self::Account {
                account_id,
                credentials,
            } => credentials.authorization(*account_id).await.map(Some),
        }
    }
}
