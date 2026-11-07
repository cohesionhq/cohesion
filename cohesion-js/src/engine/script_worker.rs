use std::borrow::Cow;
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering::Relaxed;
use std::thread;
use std::time::Duration;

use cohesion_core::log::Module;
use cohesion_core::{log_error, log_warn};
use cohesion_db::GuardedDatabaseContext;
use tokio::sync::{mpsc, oneshot};
use tokio::task;
use tokio::task::AbortHandle;
use tokio::task::JoinSet;
use tokio::time::Instant;
use tokio::time::MissedTickBehavior;
use tokio::time::interval_at;

use super::hosts::Hosts;
use super::mermaid_pool::MermaidJob;
use super::pressure_guard::PressureGuard;
use super::script_job::{ScriptJob, panic_message};
use crate::globals::{Mount, TimerRegistration};
use crate::sandbox::{Sandbox, SandboxOptions};
use crate::script_error::ScriptError;
use crate::script_stop::ScriptStop;

/// Runs one job on a worker's thread, logging a panic in it rather than
/// letting it end the thread, so it costs only that job.
fn survive_panic(run: impl FnOnce()) {
    if let Err(payload) = catch_unwind(AssertUnwindSafe(run)) {
        log_warn!(
            Module::Script,
            "A script job panicked: {}",
            panic_message(&*payload)
        );
    }
}

/// One script waiting to run: a [`ScriptJob`] on a persistent worker, a
/// [`FreshJob`] on an ephemeral one.
pub(super) struct ScriptRequest<J> {
    job: J,
    _pressure: PressureGuard,
}

/// What a fresh sandbox is built from: the database its scripts reach and
/// the globals it mounts.
#[derive(Clone)]
pub(super) struct SandboxSpec {
    pub(super) database: GuardedDatabaseContext,
    pub(super) mounts: Cow<'static, [Mount]>,
}

/// A job for an ephemeral worker, with the sandbox it runs in.
pub(super) struct FreshJob {
    pub(super) spec: SandboxSpec,
    pub(super) job: ScriptJob,
}

/// One worker thread of a [`ScriptEngine`](crate::ScriptEngine), running its
/// scripts in order: ephemeral, taking [`FreshJob`]s, or persistent, taking
/// [`ScriptJob`]s for its one sandbox.
pub(super) struct ScriptWorker<J = ScriptJob> {
    pub(super) tx: mpsc::Sender<ScriptRequest<J>>,
    pressure: Arc<AtomicI64>,
}

impl<J> Clone for ScriptWorker<J> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            pressure: self.pressure.clone(),
        }
    }
}

impl ScriptWorker<FreshJob> {
    /// A worker that builds a fresh sandbox for each job, from the job's
    /// [`SandboxSpec`], so no state carries over between scripts. A failed
    /// build goes to that job, and a panic in the build or the job costs
    /// only that job. Each sandbox is served by `hosts`; with `None`,
    /// `services.use` and `actors` throw.
    pub(super) fn ephemeral(options: SandboxOptions, hosts: Option<Hosts>) -> Self {
        let (tx, mut rx) = mpsc::channel::<ScriptRequest<FreshJob>>(64);
        thread::spawn(move || {
            while let Some(request) = rx.blocking_recv() {
                let FreshJob { spec, job } = request.job;
                let run = || match Sandbox::new(options, spec.database, &spec.mounts, None) {
                    Ok(sandbox) => {
                        if let Some(hosts) = &hosts {
                            sandbox.serve(hosts.clone());
                        }
                        job(Ok(&sandbox))
                    }
                    Err(error) => job(Err(error)),
                };
                survive_panic(run);
            }
        });
        Self {
            tx,
            pressure: Arc::new(AtomicI64::new(0)),
        }
    }
}

impl ScriptWorker<MermaidJob> {
    /// A worker that draws diagrams with one renderer it keeps between them,
    /// held to `options`. A panic in a job costs only that job.
    pub(super) fn mermaid(options: SandboxOptions, queue_limit: Duration) -> Self {
        let (tx, mut rx) = mpsc::channel::<ScriptRequest<MermaidJob>>(64);
        thread::spawn(move || {
            let mut renderer = None;
            while let Some(request) = rx.blocking_recv() {
                survive_panic(|| request.job.run(&mut renderer, options, queue_limit));
            }
        });
        Self {
            tx,
            pressure: Arc::new(AtomicI64::new(0)),
        }
    }
}

impl ScriptWorker<ScriptJob> {
    /// A worker whose jobs all share one sandbox, so globals and prototype
    /// changes persist between them. The sandbox is built before this
    /// returns; if that fails, so does this. A panic in a job costs only
    /// that job. `scope` names the worker in its timers' logs. The sandbox
    /// is served by `hosts`; a scope is trusted, so it has
    /// [`SandboxOptions::STANDARD`]. Once `closing` is stopped, a sleep in
    /// its scripts ends at once.
    pub(super) async fn persistent(
        scope: String,
        database: GuardedDatabaseContext,
        mounts: Cow<'static, [Mount]>,
        hosts: Hosts,
        closing: ScriptStop,
    ) -> Result<Self, ScriptError> {
        let (setup_tx, setup_rx) = oneshot::channel::<Result<(), ScriptError>>();
        let (tx, mut rx) = mpsc::channel::<ScriptRequest<ScriptJob>>(64);

        let worker = Self {
            tx,
            pressure: Arc::new(AtomicI64::new(0)),
        };

        let (send_timer, mut recv_timer) = mpsc::unbounded_channel::<TimerRegistration>();

        // Weak TX & Pressure cloned for the loop
        // These will be moved into the async context.
        let weak_tx = worker.tx.downgrade();
        let pressure = worker.pressure.clone();
        task::spawn(async move {
            let mut join_set = JoinSet::<()>::new();
            let mut handles = HashMap::<String, AbortHandle>::new();

            while let Some(timer) = recv_timer.recv().await {
                while let Some(result) = join_set.try_join_next() {
                    if let Err(error) = result
                        && error.is_panic()
                    {
                        log_warn!(Module::Script, "A timer task panicked: {error}")
                    }
                }

                let weak_tx = weak_tx.clone();
                let pressure = pressure.clone();

                let seconds = timer.seconds();
                let fn_name = timer.function_name.clone();
                let scope = scope.clone();

                let handle = join_set.spawn(async move {
                    let duration = Duration::from_secs(seconds);
                    let mut interval = interval_at(Instant::now() + duration, duration);

                    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

                    loop {
                        interval.tick().await;
                        // Upgrade so that the channel stays open during this send.
                        let Some(tx) = weak_tx.upgrade() else { break };
                        // Rebuild a worker.
                        let worker = ScriptWorker {
                            tx,
                            pressure: pressure.clone(),
                        };
                        // Execute function, await until it exits.
                        // This will prevent flooding, as the missed tick behavior
                        // is set to SKIP.
                        let fn_name = fn_name.clone();
                        let scope = scope.clone();
                        _ = worker
                            .submit(Box::new(move |sandbox| {
                                if let Err(err) = sandbox.and_then(|sbx| sbx.call(&fn_name)) {
                                    log_error!(
                                        Module::Script,
                                        "{scope}: timer {fn_name} failed: {err}"
                                    );
                                }
                            }))
                            .await;
                    }
                });
                if let Some(existing) = handles.remove(&timer.function_name) {
                    existing.abort();
                }
                handles.insert(timer.function_name, handle);
            }
        });

        thread::spawn(move || {
            let shared_sandbox = match Sandbox::new(
                SandboxOptions::STANDARD,
                database,
                &mounts,
                Some(send_timer),
            ) {
                Ok(shared) => {
                    shared.serve(hosts);
                    shared.closing_with(closing);
                    shared
                }
                Err(err) => {
                    _ = setup_tx.send(Err(err));
                    return;
                }
            };

            _ = setup_tx.send(Ok(()));

            while let Some(request) = rx.blocking_recv() {
                let run = || (request.job)(Ok(&shared_sandbox));
                survive_panic(run);
            }
        });

        setup_rx.await.map_err(|_| ScriptError::Cancelled)??;

        Ok(worker)
    }
}

impl<J> ScriptWorker<J> {
    /// Jobs queued, waiting for queue space, or running on this worker.
    pub(super) fn pressure(&self) -> i64 {
        self.pressure.load(Relaxed)
    }

    pub(super) async fn submit(&self, job: J) -> Result<(), ScriptError> {
        // Counted before waiting for queue space, so blocked callers add pressure.
        let pressure = PressureGuard::new(&self.pressure);
        let permit = self
            .tx
            .reserve()
            .await
            .map_err(|_| ScriptError::Cancelled)?;
        permit.send(ScriptRequest {
            job,
            _pressure: pressure,
        });
        Ok(())
    }
}
