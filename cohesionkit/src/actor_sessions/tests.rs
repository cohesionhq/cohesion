use std::sync::{Arc, Mutex};

use cohesion_core::model::{
    ActorDefinition, ActorMode, ActorSession, ActorSessionStatus, ActorUserContent,
    ReportedActorSessionStatus,
};
use cohesion_core::protocol::{
    ActorSessionReport, ClientCapabilities, ClientOutcome, ClientResult, ServerMessage,
    StartActorSessionRequest,
};
use cohesion_db::{
    Container, DatabaseContext, DatabaseEvent, DatabaseEventKind, DatabaseEventListener,
    GetActorSessionCommand, ListActorSessionMessagesCommand, ListActorSessionsCommand,
    ListActorsCommand, RegisterClientCommand, Secrets,
};
use tokio::sync::mpsc;

use super::ActorSessions;
use crate::capability_clients::{CapabilityClients, ClientSessionHandle};

/// The events the studio's commands raise, for a test to take.
#[derive(Clone, Debug, Default)]
struct Events(Arc<Mutex<Vec<DatabaseEventKind>>>);
impl DatabaseEventListener for Events {
    fn send(&self, event: DatabaseEvent) {
        self.0.lock().unwrap().push(event.kind);
    }
}

/// What a hook hears of an actor session, as the event that raises it.
#[derive(Debug, PartialEq)]
enum Heard {
    Started,
    Progressed(Option<String>),
    Moved(ActorSessionStatus),
}

/// A daemon's actor sessions with one client, a Mac mini offering the actor
/// `cohesion` with the modes `ask` and `auto`. The test answers the calls
/// made to it.
struct Studio {
    database: DatabaseContext,
    events: Events,
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
        let events = Events::default();
        let database = DatabaseContext::new(container, secrets.clone(), events.clone());
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
        let sessions = ActorSessions::new(database.clone(), clients);
        Self {
            database,
            events,
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

    /// What hooks heard of actor sessions since last asked.
    fn heard(&self) -> Vec<Heard> {
        std::mem::take(&mut *self.events.0.lock().unwrap())
            .into_iter()
            .filter_map(|event| match event {
                DatabaseEventKind::ActorSessionStarted { .. } => Some(Heard::Started),
                DatabaseEventKind::ActorSessionProgressed { actor_session } => {
                    Some(Heard::Progressed(actor_session.progress))
                }
                DatabaseEventKind::ActorSessionMoved { actor_session, .. } => {
                    Some(Heard::Moved(actor_session.status))
                }
                _ => None,
            })
            .collect()
    }

    /// The client reports `progress` on session `id`.
    async fn progress(&self, id: i64, progress: &str) {
        self.sessions
            .report(
                self.client_id,
                id,
                None,
                ActorSessionReport::Progress {
                    progress: Some(progress.to_string()),
                },
            )
            .await
            .unwrap();
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
            .send(ListActorSessionMessagesCommand {
                session_id: id,
                ids: None,
            })
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
    assert_eq!(studio.heard(), []);
}

#[tokio::test]
async fn a_start_the_client_takes_is_heard_once_it_is_taken() {
    let mut studio = Studio::new().await;
    let sessions = studio.sessions.clone();
    let request = studio.request();
    let starting = tokio::spawn(async move { sessions.start(request).await });
    studio.accept().await;
    let session = starting.await.unwrap().unwrap();
    assert_eq!(session.status, ActorSessionStatus::Working);
    assert_eq!(studio.heard(), [Heard::Started]);
}

#[tokio::test]
async fn progress_is_kept_and_heard_while_a_session_goes_on_and_gone_once_it_rests() {
    let mut studio = Studio::new().await;
    let session = studio.session(ReportedActorSessionStatus::Idle).await;
    studio.heard();

    // Progress on a session at rest works it again.
    studio.progress(session.id, "Read 12 of 40 files").await;
    let working = studio.read(session.id).await;
    assert_eq!(working.status, ActorSessionStatus::Working);
    assert_eq!(working.progress.as_deref(), Some("Read 12 of 40 files"));
    assert_eq!(
        studio.heard(),
        [
            Heard::Moved(ActorSessionStatus::Working),
            Heard::Progressed(Some("Read 12 of 40 files".to_string())),
        ]
    );

    // Said again, it is heard once; a session that needs the user keeps it.
    studio.progress(session.id, "Read 12 of 40 files").await;
    studio
        .sessions
        .report(
            studio.client_id,
            session.id,
            None,
            ActorSessionReport::Status {
                status: ReportedActorSessionStatus::NeedsAttention,
            },
        )
        .await
        .unwrap();
    studio.progress(session.id, "Waiting on an answer").await;
    assert_eq!(
        studio.read(session.id).await.status,
        ActorSessionStatus::NeedsAttention
    );
    assert_eq!(
        studio.heard(),
        [
            Heard::Moved(ActorSessionStatus::NeedsAttention),
            Heard::Progressed(Some("Waiting on an answer".to_string())),
        ]
    );

    // A new title is heard as progress too.
    studio
        .sessions
        .report(
            studio.client_id,
            session.id,
            None,
            ActorSessionReport::Title {
                title: "Fix the flaky build".to_string(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        studio.heard(),
        [Heard::Progressed(Some("Waiting on an answer".to_string()))]
    );

    studio
        .sessions
        .report(
            studio.client_id,
            session.id,
            None,
            ActorSessionReport::Status {
                status: ReportedActorSessionStatus::Idle,
            },
        )
        .await
        .unwrap();
    let rested = studio.read(session.id).await;
    assert_eq!(rested.title, "Fix the flaky build");
    assert_eq!(rested.progress, None);
    assert_eq!(studio.heard(), [Heard::Moved(ActorSessionStatus::Idle)]);
}

#[tokio::test]
async fn a_client_going_away_leaves_its_sessions_going_on_idle() {
    let mut studio = Studio::new().await;
    let working = studio.session(ReportedActorSessionStatus::Working).await;
    let attention = studio
        .session(ReportedActorSessionStatus::NeedsAttention)
        .await;
    let idle = studio.session(ReportedActorSessionStatus::Idle).await;
    studio.progress(working.id, "Building").await;
    studio.heard();

    studio.sessions.client_left(studio.client_id).await;
    for session in [working.id, attention.id, idle.id] {
        let left = studio.read(session).await;
        assert_eq!(left.status, ActorSessionStatus::Idle, "{session}");
        assert_eq!(left.progress, None, "{session}");
    }
    // Cut short, not finished: neither is unread.
    for session in [working.id, attention.id] {
        assert!(!studio.read(session).await.unread, "{session}");
    }
    assert_eq!(
        studio.heard(),
        [
            Heard::Moved(ActorSessionStatus::Idle),
            Heard::Moved(ActorSessionStatus::Idle),
        ]
    );
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
