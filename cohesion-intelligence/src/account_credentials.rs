//! How an account-backed endpoint's turns get the account's credential,
//! through the daemon above this crate, which runs the extension that
//! renews it.

use std::pin::Pin;

use crate::intelligence_error::IntelligenceError;

/// The boxed future returned by [`AccountCredentials::authorization`],
/// boxed so the trait stays usable as `dyn AccountCredentials`.
pub type CredentialFuture<'a> =
    Pin<Box<dyn Future<Output = Result<String, IntelligenceError>> + Send + 'a>>;

/// Hands out connector accounts' credentials for the endpoints that sign in
/// with them. Implemented above this crate, where the extensions that renew
/// them run.
pub trait AccountCredentials: Send + Sync {
    /// The current authorization of account `account_id`, renewed first by
    /// its extension when it is about to expire;
    /// [`IntelligenceError::AccountCredential`] when there is none to give.
    fn authorization(&self, account_id: i64) -> CredentialFuture<'_>;
}
