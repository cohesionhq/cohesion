use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use cohesion_core::log::Module;
use cohesion_core::log_error;
use cohesion_db::{DatabaseContext, SetAgentActivityCommand, WorkGuard};
use tokio::sync::{Mutex, oneshot};

use super::agent_error::AgentError;
use crate::activity::{Activity, ShowActivity, ShowFuture};

/// One piece of an agent's work, as the job [`AgentActivity::scope`] runs
/// holds it: the handle to show what it is doing with, for as long as the
/// job holds it. It is neither `Clone` nor `Copy` and the job owns it, so
/// nothing can use it once the job is over.
pub(super) struct Work {
    pub(super) agent_id: i64,
    id: u64,
    activity: Arc<AgentActivity>,
}

impl std::fmt::Debug for Work {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Work")
            .field("agent_id", &self.agent_id)
            .field("id", &self.id)
            .finish()
    }
}

impl ShowActivity for Work {
    fn show(&self, activity: Activity) -> ShowFuture<'_> {
        Box::pin(self.activity.show(self.agent_id, self.id, activity.label()))
    }
}

/// What ends a piece of work. [`AgentActivity::scope`] holds the one `begin`
/// made and `end` consumes it, so a piece of work ends exactly once.
struct Ending {
    agent_id: i64,
    id: u64,
}

/// Tracks the work in progress for each agent (a foreground turn, a
/// fork) and keeps the agent's `activity` to
/// what the newest of it is doing, so one that ends hands back to the work
/// before it, and to `null` once none is left. Work runs only through
/// [`scope`](Self::scope): beginning and ending it write to the database,
/// so a guard cannot close it, and the scope is what guarantees it ends.
pub(super) struct AgentActivity {
    database: DatabaseContext,
    /// What each agent's running work is doing, oldest first; the newest is
    /// the one shown. Held across the writes, so an older label cannot land
    /// after a newer one.
    in_progress: Mutex<HashMap<i64, Vec<(u64, String)>>>,
    next_id: AtomicU64,
}
impl AgentActivity {
    pub(super) fn new(database: DatabaseContext) -> Self {
        Self {
            database,
            in_progress: Mutex::default(),
            next_id: AtomicU64::new(0),
        }
    }

    /// Runs `job` as one piece of `agent_id`'s work, on its own task: the
    /// agent has an activity from before `job` starts until after it ends,
    /// whatever it returns, and it runs to the end even when the request
    /// that started it goes away. A panic in `job` fails with
    /// [`AgentError::Panicked`]. `begun` is sent on once the agent's activity
    /// shows it working, before `job` starts; `counted` is held until the
    /// work has ended.
    pub(super) async fn scope<T, F>(
        self: &Arc<Self>,
        agent_id: i64,
        counted: WorkGuard,
        begun: Option<oneshot::Sender<()>>,
        job: impl FnOnce(Work) -> F + Send + 'static,
    ) -> Result<T, AgentError>
    where
        T: Send + 'static,
        F: Future<Output = Result<T, AgentError>> + Send + 'static,
    {
        let activity = self.clone();
        let task = tokio::spawn(async move {
            let (work, ending) = activity.clone().begin(agent_id).await;
            if let Some(begun) = begun {
                _ = begun.send(());
            }
            let result = tokio::spawn(job(work))
                .await
                .unwrap_or(Err(AgentError::Panicked));
            activity.end(ending).await;
            drop(counted);
            result
        });
        task.await.unwrap_or(Err(AgentError::Panicked))
    }

    /// Begins a piece of work, shown as thinking until it says otherwise.
    async fn begin(self: Arc<Self>, agent_id: i64) -> (Work, Ending) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.show(agent_id, id, Activity::Thinking(0).label()).await;
        (
            Work {
                agent_id,
                id,
                activity: self,
            },
            Ending { agent_id, id },
        )
    }

    /// Shows `label` as what work `id` is doing, replacing what it showed
    /// before.
    async fn show(&self, agent_id: i64, id: u64, label: String) {
        let mut in_progress = self.in_progress.lock().await;
        let work = in_progress.entry(agent_id).or_default();
        let before = work.last().map(|(_, label)| label.clone());
        match work.iter_mut().find(|(work_id, _)| *work_id == id) {
            Some((_, shown)) => *shown = label,
            None => work.push((id, label)),
        }
        let after = work.last().map(|(_, label)| label.clone());
        self.write(agent_id, before, after).await;
    }

    /// Ends the work, withdrawing what it showed; the last piece of work
    /// clears the activity.
    async fn end(&self, Ending { agent_id, id }: Ending) {
        let mut in_progress = self.in_progress.lock().await;
        let work = in_progress.get_mut(&agent_id).expect("the work has begun");
        let before = work.last().map(|(_, label)| label.clone());
        work.retain(|(work_id, _)| *work_id != id);
        let after = work.last().map(|(_, label)| label.clone());
        if work.is_empty() {
            in_progress.remove(&agent_id);
        }
        self.write(agent_id, before, after).await;
    }

    /// Writes the activity when it changed. Callers hold the `in_progress`
    /// lock.
    async fn write(&self, agent_id: i64, before: Option<String>, after: Option<String>) {
        if before == after {
            return;
        }
        let result = self
            .database
            .send(SetAgentActivityCommand {
                agent_id,
                activity: after,
            })
            .await;
        if let Err(e) = result {
            log_error!(
                Module::Intelligence,
                "Agent {agent_id}: could not update its activity: {e:?}"
            );
        }
    }
}

#[cfg(test)]
mod tests;
