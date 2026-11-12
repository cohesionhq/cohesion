use std::collections::HashMap;
use std::future::poll_fn;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::Poll;

use cohesion_core::log::Module;
use cohesion_core::model::{
    ActorSession, ActorSessionFollower, ActorSessionMove, HookKind, Initiative,
    IntelligenceAgentCue, TaskAssignee,
};
use cohesion_core::protocol::{AgentForwardTarget, ImageUpload};
use cohesion_core::{log_error, log_info};
use cohesion_db::{
    AgentTurnPhase, CommandError, CreateUserAgentMessageCommand, DatabaseContext,
    DatabaseEventTrace, DeleteAgentCueCommand, GetAgentMessageCommand, GetTaskCommand,
    ListAgentCuesCommand, ListAgentCuesForHookCommand, PickAgentChoiceCommand,
    RecordAgentTurnCommand, ResetAgentAgendaCommand, SetAgentMessageReactionCommand, WorkGuard,
    WorkTracker,
};
use cohesion_js::{CueEvaluation, ScriptArgument, ScriptEngine};
use tokio::sync::{OwnedMutexGuard, oneshot};

use super::agent_activity::Work;
use super::agent_error::AgentError;
use super::agent_host::AgentHost;
use super::agent_messenger::AgentMessenger;
use super::conversation_session;
use super::cue_failures;
use super::foreground_turn::{ForegroundTurn, Steered};
use crate::account_credentials::AccountCredentials;

/// Runs every agent: one host per agent, created on first use, whose lock
/// serialises that agent's foreground (different agents run concurrently),
/// plus the forks cues wake, which take turns per cue, the forks other
/// agents' questions wake, and the forks actor sessions the agents follow
/// wake. Every turn and run is
/// counted in the daemon's [`WorkTracker`] until it ends. A cheap clone.
#[derive(Clone)]
pub struct AgentCoordinator {
    inner: Arc<Inner>,
}

struct Inner {
    hosts: Mutex<HashMap<i64, Arc<tokio::sync::Mutex<AgentHost>>>>,
    /// Each agent's running turn, kept when its host is dropped, so a
    /// message or a stop reaches the turn the old host still runs.
    foreground: Mutex<HashMap<i64, Arc<ForegroundTurn>>>,
    /// One lock per cue with a fork going or waiting, so a fork sees what the
    /// one before it posted; removed once no fork holds or waits on it.
    cue_turns: Mutex<HashMap<i64, Arc<tokio::sync::Mutex<()>>>>,
    /// What every host's sessions are built from (the database, the script
    /// engine, the agents' activity, thread marks and the daemon's work),
    /// which carries agents' questions to each other and wakes their forks.
    messenger: AgentMessenger,
}

impl AgentCoordinator {
    /// Agents on `database`, running scripts in `engine`, counted in `work`;
    /// their sessions on an account-backed endpoint sign in through
    /// `credentials`.
    pub fn new(
        database: DatabaseContext,
        engine: ScriptEngine,
        credentials: Arc<dyn AccountCredentials>,
        work: WorkTracker,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                messenger: AgentMessenger::new(database, engine, credentials, work),
                hosts: Mutex::default(),
                foreground: Mutex::default(),
                cue_turns: Mutex::default(),
            }),
        }
    }

    /// Foreground: saves the user's message, the images attached to it, the
    /// item it forwards or the skill it runs, if any, and the workspace the
    /// user is in, `workspace_id`, which the message records when the agent
    /// last heard of another, then hands it to the turn the agent is running, which takes it in
    /// before its next model call, or else queues the agent's turn on it
    /// behind any it is answering, and returns. What the turn posts reaches
    /// clients as the agent's invalidations.
    pub async fn send_message(
        &self,
        agent_id: i64,
        message: String,
        attachments: Vec<ImageUpload>,
        forward: Option<AgentForwardTarget>,
        skill: Option<i64>,
        workspace_id: Option<i64>,
    ) -> Result<(), AgentError> {
        let queued = self.queue(agent_id).await?;
        let (message, attachments) = self
            .database()
            .send(CreateUserAgentMessageCommand {
                agent_id,
                text: message,
                attachments,
                forward,
                skill,
                workspace_id,
            })
            .await?;
        // Taken in, the message gives up its place in the queue.
        if let Some(steered) = self.foreground(agent_id).steer(Steered {
            message,
            attachments,
        }) {
            self.answer_later(queued, steered).await;
        }
        Ok(())
    }

    /// Stops the turn agent `agent_id` is running, if it is, and any fork of
    /// its conversation answering another agent: the scripts they run are
    /// interrupted, and they make no further model call.
    pub fn stop_turn(&self, agent_id: i64) {
        self.foreground(agent_id).stop();
        self.inner.messenger.stop_forks(agent_id);
    }

    /// Runs the agent's turn on `steered` once `queued`'s turn comes. Boxed,
    /// since a turn ending queues the messages it never took in.
    fn answer_later(
        &self,
        queued: Queued,
        Steered {
            message,
            attachments,
        }: Steered,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        let counted = self.counted(queued.agent_id, "message");
        Box::pin(
            self.spawn(queued, counted, move |mut host, work| async move {
                host.answer_message(message, attachments, &work).await
            }),
        )
    }

    /// Records the user's pick of option `option` in a pending choice, then
    /// queues the agent's turn on it and returns.
    pub async fn respond_to_choice(
        &self,
        message_id: i64,
        option: usize,
    ) -> Result<(), AgentError> {
        let agent_id = self
            .database()
            .send(GetAgentMessageCommand { message_id })
            .await?
            .agent_id;
        let queued = self.queue(agent_id).await?;
        let choice = self
            .database()
            .send(PickAgentChoiceCommand { message_id, option })
            .await?;
        self.spawn(
            queued,
            self.counted(agent_id, "choice"),
            move |mut host, work| async move { host.hear_pick(choice, &work).await },
        )
        .await;
        Ok(())
    }

    /// Records the user's reaction to one of an agent's messages, or clears
    /// it. A reaction queues the agent's turn on it, which may post nothing.
    pub async fn react(&self, message_id: i64, reaction: Option<String>) -> Result<(), AgentError> {
        let agent_id = self
            .database()
            .send(GetAgentMessageCommand { message_id })
            .await?
            .agent_id;
        let queued = self.queue(agent_id).await?;
        let message = self
            .database()
            .send(SetAgentMessageReactionCommand {
                message_id,
                reaction,
            })
            .await?;
        if message.reaction.is_some() {
            self.spawn(
                queued,
                self.counted(agent_id, "reaction"),
                move |mut host, work| async move { host.hear_reaction(message, &work).await },
            )
            .await;
        }
        Ok(())
    }

    /// Takes a place in the agent's foreground queue now, so turns run in
    /// the order their requests came.
    async fn queue(&self, agent_id: i64) -> Result<Queued, AgentError> {
        let host = self.host(agent_id).await?;
        let mut lock: Pin<Box<dyn Future<Output = _> + Send>> = Box::pin(host.lock_owned());
        // Polled once, which joins the lock's FIFO queue of waiters.
        let ready = poll_fn(|cx| Poll::Ready(lock.as_mut().poll(cx))).await;
        Ok(Queued {
            agent_id,
            lock,
            ready,
        })
    }

    /// Runs `job` on the host once `queued`'s turn comes, as the agent's
    /// work, on its own task, raising `agent:turn:start` as it starts and
    /// `agent:turn:end` once it ends, however it went, under a fresh trace.
    /// `counted`, taken while the request that
    /// queued the turn still runs, counts it as the daemon's work
    /// until it ends, so a shutdown waits for it. Returns once the
    /// agent's activity shows it working, so a client refetching after the
    /// request sees it, and doesn't wait for the job. A job that panics
    /// drops the host and leaves an `error` message, as a failed turn does.
    async fn spawn<F>(
        &self,
        queued: Queued,
        counted: WorkGuard,
        job: impl FnOnce(OwnedMutexGuard<AgentHost>, Work) -> F + Send + 'static,
    ) where
        F: Future<Output = Result<(), AgentError>> + Send + 'static,
    {
        let Queued {
            agent_id,
            lock,
            ready,
        } = queued;
        let coordinator = self.clone();
        let (begun_tx, begun) = oneshot::channel();
        let database = self.database().clone();
        let queuer = self.clone();
        tokio::spawn(async move {
            let result = coordinator
                .inner
                .messenger
                .activity()
                .scope(agent_id, counted, Some(begun_tx), move |work| async move {
                    let host = match ready {
                        Poll::Ready(host) => host,
                        Poll::Pending => lock.await,
                    };
                    let foreground = queuer.foreground(agent_id);
                    let began = foreground.begin(DatabaseEventTrace::default());
                    record_turn(&database, agent_id, AgentTurnPhase::Started).await;
                    // On its own task, so a panic still ends the turn.
                    let result = tokio::spawn(job(host, work))
                        .await
                        .unwrap_or(Err(AgentError::Panicked));
                    let left = foreground.end(began);
                    record_turn(&database, agent_id, AgentTurnPhase::Ended).await;
                    // Messages a stopped turn never took in get turns of
                    // their own, in the order they came.
                    for steered in left {
                        match queuer.queue(agent_id).await {
                            Ok(queued) => queuer.answer_later(queued, steered).await,
                            Err(e) => log_error!(
                                Module::Intelligence,
                                "Agent {agent_id}: could not queue a message's turn: {e:?}"
                            ),
                        }
                    }
                    result
                })
                .await;
            match result {
                Ok(()) => {}
                Err(AgentError::Panicked) => {
                    coordinator.evict(agent_id).await;
                    _ = AgentError::Panicked
                        .post(coordinator.database(), agent_id)
                        .await;
                }
                Err(e) => log_error!(
                    Module::Intelligence,
                    "Agent {agent_id}: could not run a turn: {e:?}"
                ),
            }
        });
        _ = begun.await;
    }

    /// Clears agent `agent_id`'s agenda at the user's request, as its own
    /// `agenda_reset` does. Its running threads hear `Agenda was cleared.`
    /// in their next catch-up.
    pub async fn reset_agenda(&self, agent_id: i64) -> Result<(), AgentError> {
        self.database()
            .send(ResetAgentAgendaCommand {
                agent_id,
                run_id: None,
            })
            .await?;
        self.foreground(agent_id).agenda().publish(&[]);
        Ok(())
    }

    /// Removes cue `cue_id` at the user's request, as the agent's own
    /// `removeCue` does. The host is dropped so its next thread's status
    /// block no longer lists the cue.
    pub async fn remove_cue(&self, cue_id: i64) -> Result<(), AgentError> {
        let cue = self
            .database()
            .send(DeleteAgentCueCommand { cue_id })
            .await?;
        self.evict(cue.agent_id).await;
        Ok(())
    }

    /// Drops the agent's cached host, so the next message rebuilds it (after
    /// the agent is updated or deleted), and the thread its forks would copy
    /// and its clients see.
    pub async fn evict(&self, agent_id: i64) {
        self.hosts().remove(&agent_id);
        if let Err(e) = self.inner.messenger.marks().forget(agent_id).await {
            log_error!(
                Module::Intelligence,
                "Agent {agent_id}: could not forget its thread marks: {e:?}"
            );
        }
        if let Err(e) = conversation_session::forget_thread(self.database(), agent_id).await {
            log_error!(
                Module::Intelligence,
                "Agent {agent_id}: could not forget its thread: {e:?}"
            );
        }
    }

    /// Evaluates every cue on `hook` against the event's `arguments` and
    /// wakes a fork for each cue that fires, except for the cues
    /// of an agent whose own write caused the event; a cue whose function
    /// fails says so in its agent's feed, every time
    /// ([`cue_failures::report`]); and, for an event that
    /// moved an actor session, wakes a fork of the conversation of the agent
    /// following it, when the move is one it hears of; and, for an event that
    /// assigned a task to an agent (or created it assigned), wakes a fork of
    /// that agent's conversation to work on it, unless the agent assigned
    /// it itself or a job did. Several cues of one
    /// agent can fire on one event, each waking its own fork; forks of one
    /// cue wait for the one before, in the order they fired. Model calls run
    /// on their own tasks, so this returns once the cues are evaluated.
    pub async fn fire(
        &self,
        hook: HookKind,
        arguments: Vec<ScriptArgument>,
        trace: &DatabaseEventTrace,
    ) {
        match (&hook, arguments.as_slice()) {
            (
                _,
                [
                    ScriptArgument::ActorSession {
                        actor_session,
                        moved,
                    },
                ],
            ) => {
                self.wake_follower(actor_session, *moved, trace).await;
            }
            (
                HookKind::TaskAssigned | HookKind::TaskCreate,
                [
                    ScriptArgument::Task {
                        workspace_id,
                        task_id,
                    },
                ],
            ) => self.wake_assignee(*workspace_id, *task_id, trace).await,
            _ => {}
        }
        let cues = match self
            .database()
            .send(ListAgentCuesForHookCommand { hook: hook.clone() })
            .await
        {
            Ok(cues) => cues,
            Err(e) => {
                log_error!(
                    Module::Intelligence,
                    "Could not list agent cues for {hook}: {e:?}"
                );
                return;
            }
        };
        for cue in cues {
            let agent_id = cue.agent_id;
            if trace.is_from_agent(agent_id) {
                continue;
            }
            let evaluation = self
                .inner
                .messenger
                .engine()
                .evaluate_cue(
                    agent_id,
                    trace.clone(),
                    cue.script.clone(),
                    arguments.clone(),
                )
                .await;
            match evaluation {
                Ok(evaluation) if evaluation.fired => {
                    log_info!(
                        Module::Intelligence,
                        "Agent {agent_id}: cue \"{}\" fired on {hook}.",
                        cue.name
                    );
                    let coordinator = self.clone();
                    let trace = trace.clone();
                    // Taken before the spawn, while the hook's own work still
                    // runs, so a shutdown waits for this fork too.
                    let counted = self.counted(agent_id, &format!("cue \"{}\"", cue.name));
                    tokio::spawn(async move {
                        coordinator.run_cue(cue, evaluation, trace, counted).await;
                    });
                }
                Ok(_) => {}
                Err(e) => {
                    log_error!(
                        Module::Intelligence,
                        "Agent {agent_id}: cue \"{}\" on {hook} failed: {e}",
                        cue.name
                    );
                    let reported =
                        cue_failures::report(self.database(), &cue, &e, trace.clone()).await;
                    if let Err(e) = reported {
                        log_error!(
                            Module::Intelligence,
                            "Agent {agent_id}: could not report cue \"{}\" failing: {e:?}",
                            cue.name
                        );
                    }
                }
            }
        }
    }

    /// Wakes a fork of the conversation of the agent following `actor_session`, which
    /// an event under `trace` just moved as `moved` says, when one follows it
    /// and the move is one it hears of: the actor_session stopped on its own, never
    /// started working or was stopped.
    async fn wake_follower(
        &self,
        actor_session: &ActorSession,
        moved: ActorSessionMove,
        trace: &DatabaseEventTrace,
    ) {
        let Some(ActorSessionFollower { agent_id, .. }) = actor_session.follower else {
            return;
        };
        let Some(stop) = moved.stop() else {
            return;
        };
        log_info!(
            Module::Intelligence,
            "Agent {agent_id}: actor session {} it follows stopped: {stop:?}.",
            actor_session.id
        );
        self.inner
            .messenger
            .follow_up(agent_id, actor_session.clone(), stop, trace)
            .await;
    }

    /// Wakes a fork of the conversation of the agent task `task_id` is
    /// assigned to, which an event under `trace` just assigned (or created
    /// assigned), to work on it, with the user's rights: an assignment by
    /// the user, a script, or another agent's turn or fork wakes it. The
    /// agent's own write wakes nothing, nor does a chain an event started
    /// (a job): a job never wakes a fork, and a chain never gains rights.
    /// The task is read as it is now, so one reassigned or deleted since
    /// wakes whoever has it, or no one.
    async fn wake_assignee(&self, workspace_id: i64, task_id: i64, trace: &DatabaseEventTrace) {
        let task = match self
            .database()
            .send(GetTaskCommand {
                workspace_id,
                task_id,
            })
            .await
        {
            Ok(task) => task,
            Err(CommandError::NotFound(_)) => return,
            Err(e) => {
                log_error!(
                    Module::Intelligence,
                    "Could not read task {task_id} to wake its assignee: {e:?}"
                );
                return;
            }
        };
        let Some(TaskAssignee::Agent { agent_id }) = task.assignee else {
            return;
        };
        if trace.is_from_agent(agent_id) || trace.initiative() == Initiative::Event {
            return;
        }
        log_info!(
            Module::Intelligence,
            "Agent {agent_id}: task {task_id} was assigned to it."
        );
        self.inner.messenger.assigned(agent_id, task, trace).await;
    }

    /// Wakes `cue`'s fork once the fork the cue woke before is done, unless
    /// the user removed the cue meanwhile.
    async fn run_cue(
        &self,
        cue: IntelligenceAgentCue,
        evaluation: CueEvaluation,
        trace: DatabaseEventTrace,
        counted: WorkGuard,
    ) {
        let (agent_id, cue_id) = (cue.agent_id, cue.id);
        let turn = self.cue_turn(cue_id);
        let guard = turn.lock().await;
        // The user may have removed the cue meanwhile.
        let cues = self
            .database()
            .send(ListAgentCuesCommand { agent_id })
            .await;
        match cues {
            Ok(cues) if cues.iter().any(|current| current.id == cue_id) => {
                self.run_cue_fork(cue, evaluation, trace, counted).await;
            }
            Ok(_) => log_info!(
                Module::Intelligence,
                "Agent {agent_id}: cue \"{}\" was removed before its fork woke.",
                cue.name
            ),
            Err(e) => log_error!(
                Module::Intelligence,
                "Agent {agent_id}: could not list its cues: {e:?}"
            ),
        }
        drop(guard);
        self.release_cue_turn(cue_id, turn);
    }

    async fn run_cue_fork(
        &self,
        cue: IntelligenceAgentCue,
        evaluation: CueEvaluation,
        trace: DatabaseEventTrace,
        counted: WorkGuard,
    ) {
        let agent_id = cue.agent_id;
        let messenger = self.inner.messenger.clone();
        let result = self
            .inner
            .messenger
            .activity()
            .scope(agent_id, counted, None, move |work| async move {
                messenger
                    .cue_fired(work, &cue, &evaluation, trace)
                    .await
                    .map_err(AgentError::Command)
            })
            .await;
        if let Err(e) = result {
            log_error!(
                Module::Intelligence,
                "Agent {agent_id}: cue fork failed: {e:?}"
            );
        }
    }

    fn database(&self) -> &DatabaseContext {
        self.inner.messenger.database()
    }

    /// Counts work for the agent in the daemon's tracker, labelled `what`,
    /// until the guard is dropped; [`AgentActivity::scope`](super::agent_activity::AgentActivity::scope) holds it for as
    /// long as the work runs.
    fn counted(&self, agent_id: i64, what: &str) -> WorkGuard {
        self.inner
            .messenger
            .work()
            .begin(format!("agent {agent_id}: {what}"))
    }

    /// The agent's host, created from the database on first use.
    async fn host(&self, agent_id: i64) -> Result<Arc<tokio::sync::Mutex<AgentHost>>, AgentError> {
        if let Some(host) = self.hosts().get(&agent_id) {
            return Ok(host.clone());
        }
        let host = AgentHost::new(
            agent_id,
            self.inner.messenger.clone(),
            self.foreground(agent_id),
        )
        .await?;
        Ok(self
            .hosts()
            .entry(agent_id)
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(host)))
            .clone())
    }

    /// Agent `agent_id`'s running turn, as requests reach it.
    fn foreground(&self, agent_id: i64) -> Arc<ForegroundTurn> {
        self.inner
            .foreground
            .lock()
            .expect("the foreground turns lock is not poisoned")
            .entry(agent_id)
            .or_insert_with(|| Arc::new(self.inner.messenger.turn(agent_id)))
            .clone()
    }

    /// The lock forks of cue `cue_id` take turns on.
    fn cue_turn(&self, cue_id: i64) -> Arc<tokio::sync::Mutex<()>> {
        self.cue_turns().entry(cue_id).or_default().clone()
    }

    /// Hands back `turn`, cue `cue_id`'s, once a fork is done with it, and
    /// forgets it when no other fork holds a copy. Copies are only made
    /// under the map's lock, so none can appear between the count and the
    /// removal.
    fn release_cue_turn(&self, cue_id: i64, turn: Arc<tokio::sync::Mutex<()>>) {
        let mut turns = self.cue_turns();
        // This copy and the map's.
        if Arc::strong_count(&turn) == 2 {
            turns.remove(&cue_id);
        }
    }

    fn cue_turns(&self) -> std::sync::MutexGuard<'_, HashMap<i64, Arc<tokio::sync::Mutex<()>>>> {
        self.inner
            .cue_turns
            .lock()
            .expect("the cue turns lock is not poisoned")
    }

    fn hosts(&self) -> std::sync::MutexGuard<'_, HashMap<i64, Arc<tokio::sync::Mutex<AgentHost>>>> {
        self.inner
            .hosts
            .lock()
            .expect("the hosts lock is not poisoned")
    }
}

/// Raises `phase` of a turn of agent `agent_id`'s, traced as the agent's own
/// so its cues don't fire on it.
async fn record_turn(database: &DatabaseContext, agent_id: i64, phase: AgentTurnPhase) {
    let trace = DatabaseEventTrace::default().from_agent(agent_id);
    let recorded = database
        .send_traced(RecordAgentTurnCommand { agent_id, phase }, trace)
        .await;
    if let Err(e) = recorded {
        log_error!(
            Module::Intelligence,
            "Agent {agent_id}: could not record its turn: {e:?}"
        );
    }
}

/// A place in an agent's foreground queue: the lock on its host, polled
/// once (`ready` holds the host when it was free).
struct Queued {
    agent_id: i64,
    lock: Pin<Box<dyn Future<Output = OwnedMutexGuard<AgentHost>> + Send>>,
    ready: Poll<OwnedMutexGuard<AgentHost>>,
}

#[cfg(test)]
mod tests;
