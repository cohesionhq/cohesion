//! The handle the rest of the server uses to run commands: a channel to the
//! single worker thread that owns the connection.

use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::{broadcast, mpsc, oneshot};

use cohesion_core::invalidation::{Invalidation, InvalidationBatch, InvalidationContext};
use cohesion_core::log::Module;
use cohesion_core::log_error;
use cohesion_core::model::{RewindWriter, VersionAuthor};

use cohesion_core::protocol::DatabaseBackup;

use crate::backup::finish_backup;
use crate::command_error::{CommandError, DatabaseCommandResult};
use crate::commands::ShutdownCleanupCommand;
use crate::container::{Container, WriteContainer};
use crate::credential_manager::{CredentialLocks, CredentialManager};
use crate::database_command::{Access, CommandContext, DatabaseCommand, WriteArea};
use crate::database_error::DatabaseError;
use crate::database_event::{DatabaseEventListener, DatabaseEventTrace, EventContext, Writer};
use crate::guarded_database_context::GuardedDatabaseContext;
use crate::invalidation_watch::InvalidationWatch;
use crate::secrets::Secrets;

/// State owned by the worker thread and lent to each job.
struct Worker {
    container: Container,
    secrets: Secrets,
    invalidations: InvalidationContext,
    publisher: broadcast::Sender<Arc<InvalidationBatch>>,
    listener: Box<dyn DatabaseEventListener>,
    /// Set by [`DatabaseContext::close`]: the worker stops after this job
    /// and answers here once the connection is closed.
    closing: Option<oneshot::Sender<()>>,
    /// Who `rewind_writer` says is writing, so it is written only when that
    /// changes; `None` when it's unknown, at start and after a rollback.
    writer: Option<Writer>,
}

/// Runs one command, publishing what it changed before it replies.
type Job = Box<dyn FnOnce(&mut Worker) + Send>;

/// Runs commands on the database worker thread and fans out their invalidations.
/// Cheap to clone; clones share the worker.
#[derive(Debug, Clone)]
pub struct DatabaseContext {
    tx: mpsc::Sender<Job>,
    invalidations: broadcast::Sender<Arc<InvalidationBatch>>,
    credential_locks: Arc<CredentialLocks>,
}

impl DatabaseContext {
    /// Starts the worker thread that owns `container` for the rest of the process.
    pub fn new<L: DatabaseEventListener>(
        container: Container,
        secrets: Secrets,
        listener: L,
    ) -> Self {
        let (tx, mut rx) = mpsc::channel::<Job>(64);
        let (invalidations, _) = broadcast::channel(256);
        let mut worker = Worker {
            container,
            secrets,
            invalidations: InvalidationContext::default(),
            publisher: invalidations.clone(),
            listener: Box::new(listener),
            closing: None,
            writer: None,
        };
        thread::spawn(move || {
            while let Some(job) = rx.blocking_recv() {
                job(&mut worker);
                if let Some(closed) = worker.closing.take() {
                    drop(worker);
                    let _ = closed.send(());
                    return;
                }
            }
        });
        DatabaseContext {
            tx,
            invalidations,
            credential_locks: Arc::default(),
        }
    }

    /// Receives every batch of invalidations published after this call.
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<InvalidationBatch>> {
        self.invalidations.subscribe()
    }

    /// Watches the invalidations published after this call, to wait for a
    /// change to what the caller reads.
    pub fn watch(&self) -> InvalidationWatch {
        InvalidationWatch::new(self.subscribe())
    }

    /// Tells subscribers `key` changed outside any command, such as a client
    /// coming online.
    pub fn publish(&self, key: Invalidation) {
        let _ = self
            .invalidations
            .send(Arc::new(InvalidationBatch { keys: vec![key] }));
    }

    /// Runs `cmd` on the worker thread and waits for its result.
    pub async fn send<C: DatabaseCommand>(&self, cmd: C) -> DatabaseCommandResult<C::Response> {
        self.send_traced(cmd, DatabaseEventTrace::default()).await
    }

    /// [`DatabaseContext::send`] for a command raised while handling an
    /// event, so the events it raises continue `trace`.
    pub async fn send_traced<C: DatabaseCommand>(
        &self,
        cmd: C,
        trace: DatabaseEventTrace,
    ) -> DatabaseCommandResult<C::Response> {
        let (reply_tx, reply_rx) = oneshot::channel();
        if self.tx.send(self.job(cmd, reply_tx, trace)).await.is_err() {
            return Err(CommandError::Database(DatabaseError::Closed));
        }
        reply_rx
            .await
            .unwrap_or(Err(CommandError::Database(DatabaseError::Closed)))
    }

    /// [`DatabaseContext::send`] for synchronous callers (e.g. the script
    /// runtime). Panics if called from inside an async runtime.
    pub fn send_blocking<C: DatabaseCommand>(&self, cmd: C) -> DatabaseCommandResult<C::Response> {
        self.send_blocking_traced(cmd, DatabaseEventTrace::default())
    }

    /// [`DatabaseContext::send_blocking`] for a command raised while handling
    /// an event, so the events it raises continue `trace`.
    pub fn send_blocking_traced<C: DatabaseCommand>(
        &self,
        cmd: C,
        trace: DatabaseEventTrace,
    ) -> DatabaseCommandResult<C::Response> {
        let (reply_tx, reply_rx) = oneshot::channel();
        if self
            .tx
            .blocking_send(self.job(cmd, reply_tx, trace))
            .is_err()
        {
            return Err(CommandError::Database(DatabaseError::Closed));
        }
        reply_rx
            .blocking_recv()
            .unwrap_or(Err(CommandError::Database(DatabaseError::Closed)))
    }

    /// Runs the commands already sent, then [`ShutdownCleanupCommand`] as
    /// the last write, so nothing sent meanwhile can undo it, then closes the
    /// connection; every command sent after fails with
    /// [`DatabaseError::Closed`]. Returns once the connection is closed.
    pub async fn close(&self) {
        let (closed_tx, closed_rx) = oneshot::channel();
        let job: Job = Box::new(move |worker: &mut Worker| {
            if let Err(e) = worker.execute(ShutdownCleanupCommand, &DatabaseEventTrace::default()) {
                log_error!(Module::Database, "Shutdown cleanup failed: {e:?}");
            }
            worker.closing = Some(closed_tx);
        });
        if self.tx.send(job).await.is_ok() {
            let _ = closed_rx.await;
        }
    }

    /// Vacuums the database file if its free pages are worth a rewrite
    /// ([`worth_vacuuming`](crate::retention::worth_vacuuming)), returning the
    /// bytes of free pages it gave back to the file system, or `None` if they
    /// are not. `VACUUM` cannot run inside a command's
    /// savepoint, so this is a job of its own on the worker: it rewrites the
    /// whole file while every other command queues behind it (about a second
    /// for a hundred megabytes), and it is why this runs after the cleanup
    /// that freed the pages, not in it.
    pub async fn vacuum_if_reclaimable(&self) -> Result<Option<i64>, DatabaseError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let job: Job = Box::new(move |worker: &mut Worker| {
            let _ = reply_tx.send(worker.container.vacuum_if_reclaimable());
        });
        if self.tx.send(job).await.is_err() {
            return Err(DatabaseError::Closed);
        }
        reply_rx.await.unwrap_or(Err(DatabaseError::Closed))
    }

    /// Writes a backup of the database to a new private file beside it and
    /// says where. Like a vacuum this is a job of its own on the worker, which
    /// other commands wait behind while it copies; what it then leaves out of
    /// the copy (see [`BackupClass`](cohesion_core::stored_model::BackupClass))
    /// is done away from the worker, on the copy alone.
    pub async fn back_up(&self) -> Result<DatabaseBackup, DatabaseError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let job: Job = Box::new(move |worker: &mut Worker| {
            let _ = reply_tx.send(worker.container.copy_for_backup());
        });
        if self.tx.send(job).await.is_err() {
            return Err(DatabaseError::Closed);
        }
        let file = reply_rx.await.unwrap_or(Err(DatabaseError::Closed))?;
        tokio::task::spawn_blocking(move || finish_backup(&file))
            .await
            .map_err(|error| DatabaseError::Storage(std::io::Error::other(error)))?
    }

    /// The one way to connector accounts' credentials.
    pub fn credentials(&self) -> CredentialManager<'_> {
        CredentialManager::new(self, &self.credential_locks)
    }

    /// A guard over this handle that runs only read commands.
    pub fn read_only(&self) -> GuardedDatabaseContext {
        self.writing([])
    }

    /// A guard over this handle that runs read commands and write commands
    /// of every area.
    pub fn read_write(&self) -> GuardedDatabaseContext {
        self.writing(WriteArea::ALL.iter().copied())
    }

    /// A guard over this handle that runs read commands and write commands
    /// of `areas`.
    pub fn writing(&self, areas: impl IntoIterator<Item = WriteArea>) -> GuardedDatabaseContext {
        GuardedDatabaseContext::new(self.clone(), areas.into_iter().collect())
    }

    /// Wraps `cmd` so the worker runs it in a savepoint and replies on
    /// `reply`. Its invalidations and events go out first, so the hooks a
    /// command raises are queued before its caller hears back.
    fn job<C: DatabaseCommand>(
        &self,
        cmd: C,
        reply: oneshot::Sender<DatabaseCommandResult<C::Response>>,
        trace: DatabaseEventTrace,
    ) -> Job {
        Box::new(move |worker: &mut Worker| {
            let _ = reply.send(worker.execute(cmd, &trace));
        })
    }
}

impl Worker {
    /// Runs `cmd`, then publishes its invalidations and events if it
    /// committed or drops them if it failed.
    fn execute<C: DatabaseCommand>(
        &mut self,
        cmd: C,
        trace: &DatabaseEventTrace,
    ) -> DatabaseCommandResult<C::Response> {
        let mut events = EventContext::default();
        let result = self.run(cmd, &mut events, trace);
        // Logged here, once, because callers only tell their client that
        // something went wrong.
        if let Err(CommandError::Database(error)) = &result {
            log_error!(Module::Database, "A database error occurred: {error}");
        }
        match result.is_ok() {
            true => self.publish(events),
            false => self.invalidations.clear(),
        }
        result
    }

    /// Sends a committed command's invalidations to subscribers and its
    /// events to the listener.
    fn publish(&mut self, events: EventContext) {
        let keys = self.invalidations.take();
        if !keys.is_empty() {
            let _ = self.publisher.send(Arc::new(InvalidationBatch { keys }));
        }
        for event in events.into_events() {
            self.listener.send(event);
        }
    }

    /// Runs `cmd` inside `SAVEPOINT cmd`, releasing it on success and
    /// rolling it back on failure.
    fn run<C: DatabaseCommand>(
        &mut self,
        cmd: C,
        events: &mut EventContext,
        trace: &DatabaseEventTrace,
    ) -> DatabaseCommandResult<C::Response> {
        let connection = self.container.connection();
        connection
            .execute_batch("SAVEPOINT cmd")
            .map_err(DatabaseError::from)?;
        if C::Access::CHANGES_USER_DATA {
            note_writer(&self.container, &mut self.writer, trace.writer())?;
        }
        let mut ctx = CommandContext::new(
            C::Access::view(&self.container),
            &mut self.invalidations,
            &self.secrets,
            unix_now(),
            events,
            trace,
        );
        let result = ctx.handle(cmd);
        let finish = match result {
            Ok(_) => connection.execute_batch("RELEASE cmd"),
            Err(_) => {
                self.writer = None;
                connection.execute_batch("ROLLBACK TO cmd; RELEASE cmd")
            }
        };
        if let Err(error) = finish {
            self.writer = None;
            // A failed RELEASE leaves the savepoint open; undo it so the next job starts clean.
            let _ = connection.execute_batch("ROLLBACK TO cmd; RELEASE cmd");
            return Err(DatabaseError::from(error).into());
        }
        result
    }
}

/// Sets `rewind_writer` to `writer` inside the running command's savepoint,
/// unless `noted` says it already holds that.
fn note_writer(
    container: &Container,
    noted: &mut Option<Writer>,
    writer: Writer,
) -> Result<(), DatabaseError> {
    if noted.as_ref() == Some(&writer) {
        return Ok(());
    }
    *noted = None;
    let (author, agent_id, extension) = match &writer {
        Writer::User => (VersionAuthor::User, None, None),
        Writer::Agent(agent_id) => (VersionAuthor::Agent, Some(*agent_id), None),
        Writer::Extension(identifier) => {
            (VersionAuthor::Extension, None, Some(identifier.to_string()))
        }
        Writer::Script => (VersionAuthor::Script, None, None),
    };
    WriteContainer::new(container)
        .update::<RewindWriter>()
        .set(RewindWriter::AUTHOR, author)
        .set(RewindWriter::AGENT_ID, agent_id)
        .set(RewindWriter::EXTENSION, extension)
        .execute()?;
    *noted = Some(writer);
    Ok(())
}

/// Seconds since the Unix epoch.
pub(crate) fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
