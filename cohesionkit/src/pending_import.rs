//! An import a client finished, staged to replace the database. cohesiond
//! waits on it as it waits on a signal: it shuts down, puts the import in
//! place and starts again.

use std::sync::{Arc, Mutex};

use cohesion_db::StagedImport;
use tokio::sync::Notify;

/// The import waiting for the daemon to restart, if any. Clones share it.
#[derive(Clone, Default)]
pub struct PendingImport {
    staged: Arc<Mutex<Option<StagedImport>>>,
    ready: Arc<Notify>,
}

impl PendingImport {
    /// Asks the daemon to restart onto `staged`, replacing one asked for
    /// before.
    pub(crate) fn restart_onto(&self, staged: StagedImport) {
        *self.staged.lock().unwrap_or_else(|e| e.into_inner()) = Some(staged);
        self.ready.notify_one();
    }

    /// Waits until a client finishes an import, and answers it.
    pub async fn requested(&self) -> StagedImport {
        loop {
            self.ready.notified().await;
            if let Some(staged) = self.staged.lock().unwrap_or_else(|e| e.into_inner()).take() {
                return staged;
            }
        }
    }
}
