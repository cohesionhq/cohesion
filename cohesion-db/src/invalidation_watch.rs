//! Waiting in the server for the database to change: on the invalidations
//! its commands publish, the same ones clients hear, rather than polling.

use std::future::Future;
use std::sync::Arc;

use cohesion_core::invalidation::{Invalidation, InvalidationBatch};
use tokio::sync::broadcast::{self, error::RecvError};

/// The invalidations published since
/// [`DatabaseContext::watch`](crate::DatabaseContext::watch) made it, to wait on until one touches
/// what the caller reads. Made before the caller's first read, so no change
/// after that read is missed.
pub struct InvalidationWatch {
    changes: broadcast::Receiver<Arc<InvalidationBatch>>,
}

/// What a [`InvalidationWatch::changed`] heard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Watched {
    /// A key the caller cares about was invalidated, or batches were
    /// missed that may have held one: read again.
    Changed,
    /// The database is closing, so nothing more will change.
    Closed,
}

impl InvalidationWatch {
    pub(crate) fn new(changes: broadcast::Receiver<Arc<InvalidationBatch>>) -> Self {
        Self { changes }
    }

    /// Waits for a batch holding a key `touches` accepts, skipping the rest.
    pub async fn changed(&mut self, touches: impl Fn(&Invalidation) -> bool) -> Watched {
        loop {
            match self.changes.recv().await {
                Ok(batch) if batch.keys.iter().any(&touches) => return Watched::Changed,
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => return Watched::Changed,
                Err(RecvError::Closed) => return Watched::Closed,
            }
        }
    }

    /// Reads with `read` until what it answers is `settled`, reading again
    /// whenever a key `touches` accepts for the last read is invalidated,
    /// and answers that read; or the last read, once the database closes.
    /// Bound it with a `select!` for a time limit or a cancellation.
    pub async fn until<T, F>(
        &mut self,
        mut read: impl FnMut() -> F,
        settled: impl Fn(&T) -> bool,
        touches: impl Fn(&T, &Invalidation) -> bool,
    ) -> T
    where
        F: Future<Output = T>,
    {
        loop {
            let value = read().await;
            if settled(&value) {
                return value;
            }
            if self.changed(|key| touches(&value, key)).await == Watched::Closed {
                return value;
            }
        }
    }
}

#[cfg(test)]
mod tests;
