//! Bringing database formulas' outputs up to date. A write that outdates
//! them marks the rows and raises `DatabaseFormulasStale`, which
//! [`HookManager`](crate::hooks::HookManager) hands to a [`FormulaQueue`]
//! rather than to the hooks. The [`FormulaRunner`] runs one pass at a time
//! per database, and passes of different databases side by side: a pass
//! reads the marked rows, computes every formula for them on the script
//! engine's formula workers, and stores the outputs, which invalidates the
//! database again. An event arriving while its database's pass runs makes
//! that pass run once more. The marks outlive a restart, and
//! [`FormulaQueue::queue_stale`] queues what they name as the daemon starts.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use cohesion_core::log::Module;
use cohesion_core::log_error;
use cohesion_db::{
    CommandError, DatabaseContext, FormulaOutputs, ListDatabasesWithStaleFormulasCommand,
    ListStaleDatabaseRowsCommand, StoreDatabaseFormulaOutputsCommand, WorkGuard, WorkTracker,
};
use cohesion_js::{Formula, FormulaRow, ScriptEngine};
use tokio::sync::mpsc;

/// The sending side: databases whose formulas need a pass, each counted as
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

    /// Asks for a pass over database `database_id`'s outdated rows.
    pub(crate) fn queue(&self, database_id: i64) {
        let running = self
            .work
            .begin(format!("formulas of database {database_id}"));
        // The runner is gone only once the daemon is stopping; the marks
        // keep the work for the next start.
        let _ = self.tx.send((database_id, running));
    }

    /// Queues a pass for every database with outdated rows, which a daemon
    /// that stopped before its passes ran leaves.
    pub(crate) async fn queue_stale(&self, database: &DatabaseContext) {
        match database.send(ListDatabasesWithStaleFormulasCommand).await {
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
            // The databases with a pass running, each with whether another
            // was asked for meanwhile.
            let running: Arc<Mutex<HashMap<i64, bool>>> = Arc::default();
            while let Some((database_id, guard)) = self.0.recv().await {
                {
                    let mut running = running.lock().unwrap();
                    if let Some(again) = running.get_mut(&database_id) {
                        *again = true;
                        continue;
                    }
                    running.insert(database_id, false);
                }
                let running = running.clone();
                let database = database.clone();
                let engine = engine.clone();
                tokio::spawn(async move {
                    let _guard = guard;
                    loop {
                        if let Err(e) = evaluate_formulas(&database, &engine, database_id).await {
                            log_error!(
                                Module::Database,
                                "Computing the formulas of database {database_id} failed: {e:?}"
                            );
                        }
                        let again = {
                            let mut running = running.lock().unwrap();
                            let again = running
                                .get_mut(&database_id)
                                .map(std::mem::take)
                                .unwrap_or_default();
                            if !again {
                                running.remove(&database_id);
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

/// One pass over database `database_id`: computes every formula for each
/// row marked outdated and stores the outputs, clearing the marks no change
/// raised again meanwhile.
pub(crate) async fn evaluate_formulas(
    database: &DatabaseContext,
    engine: &ScriptEngine,
    database_id: i64,
) -> Result<(), CommandError> {
    let stale = database
        .send(ListStaleDatabaseRowsCommand { database_id })
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
        .send(StoreDatabaseFormulaOutputsCommand {
            database_id,
            rows: stale.rows.iter().map(|row| row.staleness).collect(),
            formulas,
        })
        .await
}

#[cfg(test)]
mod tests;
