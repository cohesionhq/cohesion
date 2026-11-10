//! The state every request handler runs against.

use std::sync::Arc;

use cohesion_core::ServerAddress;
use cohesion_core::log::Module;
use cohesion_core::log_error;
use cohesion_core::protocol::ServerError;
use cohesion_db::{
    ActivityTracker, Container, DatabaseContext, Secrets, ShutdownCleanupCommand, WorkTracker,
};
use cohesion_intelligence::{AgentCoordinator, ModelCatalog};
use cohesion_js::ScriptEngine;

use crate::actor_sessions::ActorSessions;
use crate::capability_clients::CapabilityClients;
use crate::connectors::{AgentConnectors, PendingSignIns};
use crate::database_formulas::FormulaQueue;
use crate::endpoint_relay::EndpointRelay;
use crate::extension_manager::{ExtensionManager, ScriptServices};
use crate::hooks::{HookManager, LaunchHook};
use crate::hostname::hostname;
use crate::pending_import::PendingImport;
use crate::presence::Presence;
use crate::script_actors::ScriptActors;
use crate::security::Security;

/// Everything a request handler needs: the database, the script engine,
/// the agents, the extensions, the clients and who is connected.
#[derive(Clone)]
pub struct CohesionContext {
    /// Runs database commands.
    pub database: DatabaseContext,
    /// Runs scripts; its agent pools reach connectors, and every pool and
    /// extension scope reaches services and actors.
    pub(crate) script_engine: ScriptEngine,
    pub(crate) hostname: Arc<String>,
    /// Opens pairing windows, whose keys the registration command checks.
    pub secrets: Secrets,
    /// What a setup code says about this server besides its key.
    pub(crate) server: ServerAddress,
    pub(crate) catalog: ModelCatalog,
    pub(crate) hooks: HookManager,
    /// Brings database formulas' outputs up to date.
    pub(crate) formulas: FormulaQueue,
    /// Runs agents: their conversations and the background runs hooks start.
    pub(crate) agents: AgentCoordinator,
    /// Installs, restarts and calls extensions, and keeps their updates.
    pub(crate) extensions: ExtensionManager,
    /// The clients serving actors now, and the calls waiting on them; actor
    /// sessions call through it.
    pub capability_clients: CapabilityClients,
    /// The sessions handed to clients' actors, and the activities that show
    /// them while they go on.
    pub actor_sessions: ActorSessions,
    /// The clients with a session open now, which the clients list reports.
    pub presence: Presence,
    /// Where the transports record refused pairings and when and from
    /// where each client last connected.
    pub security: Security,
    /// The PKCE verifier of each extension's sign-in under way.
    pub(crate) sign_ins: PendingSignIns,
    /// An import a client finished, which cohesiond restarts onto.
    pub pending_import: PendingImport,
    /// The work in flight: client requests, queued hooks, agent turns and
    /// runs, and periodic passes. [`shutdown`](Self::shutdown) closes it.
    pub work: WorkTracker,
}
impl CohesionContext {
    /// Starts the database worker for `container` and the script engine.
    pub fn new(container: Container, secrets: Secrets, server: ServerAddress) -> Self {
        let work = WorkTracker::default();
        let (formulas, formula_runner) = FormulaQueue::new(work.clone());
        let (hooks, manager) = HookManager::new(work.clone(), formulas.clone());
        let ctx = DatabaseContext::new(container, secrets.clone(), hooks.clone());
        // Initialize the engine here so that hooks can re-use the same one;
        // the extensions get a clone before connectors, services and actors,
        // which need the extensions or the clients, are set on it, and see
        // them once they are.
        let engine = ScriptEngine::new(ctx.read_write());
        let extensions = ExtensionManager::new(ctx.clone(), engine.clone(), hooks.clone());
        let capability_clients = CapabilityClients::new(Arc::new(ctx.clone()));
        let actor_sessions = ActorSessions::new(
            ctx.clone(),
            capability_clients.clone(),
            ActivityTracker::new(ctx.clone(), work.clone()),
        );
        let script_engine = engine
            .with_connectors(Arc::new(AgentConnectors::new(
                ctx.clone(),
                extensions.clone(),
            )))
            .with_services(Arc::new(ScriptServices::new(&extensions)))
            .with_actors(Arc::new(ScriptActors::new(
                actor_sessions.clone(),
                capability_clients.clone(),
            )));
        let agents = AgentCoordinator::new(
            ctx.clone(),
            script_engine.clone(),
            Arc::new(EndpointRelay::new(ctx.clone(), extensions.clone())),
            work.clone(),
        );
        // Start listening for hooks and stale formulas.
        manager.start(extensions.clone(), ctx.clone(), agents.clone());
        formula_runner.start(ctx.clone(), script_engine.clone());
        Self {
            hooks,
            formulas,
            agents,
            script_engine,
            secrets,
            server,
            hostname: Arc::new(hostname().unwrap_or("unknown".to_string())),
            catalog: ModelCatalog::new(ctx.clone()),
            extensions,
            capability_clients,
            actor_sessions,
            presence: Presence::new(ctx.clone()),
            security: Security::new(ctx.clone()),
            sign_ins: PendingSignIns::default(),
            pending_import: PendingImport::default(),
            database: ctx,
            work,
        }
    }
    /// Starts the daemon's own work, once, before it serves clients: runs
    /// the shutdown cleanup, for a daemon that stopped without closing its
    /// database (first, so nothing has started any work yet), then refreshes the model catalog in the background, queues
    /// the launch hooks and the formulas a stopped daemon left outdated, and
    /// starts the clock hooks and the periodic cleanup, resource, feed and
    /// extension-update passes.
    pub async fn start(&self) {
        if let Err(e) = self.database.send(ShutdownCleanupCommand).await {
            log_error!(Module::Database, "Startup cleanup failed: {e:?}");
        }
        let ctx = self.clone();
        tokio::spawn(async move { _ = ctx.refresh_catalog().await });
        self.hooks.send(LaunchHook {});
        self.formulas.queue_stale(&self.database).await;
        self.hooks.start_day_clock(self.database.clone());
        self.start_periodic_cleanup();
        self.start_resource_refresh();
        self.start_feed_refresh();
        self.start_update_checks();
    }
    /// Refreshes the model catalog, logging a failure; the caller only
    /// learns that something went wrong.
    pub(crate) async fn refresh_catalog(&self) -> Result<(), ServerError> {
        self.catalog.update().await.map_err(|e| {
            log_error!(Module::Intelligence, "Catalog update failed: {e:?}");
            ServerError::internal()
        })
    }
    /// Shuts down cleanly: refuses new work (client requests, clock hooks,
    /// periodic passes), which also cancels the activities, queues the
    /// settled hooks still waiting,
    /// waits for the work in flight and whatever it starts in turn (hooks,
    /// agent runs, scripts, model calls), then stops the script engine's
    /// worker threads and closes the database. `work` lists what
    /// is still running meanwhile.
    pub async fn shutdown(&self) {
        self.work.close();
        self.hooks.settle_all();
        self.work.idle().await;
        self.script_engine.shutdown().await;
        self.database.close().await;
    }
}
