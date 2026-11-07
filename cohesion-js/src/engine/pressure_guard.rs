use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering::Relaxed;

/// Counts one job against a worker's pressure until dropped, wherever the
/// request ends up: cancelled while queued, rejected, or finished.
#[must_use = "the job stops counting against the worker's pressure when this is dropped"]
pub(super) struct PressureGuard {
    pressure: Arc<AtomicI64>,
}
impl PressureGuard {
    #[must_use = "the job stops counting against the worker's pressure when the guard is dropped"]
    pub(super) fn new(pressure: &Arc<AtomicI64>) -> Self {
        pressure.fetch_add(1, Relaxed);
        Self {
            pressure: pressure.clone(),
        }
    }
}
impl Drop for PressureGuard {
    fn drop(&mut self) {
        self.pressure.fetch_sub(1, Relaxed);
    }
}
