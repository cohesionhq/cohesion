//! The sessions Cohesion hands actors, the agents clients offer on their
//! devices. The work happens on the client; the daemon keeps the session's
//! transcript and its status, as the client reports it, and shows its work
//! as one activity for the session's whole life, which [`ActivityTracker`]
//! runs: open for each stretch, from when it starts working until it is idle
//! or failed, and reopened when it works again. A daemon that
//! stops cancels the activity and leaves the session idle until its client
//! says otherwise; so does a client that goes away.
//!
//! A session's actor may launch subagents, sessions of their own below it
//! that its client reports into under the session. A subagent has no
//! activity and is told only responses to its requests; it settles idle when
//! its session is stopped or resolved, or its client goes away.

use std::collections::HashMap;
use std::sync::Arc;

use cohesion_core::log::Module;
use cohesion_core::model::{
    ActivityKind, ActivityOutcome, ActivityStanding, Actor, ActorSession, ActorSessionMessage,
    ActorSessionMessageContent, ActorSessionMove, ActorSessionStatus, ActorUserContent,
};
use cohesion_core::protocol::{
    ActorCall, ActorSessionReport, ClientCall, ClientImage, ImageUpload, ServerError,
    StartActorSessionRequest,
};
use cohesion_core::{log_debug, log_error};
use cohesion_db::{
    ActivityHandle, ActivityTracker, AddActorSessionMessageCommand, CreateActorSessionCommand,
    DatabaseContext, DeleteActorSessionCommand, DeleteActorSessionMessageCommand,
    EndActorSessionMessageCommand, GetActorCommand, GetActorIncludingDeletedCommand,
    GetActorSessionCommand, GetActorSubagentCommand, GetTopActorSessionCommand,
    LaunchActorSubagentCommand, SetActorSessionContextCommand, SetActorSessionModeCommand,
    SetActorSessionStatusCommand, SetActorSessionTitleCommand, SettleActorSubagentsCommand,
    SubagentsOf,
};
use tokio::sync::{Mutex, oneshot};
use tokio::task::JoinHandle;

use crate::capability_clients::CapabilityClients;

/// Starts actor sessions, carries what the user says to them, and keeps their
/// statuses, and the activities that show them, as their clients report.
/// A cheap clone.
#[derive(Clone)]
pub struct ActorSessions {
    inner: Arc<Inner>,
}

struct Inner {
    database: DatabaseContext,
    clients: CapabilityClients,
    activities: ActivityTracker,
    /// The stretch of work going on for each session working or needing
    /// attention, by session id. Held while one opens or ends, so a session has at
    /// most one.
    stretches: Mutex<HashMap<i64, Stretch>>,
}

/// One stretch of a session's work, its activity open until it ends.
struct Stretch {
    /// The client whose actor works on it, which going away ends it.
    client_id: i64,
    activity: ActivityHandle,
    /// Ends the activity with the outcome sent.
    end: oneshot::Sender<ActivityOutcome>,
    /// The activity's work, done once its end is recorded.
    done: JoinHandle<()>,
}

impl ActorSessions {
    pub fn new(
        database: DatabaseContext,
        clients: CapabilityClients,
        activities: ActivityTracker,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                database,
                clients,
                activities,
                stretches: Mutex::default(),
            }),
        }
    }

    /// Hands the actor `request` names a new session, working from the start:
    /// records it, then calls the actor's client. A client that can't be
    /// reached, or refuses, leaves no session behind.
    pub async fn start(
        &self,
        request: StartActorSessionRequest,
    ) -> Result<ActorSession, ServerError> {
        let StartActorSessionRequest {
            actor_id,
            prompt,
            title,
            mode,
            model,
            effort,
            attachments,
        } = request;
        let titled = title.is_some();
        let attachments = attachments.unwrap_or_default();
        let images = client_images(&attachments);
        let database = &self.inner.database;
        let actor = database.send(GetActorCommand { id: actor_id }).await?;
        self.reachable(&actor)?;
        let (session, message) = database
            .send(CreateActorSessionCommand {
                actor_id,
                prompt,
                title,
                mode,
                model,
                effort,
                attachments,
            })
            .await?;
        self.open(&session, actor.client_id, ActivityStanding::Running)
            .await?;
        let ActorSessionMessageContent::User(ActorUserContent::Text { text: prompt }) =
            message.content
        else {
            unreachable!("a session's first message is its prompt");
        };
        let call = ClientCall::StartActorSession {
            actor: actor.identifier.clone(),
            session_id: session.id,
            prompt,
            title: titled.then(|| session.title.clone()),
            images,
            mode: session.mode.clone(),
            model: session.model.clone(),
            effort: session.effort.clone(),
        };
        if let Err(error) = self.call(&actor, call).await {
            self.close(session.id, ActivityOutcome::Failed).await;
            database
                .send(DeleteActorSessionCommand { id: session.id })
                .await?;
            return Err(error);
        }
        Ok(database
            .send(GetActorSessionCommand { id: session.id })
            .await?)
    }

    /// Says `content` in session `session_id`, with the images in `attachments`,
    /// and hands it to the actor; the session is working from then on. A
    /// client that can't be reached, or refuses, leaves nothing said and the
    /// session as it stood.
    pub async fn send(
        &self,
        session_id: i64,
        content: ActorUserContent,
        attachments: Vec<ImageUpload>,
    ) -> Result<ActorSessionMessage, ServerError> {
        let images = client_images(&attachments);
        let database = &self.inner.database;
        let session = database
            .send(GetActorSessionCommand { id: session_id })
            .await?;
        let actor = database
            .send(GetActorIncludingDeletedCommand {
                id: session.actor_id,
            })
            .await?;
        self.reachable(&actor)?;
        // A subagent is told what it asked about under its session.
        let (addressed, subagent) = match &session.subagent {
            None => (session_id, None),
            Some(subagent) => {
                let top = database
                    .send(GetTopActorSessionCommand { id: session_id })
                    .await?;
                (top.id, Some(subagent.key.clone()))
            }
        };
        let message = database
            .send(AddActorSessionMessageCommand {
                session_id,
                content: ActorSessionMessageContent::User(content.clone()),
                attachments,
            })
            .await?;
        self.set_status(&session, actor.client_id, ActorSessionMove::Working)
            .await?;
        let call = ClientCall::SendActorSessionMessage {
            actor: actor.identifier.clone(),
            session_id: addressed,
            subagent,
            message_id: message.id,
            content,
            images,
        };
        if let Err(error) = self.call(&actor, call).await {
            database
                .send(DeleteActorSessionMessageCommand { id: message.id })
                .await?;
            match session.status {
                ActorSessionStatus::Working | ActorSessionStatus::NeedsAttention => {
                    let put_back = ActorSessionMove::PutBack(session.status);
                    self.set_status(&session, actor.client_id, put_back).await?;
                }
                ActorSessionStatus::Idle
                | ActorSessionStatus::Failed
                | ActorSessionStatus::Resolved => {
                    let put_back = ActorSessionMove::PutBack(session.status);
                    self.rest(session.id, put_back, ActivityOutcome::Cancelled)
                        .await?;
                }
            }
            return Err(error);
        }
        Ok(message)
    }

    /// Stops what session `session_id` is doing, telling its actor when its
    /// client can be reached; the session is idle however that goes, and its
    /// activity cancelled. A stop wakes no follower: whoever stopped it
    /// knows.
    pub async fn stop(&self, session_id: i64) -> Result<ActorSession, ServerError> {
        let session = self.started(session_id).await?;
        self.halt(&session).await?;
        let stopped = self
            .rest(
                session_id,
                ActorSessionMove::Stopped,
                ActivityOutcome::Cancelled,
            )
            .await?;
        self.settle_subagents(session_id).await?;
        Ok(stopped)
    }

    /// Session `session_id`, which must be one Cohesion started: a subagent
    /// goes with its session, so only that is stopped, resolved or switched.
    async fn started(&self, session_id: i64) -> Result<ActorSession, ServerError> {
        let session = self
            .inner
            .database
            .send(GetActorSessionCommand { id: session_id })
            .await?;
        if session.subagent.is_some() {
            return Err(ServerError::bad_request(
                "A subagent goes with its session.",
            ));
        }
        Ok(session)
    }

    /// Leaves session `session_id`'s subagents that are still going idle, as
    /// stopped: its actor stopped them with it.
    async fn settle_subagents(&self, session_id: i64) -> Result<(), ServerError> {
        self.inner
            .database
            .send(SettleActorSubagentsCommand {
                of: SubagentsOf::Session(session_id),
                moved: ActorSessionMove::Stopped,
            })
            .await?;
        Ok(())
    }

    /// Resolves session `session_id` for the user, who is done with it: one going
    /// on is stopped first, as [`stop`](Self::stop) does, and its activity
    /// cancelled. Like a stop, it wakes no follower.
    pub async fn resolve(&self, session_id: i64) -> Result<ActorSession, ServerError> {
        let session = self.started(session_id).await?;
        if matches!(
            session.status,
            ActorSessionStatus::Working | ActorSessionStatus::NeedsAttention
        ) {
            self.halt(&session).await?;
        }
        let resolved = self
            .rest(
                session_id,
                ActorSessionMove::Resolved,
                ActivityOutcome::Cancelled,
            )
            .await?;
        self.settle_subagents(session_id).await?;
        Ok(resolved)
    }

    /// Tells session `session`'s actor to stop it, when its client can be
    /// reached; one that can't be stops all the same.
    async fn halt(&self, session: &ActorSession) -> Result<(), ServerError> {
        let actor = self
            .inner
            .database
            .send(GetActorIncludingDeletedCommand {
                id: session.actor_id,
            })
            .await?;
        let call = ClientCall::StopActorSession {
            actor: actor.identifier.clone(),
            session_id: session.id,
        };
        if let Err(error) = self.call(&actor, call).await {
            log_debug!(
                Module::Actors,
                "Session {}: stopping without telling {}: {}",
                session.id,
                actor.title,
                error.message
            );
        }
        Ok(())
    }

    /// Switches session `session_id` to `mode`, telling its actor, which takes it
    /// from its next step on; the status stays as it is. A client that
    /// can't be reached, or refuses, leaves the session in the mode it had.
    pub async fn set_mode(
        &self,
        session_id: i64,
        mode: String,
    ) -> Result<ActorSession, ServerError> {
        let database = &self.inner.database;
        let session = self.started(session_id).await?;
        let actor = database
            .send(GetActorIncludingDeletedCommand {
                id: session.actor_id,
            })
            .await?;
        self.reachable(&actor)?;
        let switched = database
            .send(SetActorSessionModeCommand {
                session_id,
                mode: Some(mode.clone()),
            })
            .await?;
        let call = ClientCall::SetActorSessionMode {
            actor: actor.identifier.clone(),
            session_id,
            mode,
        };
        if let Err(error) = self.call(&actor, call).await {
            database
                .send(SetActorSessionModeCommand {
                    session_id,
                    mode: session.mode,
                })
                .await?;
            return Err(error);
        }
        Ok(switched)
    }

    /// Takes what client `client_id` reports about one of its actors'
    /// sessions, `session_id`, or about its subagent `subagent`, answering what
    /// it stored. Refused for a session of another client's.
    pub async fn report(
        &self,
        client_id: i64,
        session_id: i64,
        subagent: Option<String>,
        report: ActorSessionReport,
    ) -> Result<Reported, ServerError> {
        let database = &self.inner.database;
        let session = database
            .send(GetActorSessionCommand { id: session_id })
            .await?;
        let actor = database
            .send(GetActorIncludingDeletedCommand {
                id: session.actor_id,
            })
            .await?;
        if actor.client_id != client_id {
            return Err(ServerError::bad_request(
                "No actor of this client has this session.",
            ));
        }
        match (subagent, report) {
            (Some(key), ActorSessionReport::Launch(launch)) => {
                let subagent = database
                    .send(LaunchActorSubagentCommand {
                        session_id,
                        key,
                        launch,
                    })
                    .await?;
                Ok(Reported::Subagent(subagent))
            }
            (None, ActorSessionReport::Launch(_)) => Err(ServerError::validation(
                Some("subagent".to_string()),
                "Name the subagent launched.".to_string(),
            )),
            (Some(key), report) => {
                let subagent = database
                    .send(GetActorSubagentCommand { session_id, key })
                    .await?;
                self.take(subagent, client_id, report).await
            }
            (None, _) if session.subagent.is_some() => Err(ServerError::validation(
                Some("session_id".to_string()),
                "Report on a subagent under the session Cohesion started.".to_string(),
            )),
            (None, report) => self.take(session, client_id, report).await,
        }
    }

    /// Takes `report` about `session`, a session Cohesion started or a
    /// subagent, whose actor is client `client_id`'s.
    async fn take(
        &self,
        session: ActorSession,
        client_id: i64,
        report: ActorSessionReport,
    ) -> Result<Reported, ServerError> {
        let database = &self.inner.database;
        let session_id = session.id;
        let subagent = session.subagent.is_some();
        match report {
            ActorSessionReport::Launch(_) => unreachable!("a launch is taken by `report`"),
            ActorSessionReport::Message { content, images } => {
                let message = database
                    .send(AddActorSessionMessageCommand {
                        session_id,
                        content: ActorSessionMessageContent::Actor(content),
                        attachments: uploads(images),
                    })
                    .await?;
                return Ok(Reported::Message(message));
            }
            ActorSessionReport::Status { status } => {
                let moved = ActorSessionMove::reported(status);
                self.set_status(&session, client_id, moved).await?;
            }
            ActorSessionReport::Progress { progress } => {
                let moved = match session.status {
                    ActorSessionStatus::NeedsAttention => ActorSessionMove::NeedsYou,
                    _ => ActorSessionMove::Working,
                };
                // A subagent shows no activity, so its progress only marks
                // it working.
                if subagent {
                    database
                        .send(SetActorSessionStatusCommand { session_id, moved })
                        .await?;
                } else {
                    let activity = self.work(&session, client_id, moved).await?;
                    activity.progress(progress).await?;
                }
            }
            ActorSessionReport::End { message_id } => {
                database
                    .send(EndActorSessionMessageCommand {
                        session_id,
                        message_id,
                    })
                    .await?;
            }
            ActorSessionReport::Context { context } => {
                database
                    .send(SetActorSessionContextCommand {
                        session_id,
                        context,
                    })
                    .await?;
            }
            ActorSessionReport::Title { title } => {
                let session = database
                    .send(SetActorSessionTitleCommand { session_id, title })
                    .await?;
                self.retitle(&session).await?;
            }
        }
        Ok(Reported::Accepted)
    }

    /// Leaves client `client_id`'s working sessions, and those that need
    /// attention, idle, their stretches cancelled, and their subagents with
    /// them: it went away, so none of them is going on any longer.
    pub async fn client_left(&self, client_id: i64) {
        let mut stretches = self.inner.stretches.lock().await;
        let left: Vec<i64> = stretches
            .iter()
            .filter(|(_, stretch)| stretch.client_id == client_id)
            .map(|(session_id, _)| *session_id)
            .collect();
        for session_id in left {
            let idle = SetActorSessionStatusCommand {
                session_id,
                moved: ActorSessionMove::Interrupted,
            };
            if let Err(e) = self.inner.database.send(idle).await {
                log_error!(Module::Actors, "Session {session_id}: not left idle: {e:?}");
            }
            if let Some(stretch) = stretches.remove(&session_id) {
                end(stretch, ActivityOutcome::Cancelled).await;
            }
        }
        let interrupted = SettleActorSubagentsCommand {
            of: SubagentsOf::Client(client_id),
            moved: ActorSessionMove::Interrupted,
        };
        if let Err(e) = self.inner.database.send(interrupted).await {
            log_error!(
                Module::Actors,
                "Client {client_id}: subagents not left idle: {e:?}"
            );
        }
    }

    /// Moves session `session` as `moved` says, keeping its activity to match:
    /// open while it works or needs attention, ended succeeded as it goes
    /// idle and failed as it fails. A subagent, which has no activity, only
    /// moves.
    async fn set_status(
        &self,
        session: &ActorSession,
        client_id: i64,
        moved: ActorSessionMove,
    ) -> Result<(), ServerError> {
        if session.subagent.is_some() {
            self.inner
                .database
                .send(SetActorSessionStatusCommand {
                    session_id: session.id,
                    moved,
                })
                .await?;
            return Ok(());
        }
        match moved.status() {
            ActorSessionStatus::Working | ActorSessionStatus::NeedsAttention => {
                self.work(session, client_id, moved).await?;
            }
            ActorSessionStatus::Idle => {
                self.rest(session.id, moved, ActivityOutcome::Succeeded)
                    .await?;
            }
            ActorSessionStatus::Failed => {
                self.rest(session.id, moved, ActivityOutcome::Failed)
                    .await?;
            }
            ActorSessionStatus::Resolved => {
                self.rest(session.id, moved, ActivityOutcome::Cancelled)
                    .await?;
            }
        }
        Ok(())
    }

    /// Moves session `session` as `moved` says, to work or needing attention, and
    /// answers its stretch of work, opened now if it has none, paused while
    /// the session needs attention.
    async fn work(
        &self,
        session: &ActorSession,
        client_id: i64,
        moved: ActorSessionMove,
    ) -> Result<ActivityHandle, ServerError> {
        let standing = match moved.status() {
            ActorSessionStatus::NeedsAttention => ActivityStanding::Paused,
            _ => ActivityStanding::Running,
        };
        self.inner
            .database
            .send(SetActorSessionStatusCommand {
                session_id: session.id,
                moved,
            })
            .await?;
        self.open(session, client_id, standing).await
    }

    /// Moves session `session_id` as `moved` says, idle or failed, ending its
    /// stretch of work, if it has one, with `outcome`. Answers the session as
    /// it stands.
    async fn rest(
        &self,
        session_id: i64,
        moved: ActorSessionMove,
        outcome: ActivityOutcome,
    ) -> Result<ActorSession, ServerError> {
        let session = self
            .inner
            .database
            .send(SetActorSessionStatusCommand { session_id, moved })
            .await?;
        self.close(session_id, outcome).await;
        Ok(session)
    }

    /// Session `session`'s stretch of work, opened now if it has none, standing
    /// as `standing`.
    async fn open(
        &self,
        session: &ActorSession,
        client_id: i64,
        standing: ActivityStanding,
    ) -> Result<ActivityHandle, ServerError> {
        let mut stretches = self.inner.stretches.lock().await;
        if let Some(stretch) = stretches.get(&session.id)
            && !stretch.done.is_finished()
        {
            stretch.activity.standing(standing).await?;
            return Ok(stretch.activity.clone());
        }
        let (ready, activity) = oneshot::channel();
        let (end, ended) = oneshot::channel();
        let work = self.inner.activities.start(
            session.title.clone(),
            ActivityKind::ActorSession {
                actor_session_id: session.id,
            },
            move |activity| async move {
                let _ = ready.send(activity);
                // Dropped unsent only with the daemon, which cancels it.
                ended.await.unwrap_or(ActivityOutcome::Cancelled)
            },
        );
        let session_id = session.id;
        let done = tokio::spawn(async move {
            if let Err(e) = work.await {
                log_error!(Module::Actors, "Session {session_id}: no activity: {e:?}");
            }
        });
        let activity = activity.await.map_err(|_| ServerError::internal())?;
        activity.standing(standing).await?;
        stretches.insert(
            session.id,
            Stretch {
                client_id,
                activity: activity.clone(),
                end,
                done,
            },
        );
        Ok(activity)
    }

    /// Calls the stretch of work going on for `session`, if there is one, by
    /// the session's title.
    async fn retitle(&self, session: &ActorSession) -> Result<(), ServerError> {
        let activity = self
            .inner
            .stretches
            .lock()
            .await
            .get(&session.id)
            .map(|stretch| stretch.activity.clone());
        if let Some(activity) = activity {
            activity.title(session.title.clone()).await?;
        }
        Ok(())
    }

    /// Ends session `session_id`'s stretch of work with `outcome`, if it has one,
    /// once its end is recorded.
    async fn close(&self, session_id: i64, outcome: ActivityOutcome) {
        let stretch = self.inner.stretches.lock().await.remove(&session_id);
        if let Some(stretch) = stretch {
            end(stretch, outcome).await;
        }
    }

    /// Whether `actor` could be called now: its client is online and offers
    /// it.
    fn reachable(&self, actor: &Actor) -> Result<(), ServerError> {
        self.inner
            .clients
            .reachable(actor.client_id, &actor.identifier)
            .map_err(|error| error.naming(&actor.title))
    }

    /// Makes `call` to `actor`'s client, saying why not as the user reads it.
    async fn call(&self, actor: &Actor, call: ClientCall) -> Result<(), ServerError> {
        self.inner
            .clients
            .call(actor.client_id, actor_call(actor, call))
            .await
            .map_err(|error| error.naming(&actor.title))
    }
}

/// What a report from a client stored, which its reply carries.
#[derive(Debug)]
pub enum Reported {
    /// Nothing to answer but that it was taken.
    Accepted,
    /// The message a `message` report added.
    Message(ActorSessionMessage),
    /// The subagent a `launch` report added or updated.
    Subagent(ActorSession),
}

/// `attachments` as the client takes them.
fn client_images(attachments: &[ImageUpload]) -> Vec<ClientImage> {
    attachments
        .iter()
        .map(|image| ClientImage {
            media_type: image.media_type.clone(),
            data: image.data.clone(),
        })
        .collect()
}

/// The images an actor showed, as the transcript keeps them.
fn uploads(images: Vec<ClientImage>) -> Vec<ImageUpload> {
    images
        .into_iter()
        .map(|image| ImageUpload {
            media_type: image.media_type,
            data: image.data,
        })
        .collect()
}

/// `call`, addressed to `actor`.
fn actor_call(actor: &Actor, call: ClientCall) -> ActorCall {
    ActorCall {
        actor: actor.identifier.clone(),
        call,
    }
}

/// Ends `stretch` with `outcome` and waits for its end to be recorded.
async fn end(stretch: Stretch, outcome: ActivityOutcome) {
    let _ = stretch.end.send(outcome);
    let _ = stretch.done.await;
}
