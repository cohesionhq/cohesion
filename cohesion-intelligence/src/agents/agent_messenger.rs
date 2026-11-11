use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cohesion_core::log::Module;
use cohesion_core::model::{
    ActorSession, ActorSessionStop, AgentAskReply, AgentMessageContent, AgentWake, HookKind,
    Initiative, IntelligenceAgentCue, IntelligenceAgentMessageSender, Task,
};
use cohesion_core::{log_error, log_info};
use cohesion_db::{
    ASK_STOPPED_REASON, AnswerAgentAskCommand, AskAgentCommand, CommandError,
    CreateAgentForkRunCommand, CreateAgentMessageCommand, CreateAgentRunCommand, DatabaseContext,
    DatabaseEventTrace, WorkGuard, WorkTracker,
};
use cohesion_js::{CueEvaluation, ScriptEngine};
use tokio::sync::oneshot;

use super::agenda::Agendas;
use super::agent_activity::{AgentActivity, Work};
use super::agent_fork::{self, Fork, Question};
use super::conversation_mark::ConversationMarks;
use super::foreground_turn::ForegroundTurn;
use super::wake_description::wake_description;
use crate::account_credentials::AccountCredentials;
use crate::spend_limits::SpendLimits;
use crate::turn_input::TurnInput;

/// The longest an agent waits in its turn for another's reply; the agent
/// asked carries on past it, but its reply then reaches no one.
pub(super) const ASK_LIMIT: Duration = Duration::from_secs(10 * 60);

/// Carries one agent's question to another: records it in both
/// conversations and wakes a fork of the asked agent's conversation to
/// answer it, as that agent's work, the conversation itself carrying on
/// meanwhile. A question that asks for a reply gets it through the fork's
/// `reply_to_agent`, which goes to the asker's session while it waits; a
/// one-way question gets none, and nothing ever wakes the asker. Wakes the
/// forks that follow up actor sessions, and that work on tasks assigned to an
/// agent, too. Holds what a fork is built from,
/// which is what the conversation's sessions are built from. A cheap clone.
#[derive(Clone)]
pub(super) struct AgentMessenger {
    database: DatabaseContext,
    engine: ScriptEngine,
    credentials: Arc<dyn AccountCredentials>,
    /// What keeps forks from model calls once a spend limit is reached,
    /// shared with forks.
    spend: Arc<SpendLimits>,
    activity: Arc<AgentActivity>,
    work: WorkTracker,
    marks: ConversationMarks,
    /// The sessions waiting for a reply, by the `askedAgent` message's id.
    /// Whoever takes a waiter decides where its reply goes, so each reply
    /// reaches the asker at most once.
    waiters: Arc<Mutex<HashMap<i64, oneshot::Sender<AgentAskReply>>>>,
    /// Each agent's running forks, which the user's stop of the agent stops
    /// along with its conversation's turn.
    forks: Arc<Mutex<HashMap<i64, Vec<Arc<ForegroundTurn>>>>>,
    /// Each agent's agenda, which its threads hear one another change.
    agendas: Agendas,
}

/// Whether a question asks for a reply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AskMode {
    /// The asker waits, and the fork answers with `reply_to_agent`.
    Reply,
    /// The fork does what it asks and answers no one: the asker has
    /// nothing to wait for, and nothing wakes it.
    OneWay,
}

/// One agent's question to another.
pub(super) struct Ask {
    pub asker_id: i64,
    /// The asker's run when a fork of its conversation asks; `None` from the
    /// conversation itself.
    pub asker_run_id: Option<i64>,
    pub recipient_id: i64,
    pub question: String,
    /// The asker's turn's trace, which the fork continues one hop further.
    pub trace: DatabaseEventTrace,
}

/// A question asked for a reply, and the way to wait for it. Dropped
/// without it, the reply, when it comes, reaches no one.
pub(super) struct Asked {
    /// The `askedAgent` message in the asker's conversation.
    pub message_id: i64,
    reply: oneshot::Receiver<AgentAskReply>,
    messenger: AgentMessenger,
}

impl Asked {
    /// Waits for the reply until it comes, `limit` passes, `turn` is
    /// stopped, or the daemon shuts down (which answers that it stopped).
    /// `None` once the wait is given up.
    pub(super) async fn wait(
        mut self,
        limit: Duration,
        turn: &ForegroundTurn,
    ) -> Option<AgentAskReply> {
        tokio::select! {
            reply = &mut self.reply => return reply.ok(),
            () = tokio::time::sleep(limit) => {}
            () = turn.stopped() => {}
            () = self.messenger.closed() => {
                return Some(AgentAskReply::Unanswered {
                    reason: ASK_STOPPED_REASON.to_string(),
                });
            }
        }
        // A fork that took the waiter already is sending the reply here, so
        // hear it out.
        if self.messenger.waiters().remove(&self.message_id).is_some() {
            return None;
        }
        (&mut self.reply).await.ok()
    }
}

/// Gives up the wait, so a reply still to come finds no waiter.
impl Drop for Asked {
    fn drop(&mut self) {
        self.messenger.waiters().remove(&self.message_id);
    }
}

/// The reply a fork owes the agent waiting on its question: settled once,
/// by the fork's `reply_to_agent`, or by the fork ending without it.
pub(super) struct OwedReply {
    messenger: AgentMessenger,
    /// The question's two messages, the asker's and the fork's.
    asked: i64,
    asked_by: i64,
    settled: AtomicBool,
}

impl OwedReply {
    /// Settles the question with `reply` in both conversations and hands it
    /// to the asker if it still waits. `false`, changing nothing, once
    /// settled already.
    pub(super) async fn settle(&self, reply: AgentAskReply) -> bool {
        if self.settled.swap(true, Ordering::SeqCst) {
            return false;
        }
        let settled = self
            .messenger
            .database
            .send(AnswerAgentAskCommand {
                asked: self.asked,
                asked_by: self.asked_by,
                reply: reply.clone(),
            })
            .await;
        if let Err(e) = settled {
            log_error!(
                Module::Intelligence,
                "Could not record the reply to question [#{}]: {e:?}",
                self.asked
            );
        }
        let waiter = self.messenger.waiters().remove(&self.asked);
        if let Some(waiter) = waiter {
            _ = waiter.send(reply);
        }
        true
    }

    /// Whether the reply has been given.
    pub(super) fn is_settled(&self) -> bool {
        self.settled.load(Ordering::SeqCst)
    }
}

impl AgentMessenger {
    /// Agents on `database`, running scripts in `engine`, counted in `work`,
    /// their sessions on an account-backed endpoint signing in through
    /// `credentials`.
    pub(super) fn new(
        database: DatabaseContext,
        engine: ScriptEngine,
        credentials: Arc<dyn AccountCredentials>,
        work: WorkTracker,
    ) -> Self {
        Self {
            activity: Arc::new(AgentActivity::new(database.clone())),
            marks: ConversationMarks::new(database.clone()),
            spend: Arc::default(),
            database,
            engine,
            credentials,
            work,
            waiters: Arc::default(),
            forks: Arc::default(),
            agendas: Agendas::default(),
        }
    }

    /// Asks `ask.recipient_id` `ask.question` for `ask.asker_id`, for a
    /// reply the asker's session can wait for, and returns as soon as the
    /// fork answering it has started. Refused as [`hand_off`](Self::hand_off) is.
    pub(super) async fn ask(&self, ask: Ask) -> Result<Asked, String> {
        let (reply_tx, reply) = oneshot::channel();
        let message_id = self.start(ask, AskMode::Reply, Some(reply_tx)).await?;
        Ok(Asked {
            message_id,
            reply,
            messenger: self.clone(),
        })
    }

    /// Hands `ask.recipient_id` `ask.question` from `ask.asker_id` one way:
    /// the fork does what it asks and replies to no one. Returns the
    /// `askedAgent` message's id once the fork has started. Refused, with a
    /// sentence for the model, when the agent asked is the asker or doesn't
    /// exist, or when the daemon is shutting down.
    pub(super) async fn hand_off(&self, ask: Ask) -> Result<i64, String> {
        self.start(ask, AskMode::OneWay, None).await
    }

    /// Records `ask` and starts the fork answering it in `mode`, the
    /// asker's session waiting on `waiter` when one is given.
    async fn start(
        &self,
        ask: Ask,
        mode: AskMode,
        waiter: Option<oneshot::Sender<AgentAskReply>>,
    ) -> Result<i64, String> {
        let asked = hop(
            &ask.trace,
            ask.recipient_id,
            &format!("agent {}'s question", ask.asker_id),
        );
        let woken = asked.clone().from_agent(ask.recipient_id);
        let counted = self
            .work
            .begin_new(format!("agent {}: answering", ask.recipient_id))
            .map_err(|_| "Cohesion is shutting down; ask again once it is back.".to_string())?;
        let record = self
            .database
            .send_traced(
                AskAgentCommand {
                    asker_id: ask.asker_id,
                    asker_run_id: ask.asker_run_id,
                    recipient_id: ask.recipient_id,
                    question: ask.question.clone(),
                },
                asked,
            )
            .await
            .map_err(|e| refusal(ask.recipient_id, e))?;
        let message_id = record.asked.id;
        let recipient_id = ask.recipient_id;
        if let Some(waiter) = waiter {
            self.waiters().insert(message_id, waiter);
        }
        let owed = (mode == AskMode::Reply).then(|| {
            Arc::new(OwedReply {
                messenger: self.clone(),
                asked: record.asked.id,
                asked_by: record.asked_by.id,
                settled: AtomicBool::new(false),
            })
        });
        let question = Question {
            fork: self.fork(recipient_id, record.run.id, woken),
            record,
            text: ask.question,
            owed,
        };
        self.spawn_fork(
            recipient_id,
            counted,
            "answer a question".to_string(),
            move |messenger, work| agent_fork::answer(messenger, work, question),
        );
        Ok(message_id)
    }

    /// Wakes a fork of agent `agent_id`'s conversation, which follows
    /// `task`, to carry on the work the conversation handed it now that the
    /// actor_session stopped as `stop` says, one hop from `trace`, the moving
    /// event's. Returns once the fork has started, as the agent's work; once
    /// the daemon is shutting down, it wakes nothing.
    pub(super) async fn follow_up(
        &self,
        agent_id: i64,
        actor_session: ActorSession,
        stop: ActorSessionStop,
        trace: &DatabaseEventTrace,
    ) {
        let actor_session_id = actor_session.id;
        let wake = AgentWake::ActorSession {
            actor_session_id,
            title: actor_session.title.clone(),
            stop,
        };
        self.wake_fork(
            agent_id,
            &format!("following actor session {actor_session_id}"),
            HookKind::ActorMoved,
            wake,
            trace,
            move |messenger, work, fork| {
                agent_fork::follow_up(messenger, work, fork, actor_session, stop)
            },
        )
        .await;
    }

    /// Wakes a fork of agent `agent_id`'s conversation to work on `task`,
    /// which someone other than the agent just assigned it, one hop from
    /// `trace`, the assigning event's, a chain the user started: being
    /// assigned the task is their go-ahead, so the fork has the
    /// conversation's rights. Returns once the fork has
    /// started, as the agent's work; once the daemon is shutting down, it
    /// wakes nothing.
    pub(super) async fn assigned(&self, agent_id: i64, task: Task, trace: &DatabaseEventTrace) {
        let wake = AgentWake::AssignedTask {
            workspace_id: task.workspace_id,
            task_id: task.id,
            title: task.title.clone(),
        };
        self.wake_fork(
            agent_id,
            &format!("task {} assigned to it", task.id),
            HookKind::TaskAssigned,
            wake,
            trace,
            move |messenger, work, fork| agent_fork::assigned(messenger, work, fork, task),
        )
        .await;
    }

    /// Wakes a fork of agent `agent_id`'s conversation for `what`, recorded
    /// as a run of `hook` with no cue and a line saying it woke `by`, then
    /// runs `fork` in it on its own task, one hop from `trace` (an event's,
    /// already a level deep, so the wake adds none). Returns once the fork
    /// has started, as the agent's work; once the daemon is shutting down,
    /// it wakes nothing.
    async fn wake_fork<F>(
        &self,
        agent_id: i64,
        what: &str,
        hook: HookKind,
        by: AgentWake,
        trace: &DatabaseEventTrace,
        fork: impl FnOnce(AgentMessenger, Work, Fork) -> F + Send + 'static,
    ) where
        F: Future<Output = ()> + Send + 'static,
    {
        let woken = hop(trace, agent_id, what).from_agent(agent_id);
        let Ok(counted) = self.work.begin_new(format!("agent {agent_id}: {what}")) else {
            return;
        };
        let run = match self
            .database
            .send_traced(CreateAgentForkRunCommand { agent_id, hook }, woken.clone())
            .await
        {
            Ok(run) => run,
            Err(e) => {
                log_error!(
                    Module::Intelligence,
                    "Agent {agent_id}: could not record a fork for {what}: {e:?}"
                );
                return;
            }
        };
        let noted = self
            .note_woke(agent_id, run.id, None, by, woken.clone())
            .await;
        if let Err(e) = noted {
            log_error!(
                Module::Intelligence,
                "Agent {agent_id}: could not note why it woke for {what}: {e:?}"
            );
        }
        let woke = self.fork(agent_id, run.id, woken);
        self.spawn_fork(
            agent_id,
            counted,
            format!("run its fork for {what}"),
            move |messenger, work| fork(messenger, work, woke),
        );
    }

    /// Notes in agent `agent_id`'s feed, under `trace`, that its run `run_id`
    /// woke `by` something.
    async fn note_woke(
        &self,
        agent_id: i64,
        run_id: i64,
        run_description: Option<String>,
        by: AgentWake,
        trace: DatabaseEventTrace,
    ) -> Result<(), CommandError> {
        self.database
            .send_traced(
                CreateAgentMessageCommand {
                    agent_id,
                    run_id: Some(run_id),
                    run_description,
                    sender: IntelligenceAgentMessageSender::Agent,
                    content: AgentMessageContent::Woke { by },
                    reply_to: None,
                },
                trace,
            )
            .await
            .map(drop)
    }

    /// A fork of agent `agent_id`'s conversation for its run `run_id`, its
    /// turn begun under `trace` and counted among the agent's running forks
    /// before anything runs, so the user's stop from here on reaches it.
    fn fork(&self, agent_id: i64, run_id: i64, trace: DatabaseEventTrace) -> Fork {
        let turn = Arc::new(self.turn(agent_id));
        turn.begin(trace);
        Fork {
            agent_id,
            run_id,
            running: self.running_fork(agent_id, turn.clone()),
            turn,
        }
    }

    /// Runs `job`, a fork of agent `agent_id`'s, on its own task as the
    /// agent's work, `counted` until it ends, logging that it could not
    /// `what` if its work fails to run.
    fn spawn_fork<F>(
        &self,
        agent_id: i64,
        counted: WorkGuard,
        what: String,
        job: impl FnOnce(AgentMessenger, Work) -> F + Send + 'static,
    ) where
        F: Future<Output = ()> + Send + 'static,
    {
        let messenger = self.clone();
        tokio::spawn(async move {
            let activity = messenger.activity.clone();
            let ran = activity
                .scope(agent_id, counted, None, move |work| async move {
                    job(messenger, work).await;
                    Ok(())
                })
                .await;
            if let Err(e) = ran {
                log_error!(
                    Module::Intelligence,
                    "Agent {agent_id}: could not {what}: {e:?}"
                );
            }
        });
    }

    /// Wakes a fork of `cue`'s agent's conversation, as `work`, for `cue`,
    /// which fired on `evaluation`'s subject under `trace`, the hook's: the
    /// start of a chain an event began, so the fork only reads. A reached
    /// spend limit keeps it from starting at all, and [`SpendLimits`] tells
    /// the feed. Records a run for it, and the line saying why it woke.
    pub(super) async fn cue_fired(
        &self,
        work: Work,
        cue: &IntelligenceAgentCue,
        evaluation: &CueEvaluation,
        trace: DatabaseEventTrace,
    ) -> Result<(), CommandError> {
        let agent_id = cue.agent_id;
        let trace = trace.started_by(Initiative::Event).from_agent(agent_id);
        if self
            .spend
            .reached(&self.database, agent_id, &trace)
            .await?
            .is_some()
        {
            log_info!(
                Module::Intelligence,
                "Agent {agent_id}: cue \"{}\" woke nothing: a spend limit is reached.",
                cue.name
            );
            return Ok(());
        }
        let run_description = wake_description(&cue.hook, &evaluation.subject);
        let run = self
            .database
            .send(CreateAgentRunCommand {
                agent_id,
                hook: cue.hook.clone(),
                cue_name: cue.name.clone(),
                thread_id: None,
            })
            .await?;
        let by = AgentWake::Cue {
            cue_name: cue.name.clone(),
            hook: cue.hook.clone(),
        };
        self.note_woke(agent_id, run.id, Some(run_description), by, trace.clone())
            .await?;
        let fork = self.fork(agent_id, run.id, trace);
        agent_fork::cue(self.clone(), work, fork, cue, &evaluation.payload).await;
        Ok(())
    }

    /// Counts `turn`, a fork's, among agent `agent_id`'s running forks until
    /// the guard drops.
    fn running_fork(&self, agent_id: i64, turn: Arc<ForegroundTurn>) -> RunningFork {
        self.forks().entry(agent_id).or_default().push(turn.clone());
        RunningFork {
            forks: self.forks.clone(),
            agent_id,
            turn,
        }
    }

    /// Stops every fork of agent `agent_id`'s that is running, as the user's
    /// stop of the agent does along with its conversation's turn.
    pub(super) fn stop_forks(&self, agent_id: i64) {
        let forks = self.forks();
        for fork in forks.get(&agent_id).into_iter().flatten() {
            fork.stop();
        }
    }

    /// Waits until the daemon starts shutting down, which stops every fork.
    pub(super) async fn closed(&self) {
        self.work.closed().await;
    }

    pub(super) fn database(&self) -> &DatabaseContext {
        &self.database
    }

    pub(super) fn engine(&self) -> &ScriptEngine {
        &self.engine
    }

    pub(super) fn credentials(&self) -> &Arc<dyn AccountCredentials> {
        &self.credentials
    }

    pub(super) fn spend(&self) -> &Arc<SpendLimits> {
        &self.spend
    }

    /// A new turn for one of agent `agent_id`'s threads, hearing what its
    /// other threads change of its agenda.
    pub(super) fn turn(&self, agent_id: i64) -> ForegroundTurn {
        ForegroundTurn::new(self.agendas.of(agent_id))
    }

    pub(super) fn marks(&self) -> &ConversationMarks {
        &self.marks
    }

    pub(super) fn activity(&self) -> &Arc<AgentActivity> {
        &self.activity
    }

    pub(super) fn work(&self) -> &WorkTracker {
        &self.work
    }

    fn waiters(&self) -> std::sync::MutexGuard<'_, HashMap<i64, oneshot::Sender<AgentAskReply>>> {
        self.waiters
            .lock()
            .expect("the waiters lock is not poisoned")
    }

    fn forks(&self) -> std::sync::MutexGuard<'_, HashMap<i64, Vec<Arc<ForegroundTurn>>>> {
        self.forks.lock().expect("the forks lock is not poisoned")
    }
}

/// A fork counted among its agent's running forks; dropping it takes the
/// fork off the count.
pub(super) struct RunningFork {
    forks: Arc<Mutex<HashMap<i64, Vec<Arc<ForegroundTurn>>>>>,
    agent_id: i64,
    turn: Arc<ForegroundTurn>,
}

impl Drop for RunningFork {
    fn drop(&mut self) {
        let mut forks = self.forks.lock().expect("the forks lock is not poisoned");
        if let Some(running) = forks.get_mut(&self.agent_id) {
            running.retain(|turn| !Arc::ptr_eq(turn, &self.turn));
            if running.is_empty() {
                forks.remove(&self.agent_id);
            }
        }
    }
}

/// `trace` one hop further, for a fork of agent `woken`'s conversation that
/// `by` wakes: every fork a chain hands work through is one hop, counted and
/// logged here, the one place a hop is taken, and never refused.
fn hop(trace: &DatabaseEventTrace, woken: i64, by: &str) -> DatabaseEventTrace {
    let hopped = trace.hop();
    log_info!(
        Module::Intelligence,
        "Agent {woken}: a fork wakes for {by}, {} hops from the user.",
        hopped.hops()
    );
    hopped
}

/// Why asking agent `recipient_id` was refused, for the model.
fn refusal(recipient_id: i64, error: CommandError) -> String {
    match error {
        CommandError::Validation(e) => e.message,
        CommandError::NotFound(_) => format!(
            "There is no agent {recipient_id}; the agents you can ask are under `# Other agents`."
        ),
        e => {
            log_error!(
                Module::Intelligence,
                "Could not ask agent {recipient_id}: {e:?}"
            );
            "Could not ask; try again.".to_string()
        }
    }
}
