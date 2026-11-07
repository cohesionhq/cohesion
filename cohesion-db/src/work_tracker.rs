//! The daemon's work in flight: client requests, queued hooks and agent
//! runs, counted so a shutdown can refuse new work and wait for the rest.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use cohesion_core::protocol::ShuttingDown;
use tokio::sync::Notify;

/// Counts the work in flight, each piece under a label naming it. Once
/// [closed](Self::close), [`begin_new`](Self::begin_new) refuses new work
/// while [`begin`](Self::begin) still admits work the running work starts,
/// so everything already under way runs to the end. A cheap clone.
#[derive(Clone, Debug, Default)]
pub struct WorkTracker {
    inner: Arc<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    state: Mutex<State>,
    /// Woken whenever the last piece of work ends.
    idle: Notify,
    /// Woken when the tracker is closed.
    closing: Notify,
}

#[derive(Debug, Default)]
struct State {
    closed: bool,
    next_id: u64,
    running: BTreeMap<u64, (String, Instant)>,
}

/// One piece of work, running until it is dropped.
#[derive(Debug)]
#[must_use = "the work counts as running only until this guard is dropped"]
pub struct WorkGuard {
    inner: Arc<Inner>,
    id: u64,
}

/// A piece of work still running, as [`WorkTracker::pending`] lists it.
#[derive(Clone, Debug)]
pub struct PendingWork {
    pub label: String,
    pub elapsed: Duration,
}

impl WorkTracker {
    /// Starts work that something already running needs, such as a hook a
    /// request's write queued; it is admitted even after [`close`](Self::close).
    #[must_use = "the work counts as running only until the guard is dropped"]
    pub fn begin(&self, label: impl Into<String>) -> WorkGuard {
        let mut state = self.state();
        let id = state.next_id;
        state.next_id += 1;
        state.running.insert(id, (label.into(), Instant::now()));
        WorkGuard {
            inner: self.inner.clone(),
            id,
        }
    }

    /// Starts work nothing running asked for, such as a client's request or
    /// a timer's pass; [`ShuttingDown`] once the tracker is closed. Prefer
    /// [`run_new`](Self::run_new) when the work ends where it began; this is
    /// for work that must carry its guard elsewhere, through a channel or
    /// into a spawned task.
    #[must_use = "the work counts as running only until the guard is dropped"]
    pub fn begin_new(&self, label: impl Into<String>) -> Result<WorkGuard, ShuttingDown> {
        match self.is_closed() {
            true => Err(ShuttingDown),
            false => Ok(self.begin(label)),
        }
    }

    /// Runs `work` as new work under `label`, holding its guard exactly
    /// until `work` completes; [`ShuttingDown`] once the tracker is closed,
    /// without running it. The form to use when the work ends where it
    /// began. Work that must carry its guard elsewhere, through a channel
    /// or into a spawned task, takes [`begin`](Self::begin) or
    /// [`begin_new`](Self::begin_new) instead.
    pub async fn run_new<F: Future>(
        &self,
        label: impl Into<String>,
        work: F,
    ) -> Result<F::Output, ShuttingDown> {
        let _running = self.begin_new(label)?;
        Ok(work.await)
    }

    /// Runs `task` on a timer: first after `first_after`, then every
    /// `interval`, until the tracker is closed. Each pass is new work under
    /// `label`, so one under way runs to its end and none starts after
    /// [`close`](Self::close).
    pub fn every<F, Fut>(
        &self,
        label: &'static str,
        first_after: Duration,
        interval: Duration,
        task: F,
    ) where
        F: Fn() -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send,
    {
        let work = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(first_after).await;
            while work.run_new(label, task()).await.is_ok() {
                tokio::time::sleep(interval).await;
            }
        });
    }

    /// Refuses new work from now on, and wakes what waits on
    /// [`closed`](Self::closed).
    pub fn close(&self) {
        self.state().closed = true;
        self.inner.closing.notify_waiters();
    }

    /// Waits until the tracker is closed: for work that stops, rather than
    /// runs to its end, when the daemon shuts down.
    pub async fn closed(&self) {
        // Created before the check, so a close between the two still wakes it.
        let closing = self.inner.closing.notified();
        if self.is_closed() {
            return;
        }
        closing.await;
    }

    pub fn is_closed(&self) -> bool {
        self.state().closed
    }

    /// Waits until no work is running.
    pub async fn idle(&self) {
        loop {
            // Created before the check, so an end between the two still wakes it.
            let ended = self.inner.idle.notified();
            if self.state().running.is_empty() {
                return;
            }
            ended.await;
        }
    }

    /// The work still running, oldest first.
    pub fn pending(&self) -> Vec<PendingWork> {
        self.state()
            .running
            .values()
            .map(|(label, started)| PendingWork {
                label: label.clone(),
                elapsed: started.elapsed(),
            })
            .collect()
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.inner
            .state
            .lock()
            .expect("the work lock is not poisoned")
    }
}

impl Drop for WorkGuard {
    fn drop(&mut self) {
        let mut state = self
            .inner
            .state
            .lock()
            .expect("the work lock is not poisoned");
        state.running.remove(&self.id);
        if state.running.is_empty() {
            self.inner.idle.notify_waiters();
        }
    }
}

#[cfg(test)]
mod tests;
