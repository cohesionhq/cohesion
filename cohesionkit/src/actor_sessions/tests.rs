use std::sync::Arc;

use cohesion_core::model::{
    ActorDefinition, ActorMode, ActorSession, ActorSessionStatus, ActorUserContent,
    ReportedActorSessionStatus,
};
use cohesion_core::protocol::{
    ActorSessionReport, ClientCapabilities, ClientOutcome, ClientResult, ServerMessage,
    StartActorSessionRequest,
};
use cohesion_db::{
    ActivityTracker, Container, DatabaseContext, GetActorSessionCommand,
    ListActorSessionMessagesCommand, ListActorSessionsCommand, ListActorsCommand,
    RegisterClientCommand, Secrets, WorkTracker,
};
use tokio::sync::mpsc;

use super::ActorSessions;
use crate::capability_clients::{CapabilityClients, ClientSessionHandle};

/// A daemon's actor sessions with one client, a Mac mini offering the actor
/// `cohesion` with the modes `ask` and `auto`. The test answers the calls
/// made to it.
struct Studio {
    database: DatabaseContext,
    sessions: ActorSessions,
    client_id: i64,
    actor_id: i64,
    /// The client's session, until it goes away.
    serving: Option<ClientSessionHandle>,
    calls: mpsc::UnboundedReceiver<ServerMessage>,
}

fn mode(identifier: &str) -> ActorMode {
    ActorMode {
        identifier: identifier.to_string(),
        title: identifier.to_string(),
        description: None,
    }
}

fn says(text: &str) -> ActorUserContent {
    ActorUserContent::Text {
        text: text.to_string(),
    }
}

impl Studio {
    async fn new() -> Self {
        let mut container = Container::in_memory().unwrap();
        container.migrate().unwrap();
        let secrets = Secrets::new();
        let database = DatabaseContext::new(container, secrets.clone(), ());
        let client = database
            .send(RegisterClientCommand {
                setup_key: secrets.begin_pairing(),
                fingerprint: "Mac mini".to_string(),
                client_name: "Mac mini".to_string(),
                address: "203.0.113.1".to_string(),
            })
            .await
            .unwrap();
        let clients = CapabilityClients::new(Arc::new(database.clone()));
        let offers = ClientCapabilities {
            actors: vec![ActorDefinition {
                identifier: "cohesion".to_string(),
                product: "Claude Code".to_string(),
                scope: "/src/cohesion".to_string(),
                modes: vec![mode("ask"), mode("auto")],
                models: Vec::new(),
            }],
        };
        let (sender, calls) = mpsc::unbounded_channel();
        let serving = clients.register(client.id, offers, sender).await.unwrap();
        let actor_id = database.send(ListActorsCommand).await.unwrap()[0].id;
        let sessions = ActorSessions::new(
            database.clone(),
            clients,
            ActivityTracker::new(database.clone(), WorkTracker::default()),
        );
        Self {
            database,
            sessions,
            client_id: client.id,
            actor_id,
            serving: Some(serving),
            calls,
        }
    }

    /// Answers the next call made to the client with `outcome`.
    async fn answer(&mut self, outcome: ClientOutcome) {
        let Some(ServerMessage::Call { call_id, .. }) = self.calls.recv().await else {
            panic!("expected a call");
        };
        let serving = self.serving.as_ref().expect("the client is away");
        assert!(serving.complete(call_id, outcome));
    }

    async fn accept(&mut self) {
        self.answer(ClientOutcome::Ok(ClientResult::Accepted)).await;
    }

    async fn refuse(&mut self) {
        self.answer(ClientOutcome::Error("busy".to_string())).await;
    }

    /// The client goes away, so nothing can be called on it.
    fn leave(&mut self) {
        self.serving = None;
    }

    fn request(&self) -> StartActorSessionRequest {
        StartActorSessionRequest {
            actor_id: self.actor_id,
            prompt: "Fix the build".to_string(),
            title: None,
            mode: Some("ask".to_string()),
            model: None,
            effort: None,
            attachments: None,
        }
    }

    /// A session started and accepted, which its actor then reported as
    /// `status`.
    async fn session(&mut self, status: ReportedActorSessionStatus) -> ActorSession {
        let sessions = self.sessions.clone();
        let request = self.request();
        let starting = tokio::spawn(async move { sessions.start(request).await });
        self.accept().await;
        let session = starting.await.unwrap().unwrap();
        self.sessions
            .report(
                self.client_id,
                session.id,
                None,
                ActorSessionReport::Status { status },
            )
            .await
            .unwrap();
        self.read(session.id).await
    }

    async fn read(&self, id: i64) -> ActorSession {
        self.database
            .send(GetActorSessionCommand { id })
            .await
            .unwrap()
    }

    async fn all(&self) -> Vec<ActorSession> {
        self.database
            .send(ListActorSessionsCommand { actor_id: None })
            .await
            .unwrap()
    }

    /// How many messages session `id` holds.
    async fn said(&self, id: i64) -> usize {
        self.database
            .send(ListActorSessionMessagesCommand { session_id: id })
            .await
            .unwrap()
            .len()
    }
}

#[tokio::test]
async fn a_start_the_client_refuses_leaves_no_session() {
    let mut studio = Studio::new().await;
    let sessions = studio.sessions.clone();
    let request = studio.request();
    let starting = tokio::spawn(async move { sessions.start(request).await });
    studio.refuse().await;
    let error = starting.await.unwrap().unwrap_err();
    assert_eq!(error.message, "busy");
    assert!(studio.all().await.is_empty());
}

#[tokio::test]
async fn a_start_on_a_client_that_is_away_records_nothing() {
    let mut studio = Studio::new().await;
    studio.leave();
    let error = studio.sessions.start(studio.request()).await.unwrap_err();
    assert_eq!(error.message, "Claude Code on Mac mini is offline.");
    assert!(studio.all().await.is_empty());
}

#[tokio::test]
async fn a_message_the_client_refuses_is_not_said_and_leaves_the_session_as_it_stood() {
    use ReportedActorSessionStatus::{Failed, Idle, NeedsAttention, Working};
    let stood = [
        (Idle, ActorSessionStatus::Idle),
        (Failed, ActorSessionStatus::Failed),
        (Working, ActorSessionStatus::Working),
        (NeedsAttention, ActorSessionStatus::NeedsAttention),
    ];
    for (reported, status) in stood {
        let mut studio = Studio::new().await;
        let session = studio.session(reported).await;
        assert_eq!(session.status, status);
        let said = studio.said(session.id).await;
        let sessions = studio.sessions.clone();
        let sending = tokio::spawn(async move {
            sessions
                .send(session.id, says("And the tests"), Vec::new())
                .await
        });
        studio.refuse().await;
        let error = sending.await.unwrap().unwrap_err();
        assert_eq!(error.message, "busy");
        assert_eq!(studio.said(session.id).await, said, "{status:?}");
        assert_eq!(studio.read(session.id).await.status, status);
    }
}

#[tokio::test]
async fn a_session_put_back_to_rest_works_again_when_a_message_lands() {
    let mut studio = Studio::new().await;
    let session = studio.session(ReportedActorSessionStatus::Idle).await;
    let sessions = studio.sessions.clone();
    let sending =
        tokio::spawn(async move { sessions.send(session.id, says("Once"), Vec::new()).await });
    studio.refuse().await;
    assert!(sending.await.unwrap().is_err());
    let sessions = studio.sessions.clone();
    let sending =
        tokio::spawn(async move { sessions.send(session.id, says("Twice"), Vec::new()).await });
    studio.accept().await;
    sending.await.unwrap().unwrap();
    assert_eq!(
        studio.read(session.id).await.status,
        ActorSessionStatus::Working
    );
}

#[tokio::test]
async fn a_mode_the_client_refuses_leaves_the_session_in_the_mode_it_had() {
    let mut studio = Studio::new().await;
    let session = studio.session(ReportedActorSessionStatus::Working).await;
    assert_eq!(session.mode.as_deref(), Some("ask"));
    let sessions = studio.sessions.clone();
    let switching =
        tokio::spawn(async move { sessions.set_mode(session.id, "auto".to_string()).await });
    studio.refuse().await;
    assert!(switching.await.unwrap().is_err());
    assert_eq!(studio.read(session.id).await.mode.as_deref(), Some("ask"));

    let sessions = studio.sessions.clone();
    let switching =
        tokio::spawn(async move { sessions.set_mode(session.id, "auto".to_string()).await });
    studio.accept().await;
    let switched = switching.await.unwrap().unwrap();
    assert_eq!(switched.mode.as_deref(), Some("auto"));
}

#[tokio::test]
async fn a_message_or_mode_for_a_client_that_is_away_changes_nothing() {
    let mut studio = Studio::new().await;
    let session = studio.session(ReportedActorSessionStatus::Idle).await;
    let said = studio.said(session.id).await;
    studio.leave();
    assert!(
        studio
            .sessions
            .send(session.id, says("Hello?"), Vec::new())
            .await
            .is_err()
    );
    assert!(
        studio
            .sessions
            .set_mode(session.id, "auto".to_string())
            .await
            .is_err()
    );
    let after = studio.read(session.id).await;
    assert_eq!(studio.said(session.id).await, said);
    assert_eq!(after.status, ActorSessionStatus::Idle);
    assert_eq!(after.mode.as_deref(), Some("ask"));
}
