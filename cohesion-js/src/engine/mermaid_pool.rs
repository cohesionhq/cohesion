//! Workers that draw Mermaid diagrams, off the async runtime. Each keeps one
//! [`MermaidRenderer`], loaded when first needed and replaced after a render
//! it didn't come through, so a bad diagram costs only itself.

use std::sync::Arc;
use std::time::{Duration, Instant};

use cohesion_core::log::Module;
use cohesion_core::log_warn;
use cohesion_core::protocol::{MermaidRefusal, RenderedMermaid};
use tokio::sync::oneshot;

use super::script_worker::ScriptWorker;
use crate::mermaid::{MermaidRenderer, refused};
use crate::sandbox::SandboxOptions;

/// Drawing computes rather than waits, and a typical diagram takes a
/// millisecond, so two workers keep one dense diagram from holding up the rest
/// without letting diagrams take over the machine.
const WORKERS: usize = 2;

/// How long a diagram may wait for a worker before it is refused, which bounds
/// a client's wait to this plus [`SandboxOptions::MERMAID`]'s time limit.
const QUEUE_LIMIT: Duration = Duration::from_secs(30);

/// One diagram waiting to be drawn.
pub(super) struct MermaidJob {
    source: String,
    width: Option<u32>,
    queued_at: Instant,
    reply: oneshot::Sender<RenderedMermaid>,
}

impl MermaidJob {
    /// Draws this diagram with `renderer`, starting one if it holds none.
    /// `renderer` is only put back after a render that left it intact, which
    /// a panic in this one doesn't.
    pub(super) fn run(
        self,
        renderer: &mut Option<MermaidRenderer>,
        options: SandboxOptions,
        queue_limit: Duration,
    ) {
        if self.reply.is_closed() {
            return;
        }
        if self.queued_at.elapsed() >= queue_limit {
            _ = self.reply.send(refused(
                MermaidRefusal::Busy,
                "Too many diagrams are being drawn.",
            ));
            return;
        }
        let active = match renderer
            .take()
            .map_or_else(|| MermaidRenderer::new(options), Ok)
        {
            Ok(active) => active,
            Err(stop) => {
                log_warn!(Module::Script, "Loading Pele failed: {:?}", stop.refused);
                _ = self.reply.send(stop.refused);
                return;
            }
        };
        let reply = match active.render(&self.source, self.width) {
            Ok(reply) => {
                *renderer = Some(active);
                reply
            }
            Err(stop) => stop.refused,
        };
        _ = self.reply.send(reply);
    }
}

/// The workers, each diagram going to the least busy.
#[derive(Clone)]
pub(super) struct MermaidPool {
    workers: Arc<[ScriptWorker<MermaidJob>]>,
}

impl MermaidPool {
    pub(super) fn new() -> Self {
        Self::with_limits(WORKERS, SandboxOptions::MERMAID, QUEUE_LIMIT)
    }

    fn with_limits(count: usize, options: SandboxOptions, queue_limit: Duration) -> Self {
        Self {
            workers: (0..count)
                .map(|_| ScriptWorker::mermaid(options, queue_limit))
                .collect(),
        }
    }

    pub(super) async fn render(&self, source: String, width: Option<u32>) -> RenderedMermaid {
        let worker = self
            .workers
            .iter()
            .min_by_key(|worker| worker.pressure())
            .expect("the pool has at least one worker");
        let (reply, reply_rx) = oneshot::channel();
        let job = MermaidJob {
            source,
            width,
            queued_at: Instant::now(),
            reply,
        };
        let failed = || refused(MermaidRefusal::Failed, "This diagram couldn’t be drawn.");
        if worker.submit(job).await.is_err() {
            return failed();
        }
        reply_rx.await.unwrap_or_else(|_| failed())
    }
}

#[cfg(test)]
mod tests;
