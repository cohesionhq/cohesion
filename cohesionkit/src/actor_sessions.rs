//! The sessions Cohesion hands actors, the agents clients offer on their
//! devices. The work happens on the client; the daemon keeps the session's
//! transcript, its status and its progress, as the client reports them, and
//! hooks follow them (`actor:started`, `actor:progress`, `actor:moved`,
//! `actor:resolved`). A daemon that stops leaves the session idle until its
//! client says otherwise; so does a client that goes away.
//!
//! A session's actor may launch subagents, sessions of their own below it
//! that its client reports into under the session. A subagent keeps no
//! progress, raises no hook and is told only responses to its requests; it
//! settles idle when its session is stopped or resolved, or its client goes
//! away.

use std::sync::Arc;

use cohesion_core::log::Module;
use cohesion_core::model::{
    Actor, ActorSession, ActorSessionMessage, ActorSessionMessageContent, ActorSessionMove,
    ActorSessionStatus, ActorUserContent,
};
use cohesion_core::protocol::{
    ActorCall, ActorSessionReport, ClientCall, ClientImage, ImageUpload, ServerError,
    StartActorSessionRequest,
};
use cohesion_core::{log_debug, log_error};
use cohesion_db::{
    AddActorSessionMessageCommand, AnnounceActorSessionCommand, CreateActorSessionCommand,
    DatabaseContext, DeleteActorSessionCommand, DeleteActorSessionMessageCommand,
    EndActorSessionMessageCommand, GetActorCommand, GetActorIncludingDeletedCommand,
    GetActorSessionCommand, GetActorSubagentCommand, GetTopActorSessionCommand,
    InterruptActorSessionsCommand, LaunchActorSubagentCommand, SetActorSessionContextCommand,
    SetActorSessionModeCommand, SetActorSessionProgressCommand, SetActorSessionStatusCommand,
    SetActorSessionTitleCommand, SettleActorSubagentsCommand, SubagentsOf,
};

use crate::capability_clients::CapabilityClients;

/// Starts actor sessions, carries what the user says to them, and keeps their
/// statuses and progress as their clients report. A cheap clone.
#[derive(Clone)]
pub struct ActorSessions {
    inner: Arc<Inner>,
}

struct Inner {
    database: DatabaseContext,
    clients: CapabilityClients,
}

impl ActorSessions {
    pub fn new(database: DatabaseContext, clients: CapabilityClients) -> Self {
        Self {
            inner: Arc::new(Inner { database, clients }),
        }
    }

    /// Hands the actor `request` names a new session, working from the start:
    /// records it, then calls the actor's client, and once the client takes
    /// it, raises `actor:started`. A client that can't be reached, or
    /// refuses, leaves no session behind and raises nothing.
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
        self.call_or_undo(&actor, call, async {
            database
                .send(DeleteActorSessionCommand { id: session.id })
                .await?;
            Ok(())
        })
        .await?;
        Ok(database
            .send(AnnounceActorSessionCommand {
                session_id: session.id,
            })
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
        let actor = self.actor_of(&session).await?;
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
        self.set_status(session_id, ActorSessionMove::Working)
            .await?;
        let call = ClientCall::SendActorSessionMessage {
            actor: actor.identifier.clone(),
            session_id: addressed,
            subagent,
            message_id: message.id,
            content,
            images,
        };
        self.call_or_undo(&actor, call, async {
            database
                .send(DeleteActorSessionMessageCommand { id: message.id })
                .await?;
            self.set_status(session_id, ActorSessionMove::PutBack(session.status))
                .await?;
            Ok(())
        })
        .await?;
        Ok(message)
    }

    /// Stops what session `session_id` is doing, telling its actor when its
    /// client can be reached; the session is idle however that goes. A stop
    /// wakes no follower: whoever stopped it knows.
    pub async fn stop(&self, session_id: i64) -> Result<ActorSession, ServerError> {
        let session = self.started(session_id).await?;
        self.halt(&session).await?;
        let stopped = self
            .set_status(session_id, ActorSessionMove::Stopped)
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
    /// on is stopped first, as [`stop`](Self::stop) does. Like a stop, it
    /// wakes no follower.
    pub async fn resolve(&self, session_id: i64) -> Result<ActorSession, ServerError> {
        let session = self.started(session_id).await?;
        if matches!(
            session.status,
            ActorSessionStatus::Working | ActorSessionStatus::NeedsAttention
        ) {
            self.halt(&session).await?;
        }
        let resolved = self
            .set_status(session_id, ActorSessionMove::Resolved)
            .await?;
        self.settle_subagents(session_id).await?;
        Ok(resolved)
    }

    /// Tells session `session`'s actor to stop it, when its client can be
    /// reached; one that can't be stops all the same.
    async fn halt(&self, session: &ActorSession) -> Result<(), ServerError> {
        let actor = self.actor_of(session).await?;
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
        let actor = self.actor_of(&session).await?;
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
        self.call_or_undo(&actor, call, async {
            database
                .send(SetActorSessionModeCommand {
                    session_id,
                    mode: session.mode,
                })
                .await?;
            Ok(())
        })
        .await?;
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
        let actor = self.actor_of(&session).await?;
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
                self.take(subagent, report).await
            }
            (None, _) if session.subagent.is_some() => Err(ServerError::validation(
                Some("session_id".to_string()),
                "Report on a subagent under the session Cohesion started.".to_string(),
            )),
            (None, report) => self.take(session, report).await,
        }
    }

    /// Takes `report` about `session`, a session Cohesion started or a
    /// subagent, whose actor is the reporting client's.
    async fn take(
        &self,
        session: ActorSession,
        report: ActorSessionReport,
    ) -> Result<Reported, ServerError> {
        let database = &self.inner.database;
        let session_id = session.id;
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
                self.set_status(session_id, moved).await?;
            }
            ActorSessionReport::Progress { progress } => {
                // Progress says the session is going on: one at rest works
                // again, and one that needs attention still does.
                let moved = match session.status {
                    ActorSessionStatus::NeedsAttention => ActorSessionMove::NeedsYou,
                    _ => ActorSessionMove::Working,
                };
                self.set_status(session_id, moved).await?;
                database
                    .send(SetActorSessionProgressCommand {
                        session_id,
                        progress,
                    })
                    .await?;
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
                database
                    .send(SetActorSessionTitleCommand { session_id, title })
                    .await?;
            }
        }
        Ok(Reported::Accepted)
    }

    /// Leaves client `client_id`'s working sessions, and those that need
    /// attention, idle, and their subagents with them: it went away, so none
    /// of them is going on any longer.
    pub async fn client_left(&self, client_id: i64) {
        let interrupted = InterruptActorSessionsCommand { client_id };
        if let Err(e) = self.inner.database.send(interrupted).await {
            log_error!(
                Module::Actors,
                "Client {client_id}: sessions not left idle: {e:?}"
            );
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

    /// Moves session `session_id` as `moved` says, answering it as it stands.
    async fn set_status(
        &self,
        session_id: i64,
        moved: ActorSessionMove,
    ) -> Result<ActorSession, ServerError> {
        Ok(self
            .inner
            .database
            .send(SetActorSessionStatusCommand { session_id, moved })
            .await?)
    }

    /// Whether `actor` could be called now: its client is online and offers
    /// it.
    fn reachable(&self, actor: &Actor) -> Result<(), ServerError> {
        self.inner
            .clients
            .reachable(actor.client_id, &actor.identifier)
            .map_err(|error| error.naming(&actor.title))
    }

    /// The actor `session` is a session of, which may since have been deleted.
    async fn actor_of(&self, session: &ActorSession) -> Result<Actor, ServerError> {
        Ok(self
            .inner
            .database
            .send(GetActorIncludingDeletedCommand {
                id: session.actor_id,
            })
            .await?)
    }

    /// Makes `call` to `actor`'s client for a write already recorded. When
    /// the client can't be reached, or refuses, runs `undo` to take the
    /// write back and answers why the call failed, so a call that didn't
    /// land leaves nothing behind. Every call that follows a write goes
    /// through here.
    async fn call_or_undo(
        &self,
        actor: &Actor,
        call: ClientCall,
        undo: impl Future<Output = Result<(), ServerError>>,
    ) -> Result<(), ServerError> {
        let Err(error) = self.call(actor, call).await else {
            return Ok(());
        };
        undo.await?;
        Err(error)
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

#[cfg(test)]
mod tests;

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
