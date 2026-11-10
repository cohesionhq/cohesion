//! Bringing table formulas' outputs up to date. A write that outdates
//! them marks the rows and raises `TableFormulasStale`, which
//! [`HookManager`](crate::hooks::HookManager) hands to a [`FormulaQueue`]
//! rather than to the hooks. The [`FormulaRunner`] runs one pass at a time
//! per table, and passes of different tables side by side: a pass
//! reads the marked rows, computes every formula for them on the script
//! engine's formula workers, and stores the outputs, which invalidates the
//! table again. An event arriving while its table's pass runs makes
//! that pass run once more. The marks outlive a restart, and
//! [`FormulaQueue::queue_stale`] queues what they name as the daemon starts.
//! Formulas whose update mode is `continuously` are marked as the daemon
//! starts and every hour after ([`CohesionContext::start_continuous_formulas`]),
//! so their passes follow the same way.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cohesion_core::log::Module;
use cohesion_core::log_error;
use cohesion_db::{
    CommandError, DatabaseContext, FormulaOutputs, ListStaleTableRowsCommand,
    ListTablesWithStaleFormulasCommand, MarkContinuousFormulasStaleCommand,
    StoreTableFormulaOutputsCommand, WorkGuard, WorkTracker,
};
use cohesion_js::{Formula, FormulaRow, ScriptEngine};
use tokio::sync::mpsc;

use crate::cohesion_context::CohesionContext;

/// How often formulas whose update mode is `continuously` are computed
/// again.
const CONTINUOUS_INTERVAL: Duration = Duration::from_secs(60 * 60);

impl CohesionContext {
    /// Marks the live documents of every table with a continuous formula
    /// as outdated as the daemon starts, then every hour until it shuts
    /// down; each table marked raises `TableFormulasStale`, which queues
    /// its pass. Hours passed while the daemon was stopped, or the computer
    /// slept, aren't made up beyond the run at start.
    pub(crate) fn start_continuous_formulas(&self) {
        let database = self.database.clone();
        self.work.every(
            "continuous formulas",
            Duration::ZERO,
            CONTINUOUS_INTERVAL,
            move || {
                let database = database.clone();
                async move { mark_continuous_formulas(&database).await }
            },
        );
    }
}

/// One hourly run: marks the live documents of every table with a
/// continuous formula as outdated, logging a failure.
pub(crate) async fn mark_continuous_formulas(database: &DatabaseContext) {
    if let Err(e) = database.send(MarkContinuousFormulasStaleCommand).await {
        log_error!(
            Module::Database,
            "Marking continuous formulas outdated failed: {e:?}"
        );
    }
}

/// The sending side: tables (document folders) whose formulas need a pass, each counted as
/// work until its pass has run.
#[derive(Clone, Debug)]
pub(crate) struct FormulaQueue {
    tx: mpsc::UnboundedSender<(i64, WorkGuard)>,
    work: WorkTracker,
}

impl FormulaQueue {
    pub(crate) fn new(work: WorkTracker) -> (FormulaQueue, FormulaRunner) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self { tx, work }, FormulaRunner(rx))
    }

    /// Asks for a pass over the outdated documents of folder `folder_id`.
    pub(crate) fn queue(&self, folder_id: i64) {
        let running = self
            .work
            .begin(format!("formulas of document folder {folder_id}"));
        // The runner is gone only once the daemon is stopping; the marks
        // keep the work for the next start.
        let _ = self.tx.send((folder_id, running));
    }

    /// Queues a pass for every table with outdated documents, which a daemon
    /// that stopped before its passes ran leaves.
    pub(crate) async fn queue_stale(&self, database: &DatabaseContext) {
        match database.send(ListTablesWithStaleFormulasCommand).await {
            Ok(ids) => ids.into_iter().for_each(|id| self.queue(id)),
            Err(e) => log_error!(Module::Database, "Listing stale formulas failed: {e:?}"),
        }
    }
}

/// The receiving side, started once the database and script engine are.
pub(crate) struct FormulaRunner(mpsc::UnboundedReceiver<(i64, WorkGuard)>);

impl FormulaRunner {
    pub(crate) fn start(mut self, database: DatabaseContext, engine: ScriptEngine) {
        tokio::spawn(async move {
            // The tables with a pass running, each with whether another
            // was asked for meanwhile.
            let running: Arc<Mutex<HashMap<i64, bool>>> = Arc::default();
            while let Some((folder_id, guard)) = self.0.recv().await {
                {
                    let mut running = running.lock().unwrap();
                    if let Some(again) = running.get_mut(&folder_id) {
                        *again = true;
                        continue;
                    }
                    running.insert(folder_id, false);
                }
                let running = running.clone();
                let database = database.clone();
                let engine = engine.clone();
                tokio::spawn(async move {
                    let _guard = guard;
                    loop {
                        if let Err(e) = evaluate_formulas(&database, &engine, folder_id).await {
                            log_error!(
                                Module::Database,
                                "Computing the formulas of document folder {folder_id} failed: {e:?}"
                            );
                        }
                        let again = {
                            let mut running = running.lock().unwrap();
                            let again = running
                                .get_mut(&folder_id)
                                .map(std::mem::take)
                                .unwrap_or_default();
                            if !again {
                                running.remove(&folder_id);
                            }
                            again
                        };
                        if !again {
                            break;
                        }
                    }
                });
            }
        });
    }
}

/// One pass over the table of folder `folder_id`: computes every formula
/// for each document marked outdated and stores the outputs, clearing the marks no change
/// raised again meanwhile.
pub(crate) async fn evaluate_formulas(
    database: &DatabaseContext,
    engine: &ScriptEngine,
    folder_id: i64,
) -> Result<(), CommandError> {
    let stale = database
        .send(ListStaleTableRowsCommand { folder_id })
        .await?;
    if stale.rows.is_empty() {
        return Ok(());
    }
    let rows: Vec<FormulaRow> = stale.rows.iter().map(|row| row.inputs.clone()).collect();
    let mut formulas = Vec::with_capacity(stale.formulas.len());
    for formula in stale.formulas {
        let outputs = engine
            .evaluate_formula(
                Formula {
                    property_id: formula.property_id,
                    source: formula.source.clone(),
                    data_type: formula.data_type,
                },
                rows.clone(),
            )
            .await;
        formulas.push(FormulaOutputs { formula, outputs });
    }
    database
        .send(StoreTableFormulaOutputsCommand {
            folder_id,
            rows: stale.rows.iter().map(|row| row.staleness).collect(),
            formulas,
        })
        .await
}

#[cfg(test)]
mod tests;
