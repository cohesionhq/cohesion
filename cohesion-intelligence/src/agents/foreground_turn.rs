use std::sync::{Arc, Mutex};

use cohesion_core::model::{Attachment, IntelligenceAgentMessage};
use cohesion_db::DatabaseEventTrace;
use cohesion_js::ScriptStop;
use tokio::sync::watch;

use super::agenda::{Agenda, HeardAgenda};
use super::agent_transcript;
use crate::transcript_item::{Prompt, PromptImage};
use crate::turn_input::{StoppedFuture, TurnInput};

/// The running turn of one agent's conversation, as the user reaches it
/// while it works: the messages they send join it, and their stop ends it.
/// One per agent, kept across hosts, so a request finds the turn whichever
/// host runs it; a fork of the conversation has one of its own. It carries
/// the trace the turn's writes and questions continue: a turn the user
/// started begins a fresh one, a fork or a turn another agent's answer
/// woke continues the question's. It also hears, for its thread, what
/// the agent's other threads changed of its agenda.
#[derive(Default)]
pub(super) struct ForegroundTurn {
    state: Mutex<State>,
    /// Whether the running turn is stopped, for the model call it waits on.
    stopped: watch::Sender<bool>,
    agenda: HeardAgenda,
}

/// A message of the user's waiting for the running turn to take it in.
pub(super) struct Steered {
    pub message: IntelligenceAgentMessage,
    pub attachments: Vec<Attachment>,
}

#[derive(Default)]
struct State {
    /// Whether a turn is running and taking messages in.
    open: bool,
    /// Stops the running turn's scripts.
    stop: ScriptStop,
    /// Sent to the running turn, not yet taken in.
    waiting: Vec<Steered>,
    /// Taken in by the running turn, for the host's working set.
    joined: Vec<IntelligenceAgentMessage>,
    /// What the running turn's writes and questions continue.
    trace: DatabaseEventTrace,
    /// Which turn this is, so a turn that ends late (the host's lock was
    /// released before it) can't close the one begun after it.
    generation: u64,
}

impl ForegroundTurn {
    /// A thread's turn, hearing changes to `agenda`, its agent's.
    pub(super) fn new(agenda: Arc<Agenda>) -> Self {
        Self {
            agenda: HeardAgenda::new(agenda),
            ..Self::default()
        }
    }

    /// What the thread heard of its agent's agenda.
    pub(super) fn agenda(&self) -> &HeardAgenda {
        &self.agenda
    }

    /// Starts a turn under `trace`, taking messages in until it closes, and
    /// returns which turn it is, for [`end`](Self::end). Messages a turn
    /// still open never took in wait for this one.
    pub(super) fn begin(&self, trace: DatabaseEventTrace) -> u64 {
        let mut state = self.state();
        let generation = state.generation + 1;
        *state = State {
            open: true,
            trace,
            generation,
            waiting: std::mem::take(&mut state.waiting),
            ..State::default()
        };
        self.stopped.send_replace(false);
        generation
    }

    /// The trace the running turn's writes and questions continue.
    pub(super) fn trace(&self) -> DatabaseEventTrace {
        self.state().trace.clone()
    }

    /// Hands `steered` to the running turn; returns it when no turn takes
    /// it in. The user's message is a fresh go-ahead: the turn's trace
    /// starts over, however deep an answer woke it.
    pub(super) fn steer(&self, steered: Steered) -> Option<Steered> {
        let mut state = self.state();
        if !state.open || state.stop.is_stopped() {
            return Some(steered);
        }
        state.waiting.push(steered);
        state.trace = DatabaseEventTrace::default();
        None
    }

    /// Stops the running turn, if one is; a script it runs is interrupted.
    pub(super) fn stop(&self) {
        let state = self.state();
        if state.open {
            state.stop.stop();
            self.stopped.send_replace(true);
        }
    }

    /// What stops the running turn's scripts.
    pub(super) fn script_stop(&self) -> ScriptStop {
        self.state().stop.clone()
    }

    /// Whether the running turn took in a message since last asked.
    pub(super) fn has_joined(&self) -> bool {
        !self.state().joined.is_empty()
    }

    /// The messages the running turn took in since last asked, oldest
    /// first.
    pub(super) fn take_joined(&self) -> Vec<IntelligenceAgentMessage> {
        std::mem::take(&mut self.state().joined)
    }

    /// Ends turn `generation`, returning the messages it never took in,
    /// which wait for turns of their own; nothing, when a later turn has
    /// begun since, which takes them in.
    pub(super) fn end(&self, generation: u64) -> Vec<Steered> {
        let mut state = self.state();
        if state.generation != generation {
            return Vec::new();
        }
        state.open = false;
        std::mem::take(&mut state.waiting)
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .expect("the foreground turn's lock is not poisoned")
    }
}

impl State {
    /// Moves the waiting messages into `joined`, as the prompts the turn
    /// reads them by; none once the turn is stopped, so they keep for turns
    /// of their own.
    fn take(&mut self) -> Vec<Prompt> {
        if self.stop.is_stopped() {
            return Vec::new();
        }
        let waiting = std::mem::take(&mut self.waiting);
        waiting
            .into_iter()
            .map(
                |Steered {
                     message,
                     attachments,
                 }| {
                    let prompt = Prompt {
                        text: agent_transcript::steered(&message),
                        images: attachments.into_iter().map(PromptImage::from).collect(),
                    };
                    self.joined.push(message);
                    prompt
                },
            )
            .collect()
    }
}

impl TurnInput for ForegroundTurn {
    /// The user's messages, after what another thread changed of the
    /// agenda, if it did.
    fn take(&self) -> Vec<Prompt> {
        let mut prompts = self.state().take();
        if !self.is_stopped()
            && let Some(news) = self.agenda.news()
        {
            prompts.insert(
                0,
                Prompt {
                    text: news,
                    images: Vec::new(),
                },
            );
        }
        prompts
    }

    fn close(&self) -> Vec<Prompt> {
        let mut state = self.state();
        let prompts = state.take();
        if prompts.is_empty() {
            state.open = false;
        }
        prompts
    }

    fn is_stopped(&self) -> bool {
        self.state().stop.is_stopped()
    }

    fn stopped(&self) -> StoppedFuture<'_> {
        let mut stopped = self.stopped.subscribe();
        Box::pin(async move {
            _ = stopped.wait_for(|stopped| *stopped).await;
        })
    }
}

#[cfg(test)]
mod tests;
