use std::borrow::Cow;
use std::cmp::max;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock};
use std::thread;

use super::agent_role::AgentRole;
use super::cue_evaluation::CueEvaluation;
use super::formula_pool::FormulaPool;
use super::hosts::Hosts;
use super::mermaid_pool::MermaidPool;
use super::scope_handle::ScopeHandle;
use super::script_job::replying;
use super::script_worker::{FreshJob, SandboxSpec, ScriptWorker};
use crate::actor_access::ActorAccess;
use crate::connector_access::ConnectorAccess;
use crate::formula::{Formula, FormulaOutput, FormulaRow};
use crate::globals::{Effects, ForegroundEffect, Mount};
use crate::sandbox::{Sandbox, SandboxOptions, ScriptArgument};
use crate::script_error::ScriptError;
use crate::script_output::ScriptOutput;
use crate::script_stop::ScriptStop;
use crate::service_access::ServiceAccess;
use cohesion_core::log::Module;
use cohesion_core::log_warn;
use cohesion_core::model::Initiative;
use cohesion_core::protocol::RenderedMermaid;
use cohesion_db::{DatabaseEventTrace, GuardedDatabaseContext, SandboxKey};

/// Runs scripts on a pool of worker threads, each script on the least busy
/// worker. Cheap to clone; clones share the pool.
#[derive(Clone)]
pub struct ScriptEngine {
    /// User-space, trusted workers. This worker
    /// is in ephemeral mode, meaning every invocation
    /// creates a new context.
    general_purpose: Pool,
    /// Agents' turns, as [`AgentRole::Turn`] mounts them: a turn an event
    /// started brings its own sandbox.
    turns: Pool,
    /// Agents' cues, as [`AgentRole::Cue`] mounts them.
    cues: Pool,
    /// The database every pool's is narrowed from, as each [`AgentRole`]
    /// says.
    database: GuardedDatabaseContext,
    /// Untrusted, isolated, with no globals (but types), held to
    /// [`SandboxOptions::ISOLATED`]'s much smaller limits.
    /// Used for untrusted sources (like an extension, when vetting it).
    isolated: Pool,
    /// Draws Mermaid diagrams, which reach neither the database nor a mount.
    mermaid: MermaidPool,
    /// Computes table formulas, which reach neither the database nor a
    /// mount either.
    formulas: FormulaPool,
    /// Keyed by their sandbox, such as `extension:com.example.mailer`.
    /// These workers are persistent and are not re-created.
    scoped: Arc<Mutex<HashMap<SandboxKey, Scope>>>,
    /// What agent jobs reach connector extensions through, shared by all
    /// clones; `connectors` throws until
    /// [`with_connectors`](Self::with_connectors).
    connectors: Arc<OnceLock<Arc<dyn ConnectorAccess>>>,
    /// What `services` and `actors` reach services and actors through,
    /// shared by all clones: attached to every sandbox but the isolated
    /// worker's; each throws until [`with_services`](Self::with_services)
    /// or [`with_actors`](Self::with_actors).
    hosts: Hosts,
}

/// What `cohesion run` and other general-purpose scripts mount.
pub const GENERAL_PURPOSE_MOUNTS: &[Mount] = &[
    Mount::Cohesion,
    Mount::Intelligence,
    Mount::Log,
    Mount::Services,
    Mount::Notifications,
    Mount::ArticleFetch,
    Mount::Sleep,
];
/// A spawned scope's worker, and the stop that cuts its scripts' sleeps
/// short once it shuts down.
struct Scope {
    worker: ScriptWorker,
    closing: ScriptStop,
}

impl Drop for Scope {
    fn drop(&mut self) {
        self.closing.stop();
    }
}

/// Ephemeral workers and the sandbox their jobs get unless one brings its
/// own.
#[derive(Clone)]
struct Pool {
    workers: Arc<[ScriptWorker<FreshJob>]>,
    spec: SandboxSpec,
}

impl Pool {
    /// Ends every worker's thread once its queue has drained.
    async fn stop(&self) {
        for worker in self.workers.iter() {
            worker.stop().await;
        }
    }

    /// `count` workers held to `options`, whose sandboxes are served by
    /// `hosts`, building `spec`'s sandbox by default.
    fn new(count: usize, options: SandboxOptions, spec: SandboxSpec, hosts: Option<Hosts>) -> Self {
        Self {
            workers: (0..count)
                .map(|_| ScriptWorker::ephemeral(options, hosts.clone()))
                .collect(),
            spec,
        }
    }

    /// Runs `job` in a fresh sandbox of the pool's own spec.
    async fn run<T: Send + 'static>(
        &self,
        job: impl FnOnce(&Sandbox) -> Result<T, ScriptError> + Send + 'static,
    ) -> Result<T, ScriptError> {
        self.run_in(self.spec.clone(), job).await
    }

    /// Runs `job` in a fresh sandbox built from `spec`, on the least busy
    /// worker, and waits for its result.
    async fn run_in<T: Send + 'static>(
        &self,
        spec: SandboxSpec,
        job: impl FnOnce(&Sandbox) -> Result<T, ScriptError> + Send + 'static,
    ) -> Result<T, ScriptError> {
        let worker = self
            .workers
            .iter()
            .min_by_key(|worker| worker.pressure())
            .expect("the pool has at least one worker");
        let (job, reply_rx) = replying(job);
        worker.submit(FreshJob { spec, job }).await?;
        reply_rx.await.map_err(|_| ScriptError::Cancelled)?
    }
}

impl ScriptEngine {
    /// Starts one worker per core (at least four) in each pool, and twice
    /// that in the cues pool, whose scripts reach the database through
    /// `database`, narrowed as each agent role says. The isolated worker gets
    /// a read-only copy of it.
    /// Stops every worker thread, each once the scripts queued on it have
    /// run, and the scoped workers with them: a script submitted after is
    /// cancelled. The daemon calls it as it shuts down; without it the
    /// threads outlive the engine, since its clones hold one another.
    pub async fn shutdown(&self) {
        for pool in [
            &self.general_purpose,
            &self.turns,
            &self.cues,
            &self.isolated,
        ] {
            pool.stop().await;
        }
        self.mermaid.stop().await;
        self.formulas.stop().await;
        let scoped: Vec<Scope> = self
            .scoped
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .drain()
            .map(|(_, scope)| scope)
            .collect();
        for scope in scoped {
            scope.worker.stop().await;
        }
    }

    pub fn new(database: GuardedDatabaseContext) -> Self {
        let cores = thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        let hosts = Hosts::default();
        // A worker spends most of its time blocked on a fetch, a service or a
        // connector, not computing, so the count is about how many scripts
        // may wait at once, not the cores; a few slow calls must not stall
        // every agent and cue.
        let workers = max(cores, 4);
        let pool =
            |count: usize, database: GuardedDatabaseContext, mounts: Cow<'static, [Mount]>| {
                Pool::new(
                    count,
                    SandboxOptions::STANDARD,
                    SandboxSpec { database, mounts },
                    Some(hosts.clone()),
                )
            };
        let role =
            |role: AgentRole, count: usize| pool(count, role.database(&database), role.mounts());

        Self {
            isolated: Pool::new(
                1,
                SandboxOptions::ISOLATED,
                SandboxSpec {
                    database: database.read_only(),
                    mounts: Cow::Borrowed(&[]),
                },
                None,
            ),
            general_purpose: pool(
                workers,
                database.clone(),
                Cow::Borrowed(GENERAL_PURPOSE_MOUNTS),
            ),
            turns: role(AgentRole::Turn(Initiative::User), workers),
            // Twice the others: every hook's cues are evaluated here, `timer`'s
            // each minute.
            cues: role(AgentRole::Cue, workers * 2),
            mermaid: MermaidPool::new(),
            formulas: FormulaPool::new(),
            database,
            scoped: Arc::new(Mutex::new(HashMap::new())),
            connectors: Arc::default(),
            hosts,
        }
    }

    /// This engine, whose agent jobs reach connector extensions through
    /// `access`. Like [`with_services`](Self::with_services), it reaches
    /// every clone of the engine, those made before included. Set once.
    pub fn with_connectors(self, access: Arc<dyn ConnectorAccess>) -> Self {
        assert!(
            self.connectors.set(access).is_ok(),
            "an engine's connectors are set once"
        );
        self
    }

    /// This engine, whose scripts' `services` reach installed services
    /// through `access`. It reaches every clone of the engine, those made
    /// before included, so extensions' scopes, spawned through an earlier
    /// clone, reach services too. Set once.
    pub fn with_services(self, access: Arc<dyn ServiceAccess>) -> Self {
        assert!(
            self.hosts.services.access.set(access).is_ok(),
            "an engine's services are set once"
        );
        self
    }

    /// This engine, whose scripts' `actors` reach actors through `access`.
    /// Like [`with_services`](Self::with_services), it reaches every clone
    /// of the engine, those made before included. Set once.
    pub fn with_actors(self, access: Arc<dyn ActorAccess>) -> Self {
        assert!(
            self.hosts.actors.0.set(access).is_ok(),
            "an engine's actors are set once"
        );
        self
    }

    /// Draws a Mermaid diagram laid out for `width`, or says why it won't draw.
    pub async fn render_mermaid(&self, source: String, width: Option<u32>) -> RenderedMermaid {
        self.mermaid.render(source, width).await
    }

    /// Computes table formula `formula` for each of `rows`, one output
    /// per row in order, in a runtime with nothing mounted: the row in, a
    /// value of the formula's type out, under [`SandboxOptions::FORMULA`]'s
    /// limits. A row whose call throws, runs too long or returns another
    /// type gets why, in words for the user.
    pub async fn evaluate_formula(
        &self,
        formula: Formula,
        rows: Vec<FormulaRow>,
    ) -> Vec<FormulaOutput> {
        self.formulas.evaluate(formula, rows).await
    }

    /// Starts a persistent worker for `scope` with `mounts`, reaching the
    /// database through `database`, runs `script` in it, and answers a
    /// handle on it. A scope it replaces shuts down.
    pub async fn spawn_scope(
        &self,
        scope: SandboxKey,
        script: String,
        mounts: Cow<'static, [Mount]>,
        database: GuardedDatabaseContext,
    ) -> Result<ScopeHandle, ScriptError> {
        let closing = ScriptStop::default();
        let instance = ScriptWorker::persistent(
            scope.to_string(),
            database,
            mounts,
            self.hosts.clone(),
            closing.clone(),
        )
        .await?;
        run_on(&instance, move |sandbox| sandbox.evaluate(script)).await?;
        let handle = ScopeHandle {
            worker: instance.clone(),
        };
        if self
            .scoped
            .lock()
            .expect("mutex is not poisoned")
            .insert(
                scope,
                Scope {
                    worker: instance,
                    closing,
                },
            )
            .is_some()
        {
            log_warn!(
                Module::Script,
                "Replaced an existing scope. If not reloading an extension, this is probably a bug."
            )
        }

        Ok(handle)
    }

    /// Forgets `scope`, so no new lookup finds it, and cuts short any sleep
    /// its scripts are in; a job already sent to it still runs.
    pub fn shutdown_scope(&self, scope: &SandboxKey) {
        self.scoped
            .lock()
            .expect("mutex is not poisoned")
            .remove(scope);
    }

    /// Whether `scope` was spawned and its worker is still running.
    pub fn is_scope_running(&self, scope: &SandboxKey) -> bool {
        self.scope(scope).is_some()
    }

    pub async fn general_purpose<T: Send + 'static>(
        &self,
        job: impl FnOnce(&Sandbox) -> Result<T, ScriptError> + Send + 'static,
    ) -> Result<T, ScriptError> {
        self.general_purpose.run(job).await
    }

    pub async fn isolated<T: Send + 'static>(
        &self,
        job: impl FnOnce(&Sandbox) -> Result<T, ScriptError> + Send + 'static,
    ) -> Result<T, ScriptError> {
        self.isolated.run(job).await
    }

    /// A handle on `scope`, while it was spawned and its worker is still
    /// running.
    pub fn scope(&self, scope: &SandboxKey) -> Option<ScopeHandle> {
        self.scoped
            .lock()
            .expect("the mutex isn't poisoned")
            .get(scope)
            .filter(|scope| !scope.worker.tx.is_closed())
            .map(|scope| ScopeHandle {
                worker: scope.worker.clone(),
            })
    }

    pub async fn general_purpose_run(&self, script: String) -> Result<ScriptOutput, ScriptError> {
        let trace = DatabaseEventTrace::default().from_script();
        self.general_purpose(move |sandbox| {
            sandbox.traced(trace, |sandbox| sandbox.evaluate(script))
        })
        .await
    }

    /// Runs `script` as a turn the user started ([`AgentRole::Turn`]), with its
    /// writes traced as coming from agent `agent_id`, so the agent's own cues can skip them, and
    /// `storage` reaching that agent's store, until `stop` stops it. Effects are discarded if the
    /// script fails. Its writes start a trace of their own.
    pub async fn foreground_agent_run(
        &self,
        agent_id: i64,
        script: String,
        stop: ScriptStop,
    ) -> Result<(ScriptOutput, Effects<ForegroundEffect>), ScriptError> {
        self.traced_foreground_agent_run(agent_id, script, stop, DatabaseEventTrace::default())
            .await
    }

    /// Runs `script` as [`foreground_agent_run`](Self::foreground_agent_run) does, for a turn of
    /// agent `agent_id`'s conversation or a fork of it, with the rights of whoever started
    /// `trace`'s chain ([`AgentRole::Turn`]): what it writes continues `trace`, the turn's (a
    /// question's, when another agent's question woke the fork), so agents asking each other stop
    /// at the depth limit.
    pub async fn traced_foreground_agent_run(
        &self,
        agent_id: i64,
        script: String,
        stop: ScriptStop,
        trace: DatabaseEventTrace,
    ) -> Result<(ScriptOutput, Effects<ForegroundEffect>), ScriptError> {
        let role = AgentRole::Turn(trace.initiative());
        let spec = SandboxSpec {
            database: role.database(&self.database),
            mounts: role.mounts(),
        };
        let trace = trace.from_agent(agent_id);
        let connectors = self.connectors.get().cloned();
        self.turns
            .run_in(spec, move |sandbox| {
                sandbox.stoppable(stop, |sandbox| {
                    sandbox.storing(SandboxKey::Agent(agent_id), |sandbox| {
                        sandbox.with_connectors(connectors, |sandbox| {
                            sandbox.traced(trace, |sandbox| {
                                sandbox.collecting_effects(|sandbox| sandbox.evaluate(script))
                            })
                        })
                    })
                })
            })
            .await
    }

    /// Evaluates `code` to a function as [`AgentRole::Cue`] and calls it with `arguments`
    /// inflated. Errors if `code` does not evaluate to a function. With read-only access, only
    /// its services' read-only functions succeed. Its `storage` reaches agent `agent_id`'s store.
    /// Like a job, what it raises continues `trace`, the hook's, as coming from the agent.
    pub async fn evaluate_cue(
        &self,
        agent_id: i64,
        trace: DatabaseEventTrace,
        code: String,
        arguments: Vec<ScriptArgument>,
    ) -> Result<CueEvaluation, ScriptError> {
        let trace = trace.from_agent(agent_id);
        self.cues
            .run(move |sandbox| {
                sandbox.storing(SandboxKey::Agent(agent_id), |sandbox| {
                    sandbox.traced(trace, |sandbox| sandbox.evaluate_cue(code, &arguments))
                })
            })
            .await
    }
}

/// Runs `job` on persistent `worker` and waits for its result.
pub(super) async fn run_on<T: Send + 'static>(
    worker: &ScriptWorker,
    job: impl FnOnce(&Sandbox) -> Result<T, ScriptError> + Send + 'static,
) -> Result<T, ScriptError> {
    let (job, reply_rx) = replying(job);
    worker.submit(job).await?;
    reply_rx.await.map_err(|_| ScriptError::Cancelled)?
}
