//! Workers that compute table formulas, off the async runtime. Each keeps
//! one [`FormulaRuntime`] with the formulas it compiled, started when first
//! needed and replaced after a job it stopped for time or memory, so a bad
//! formula costs only itself.

use std::sync::Arc;

use tokio::sync::oneshot;

use super::script_worker::ScriptWorker;
use crate::formula::{FAILED, Formula, FormulaOutput, FormulaRow, FormulaRun, FormulaRuntime};
use crate::sandbox::SandboxOptions;

/// A formula computes rather than waits, so two workers keep one slow
/// formula from holding up every table without letting formulas take
/// over the machine.
const WORKERS: usize = 2;

/// How many rows one job computes, so the jobs of several tables'
/// passes take turns on the workers.
pub(super) const ROWS_PER_JOB: usize = 64;

/// One formula to compute for some rows.
pub(super) struct FormulaJob {
    formula: Arc<Formula>,
    rows: Vec<FormulaRow>,
    reply: oneshot::Sender<FormulaRun>,
}

impl FormulaJob {
    /// Computes this job with `runtime`, starting one if it holds none.
    /// `runtime` is only put back after a job that didn't stop it.
    pub(super) fn run(self, runtime: &mut Option<FormulaRuntime>, options: SandboxOptions) {
        if self.reply.is_closed() {
            return;
        }
        let mut active = match runtime
            .take()
            .map_or_else(|| FormulaRuntime::new(options), Ok)
        {
            Ok(active) => active,
            Err(_) => {
                _ = self.reply.send(failed());
                return;
            }
        };
        let run = active.evaluate(&self.formula, &self.rows);
        if run.stopped.is_none() {
            *runtime = Some(active);
        }
        _ = self.reply.send(run);
    }
}

/// A job that couldn't run at all.
fn failed() -> FormulaRun {
    FormulaRun {
        outputs: Vec::new(),
        stopped: Some(FAILED),
    }
}

/// The workers, each job going to the least busy.
#[derive(Clone)]
pub(super) struct FormulaPool {
    workers: Arc<[ScriptWorker<FormulaJob>]>,
}

impl FormulaPool {
    pub(super) fn new() -> Self {
        Self::with_options(WORKERS, SandboxOptions::FORMULA)
    }

    pub(super) fn with_options(count: usize, options: SandboxOptions) -> Self {
        Self {
            workers: (0..count).map(|_| ScriptWorker::formula(options)).collect(),
        }
    }

    /// Computes `formula` for each of `rows`, [`ROWS_PER_JOB`] at a time,
    /// giving one output per row in order. Once a job is stopped for time
    /// or memory, its remaining rows and every later one fail with that, so
    /// a formula that never returns costs one time limit a pass.
    pub(super) async fn evaluate(
        &self,
        formula: Formula,
        rows: Vec<FormulaRow>,
    ) -> Vec<FormulaOutput> {
        let formula = Arc::new(formula);
        let total = rows.len();
        let mut outputs = Vec::with_capacity(total);
        let mut rows = rows.into_iter().peekable();
        while rows.peek().is_some() {
            let chunk: Vec<FormulaRow> = rows.by_ref().take(ROWS_PER_JOB).collect();
            let run = self.run(formula.clone(), chunk).await;
            outputs.extend(run.outputs);
            if let Some(error) = run.stopped {
                outputs.resize(total, Err(error.to_string()));
                break;
            }
        }
        outputs
    }

    async fn run(&self, formula: Arc<Formula>, rows: Vec<FormulaRow>) -> FormulaRun {
        let worker = self
            .workers
            .iter()
            .min_by_key(|worker| worker.pressure())
            .expect("the pool has at least one worker");
        let (reply, reply_rx) = oneshot::channel();
        let job = FormulaJob {
            formula,
            rows,
            reply,
        };
        if worker.submit(job).await.is_err() {
            return failed();
        }
        reply_rx.await.unwrap_or_else(|_| failed())
    }
}

#[cfg(test)]
mod tests;
