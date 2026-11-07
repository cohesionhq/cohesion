//! The sending side of the hook queue, fed by the database's events.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cohesion_db::{
    DatabaseEvent, DatabaseEventKind, DatabaseEventListener, DatabaseEventTrace, WorkTracker,
};
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::task::AbortHandle;

use super::hook_events::{HookEvent, HookInstance, event_hook};
use super::hook_receiver::HookReceiver;

/// How long a journal or document goes without a save before its settled
/// hook runs.
const DOCUMENT_SETTLE: Duration = Duration::from_secs(120);

#[derive(Clone, Debug)]
pub struct HookManager {
    tx: mpsc::UnboundedSender<HookInstance>,
    /// The journals and documents still settling, by document id.
    settling: Arc<Mutex<HashMap<i64, Settling>>>,
    /// Every queued hook counts as work until it has run.
    pub(super) work: WorkTracker,
    /// Database events arrive on the worker thread, outside the runtime.
    pub(super) runtime: Handle,
}

/// A document's settle timer, and the settled hook to queue when it runs out.
struct Settling {
    timer: AbortHandle,
    hook: Arc<dyn HookEvent>,
    trace: DatabaseEventTrace,
}
impl std::fmt::Debug for Settling {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Settling")
            .field("timer", &self.timer)
            .field("hook", &self.hook.kind())
            .finish_non_exhaustive()
    }
}

impl HookManager {
    /// Must be called inside the tokio runtime, which runs the settle timers.
    pub fn new(work: WorkTracker) -> (HookManager, HookReceiver) {
        let (tx, rx) = mpsc::unbounded_channel::<HookInstance>();
        let manager = Self {
            tx,
            settling: Arc::default(),
            work,
            runtime: Handle::current(),
        };
        (manager, HookReceiver(rx))
    }
    /// Queues the hook event. The channel is ordered, so events run in the
    /// order they were sent; [`HookReceiver`] runs each in every extension
    /// with its hook enabled.
    pub fn send<E: HookEvent>(&self, event: E) {
        self.queue(Arc::new(event), None, DatabaseEventTrace::default())
    }

    /// Queues the hook event for one extension only.
    pub fn send_scoped<E: HookEvent>(&self, event: E, extension_id: i64) {
        self.queue(
            Arc::new(event),
            Some(extension_id),
            DatabaseEventTrace::default(),
        )
    }

    pub(super) fn queue(
        &self,
        hook: Arc<dyn HookEvent>,
        extension_id: Option<i64>,
        trace: DatabaseEventTrace,
    ) {
        let running = self.work.begin(match extension_id {
            Some(extension_id) => format!("hook {} for extension {extension_id}", hook.kind()),
            None => format!("hook {}", hook.kind()),
        });
        // The receiver is gone only once the daemon is stopping.
        let _ = self.tx.send(HookInstance {
            hook,
            extension_id,
            trace,
            _running: running,
        });
    }

    /// Restarts the document's settle timer, cancelling the one running; when
    /// it runs out without another save, `hook` (`onJournalSettled` or
    /// `onDocumentSettled`) is queued with the last save's trace. Once the
    /// daemon is shutting down it is queued at once instead;
    /// [`settle_all`](Self::settle_all) queues the ones settling when that
    /// began.
    fn settle(&self, document_id: i64, hook: Arc<dyn HookEvent>, trace: DatabaseEventTrace) {
        // Held until the new timer is recorded, so the timer's check below
        // always finds it or its replacement, and `settle_all` either
        // finds it or this save sees the shutdown.
        let mut settling = self.settling.lock().unwrap();
        if self.work.is_closed() {
            if let Some(previous) = settling.remove(&document_id) {
                previous.timer.abort();
            }
            self.queue(hook, None, trace);
            return;
        }
        let manager = self.clone();
        let timer = self.runtime.spawn(async move {
            tokio::time::sleep(DOCUMENT_SETTLE).await;
            let document = {
                let mut settling = manager.settling.lock().unwrap();
                // A save landing as this timer woke replaced it before the
                // abort could take effect.
                if settling.get(&document_id).map(|s| s.timer.id()) != Some(tokio::task::id()) {
                    return;
                }
                settling.remove(&document_id).expect("checked above")
            };
            manager.queue(document.hook, None, document.trace);
        });
        let entry = Settling {
            timer: timer.abort_handle(),
            hook,
            trace,
        };
        if let Some(previous) = settling.insert(document_id, entry) {
            previous.timer.abort();
        }
    }

    /// Queues every settled hook still waiting for quiet now
    /// (`onJournalSettled` and `onDocumentSettled`, for each journal and
    /// document still settling), so a shutdown runs them rather than losing
    /// them. A new settled hook joins this rather than adding its own
    /// shutdown step. Call after closing the daemon's work.
    pub fn settle_all(&self) {
        let mut settling = self.settling.lock().unwrap();
        for (_, document) in settling.drain() {
            document.timer.abort();
            self.queue(document.hook, None, document.trace);
        }
    }
}
impl DatabaseEventListener for HookManager {
    fn send(&self, event: DatabaseEvent) {
        match event.kind {
            DatabaseEventKind::JournalSaved { document_id, .. }
            | DatabaseEventKind::DocumentSaved { document_id, .. } => {
                self.settle(document_id, event_hook(event.kind), event.trace)
            }
            kind => {
                self.queue(event_hook(kind), None, event.trace);
            }
        }
    }
}
