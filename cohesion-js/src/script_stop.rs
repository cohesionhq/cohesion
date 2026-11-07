//! Stopping a running script from outside its worker.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Stops the scripts run under it, from any thread: one already running is
/// interrupted at its next step, as its deadline would, and one started
/// after fails at once. A cheap clone; every clone stops the same scripts.
#[derive(Clone, Debug, Default)]
pub struct ScriptStop(Arc<AtomicBool>);

impl ScriptStop {
    pub fn stop(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_stopped(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}
