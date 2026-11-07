//! Shutting the daemon down cleanly: what asks for it, and waiting out the
//! work in flight while reporting what still holds it up.

use std::time::Duration;

use cohesionkit::log::Module;
use cohesionkit::{CohesionContext, log_info, log_warn};
use tokio::time::{Instant, interval_at, sleep};

/// How long the work in flight gets before the daemon exits anyway.
const LIMIT: Duration = Duration::from_secs(5 * 60);
/// How often the work still running is reported.
const REPORT: Duration = Duration::from_secs(5);

/// The signals that stop the daemon: Ctrl-C, and `SIGTERM` on Unix.
/// Listening starts when it is created, so a signal is never missed between
/// two waits.
pub struct Signals {
    #[cfg(unix)]
    interrupt: tokio::signal::unix::Signal,
    #[cfg(unix)]
    terminate: tokio::signal::unix::Signal,
}
impl Signals {
    #[cfg(unix)]
    pub fn new() -> Self {
        use tokio::signal::unix::{SignalKind, signal};
        Self {
            interrupt: signal(SignalKind::interrupt()).expect("can listen for SIGINT"),
            terminate: signal(SignalKind::terminate()).expect("can listen for SIGTERM"),
        }
    }
    #[cfg(not(unix))]
    pub fn new() -> Self {
        Self {}
    }

    /// Waits for the next signal and names it.
    #[cfg(unix)]
    async fn next(&mut self) -> &'static str {
        tokio::select! {
            _ = self.interrupt.recv() => "SIGINT",
            _ = self.terminate.recv() => "SIGTERM",
        }
    }
    #[cfg(not(unix))]
    async fn next(&mut self) -> &'static str {
        let _ = tokio::signal::ctrl_c().await;
        "Ctrl-C"
    }
}

/// Waits for a reason to shut down and says what it was: a signal or, with
/// `with_parent`, stdin reaching end of file. The parent holds the pipe's
/// write end, and the kernel closes it however the parent exits.
pub async fn requested(signals: &mut Signals, with_parent: bool) -> String {
    let parent = async {
        match with_parent {
            true => {
                let _ = tokio::io::copy(&mut tokio::io::stdin(), &mut tokio::io::sink()).await;
            }
            false => std::future::pending().await,
        }
    };
    tokio::select! {
        signal = signals.next() => format!("Received {signal}"),
        _ = parent => "Parent process exited".to_string(),
    }
}

/// Runs the context's clean shutdown, logging every few seconds the work it
/// still waits on. A second signal, or [`LIMIT`] passing, gives up on the
/// work but still closes the database, whose last write is the shutdown
/// cleanup; returns whether the work finished.
pub async fn run(context: &CohesionContext, signals: &mut Signals) -> bool {
    let started = Instant::now();
    let shutdown = context.shutdown();
    tokio::pin!(shutdown);
    let deadline = sleep(LIMIT);
    tokio::pin!(deadline);
    let mut report = interval_at(started + REPORT, REPORT);
    loop {
        tokio::select! {
            _ = &mut shutdown => {
                log_info!(Module::Server, "cohesiond stopped cleanly.");
                return true;
            }
            _ = report.tick() => report_pending(context, started.elapsed()),
            _ = &mut deadline => {
                log_warn!(
                    Module::Server,
                    "Gave up after {}; stopping with work still running.",
                    elapsed(LIMIT)
                );
                report_pending(context, LIMIT);
                context.database.close().await;
                return false;
            }
            signal = signals.next() => {
                log_warn!(Module::Server, "Received {signal} while shutting down; stopping now.");
                context.database.close().await;
                return false;
            }
        }
    }
}

/// Logs the work still running, one line each, oldest first.
fn report_pending(context: &CohesionContext, waited: Duration) {
    let pending = context.work.pending();
    log_info!(
        Module::Server,
        "Shutting down ({} / {}): waiting for {} {}; Ctrl-C to stop now",
        elapsed(waited),
        elapsed(LIMIT),
        pending.len(),
        match pending.len() {
            1 => "job",
            _ => "jobs",
        }
    );
    for work in pending {
        log_info!(
            Module::Server,
            "  {} (running {})",
            work.label,
            elapsed(work.elapsed)
        );
    }
}

/// `42s` or `3min 5s`.
fn elapsed(duration: Duration) -> String {
    let seconds = duration.as_secs();
    match (seconds / 60, seconds % 60) {
        (0, seconds) => format!("{seconds}s"),
        (minutes, 0) => format!("{minutes}min"),
        (minutes, seconds) => format!("{minutes}min {seconds}s"),
    }
}
