//! What the registry of clients needs from the rest of the daemon: the
//! database rows that mirror a client's offers.

use std::pin::Pin;

use cohesion_core::protocol::{ClientCapabilities, ServerError};
use cohesion_db::{DatabaseContext, SyncCapabilitiesCommand};

/// The boxed future returned by [`CapabilityStore::sync`], boxed so the trait
/// stays usable as `dyn CapabilityStore`.
pub type SyncFuture = Pin<Box<dyn Future<Output = Result<(), ServerError>> + Send>>;

/// Where [`CapabilityClients`](crate::CapabilityClients) keeps what a client offers.
/// The daemon's implementation is the database; the registry's tests bring
/// their own.
pub trait CapabilityStore: Send + Sync {
    /// Makes the rows standing for `client_id`'s offers the ones in `offers`.
    fn sync(&self, client_id: i64, offers: ClientCapabilities) -> SyncFuture;
}

impl CapabilityStore for DatabaseContext {
    fn sync(&self, client_id: i64, offers: ClientCapabilities) -> SyncFuture {
        let database = self.clone();
        Box::pin(async move {
            database
                .send(SyncCapabilitiesCommand { client_id, offers })
                .await?;
            Ok(())
        })
    }
}
