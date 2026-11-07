use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use cohesionkit::db::{
    Container, DeleteClientCommand, IdentifyLocalClientCommand, RegisterClientCommand, Secrets,
};
use cohesionkit::model::ActorDefinition;
use cohesionkit::protocol::{
    ActorCall, CallResult, ClientCall, ClientCapabilities, ClientFrame, ClientHello, ClientOutcome,
    ClientResult, Command, CreateWorkspaceRequest, DeleteClientRequest, ErrorCode, EvictionReason,
    Frame, ListActorsRequest, ListClientsRequest, ListWorkspacesRequest, Outcome, Reply, ReplyData,
    Request, RevokeOtherClientsRequest, ServerMessage, Subscribe,
};
use cohesionkit::{
    Caller, CallerClient, Client, ClientCallError, ClientKeyKind, ClientRequest, CohesionContext,
    LocalIdentity, Transport,
};
use tokio::sync::{Notify, mpsc};
use tokio::task::JoinHandle;

use super::Session;
use crate::server::request_handler::handle_request;
use crate::server::transport::{
    Exchange, ExchangeReader, ExchangeResult, ExchangeWriter, Incoming, ReceivableMessage,
};

/// How many frames the test's end holds unread before the daemon's writes
/// wait, unless a test asks for fewer.
const ROOM: usize = 256;

/// A connection fed frames by the test, whose sent frames the test reads.
/// Dropping the test's end closes it, as a client hanging up does.
struct FakeExchange {
    incoming: mpsc::UnboundedReceiver<String>,
    outgoing: FakeWriter,
}
/// What the fake peer writes for a pong, and reads for a ping: not JSON,
/// so never a frame.
const PONG: &str = "pong";
const PING: &str = "ping";

impl Exchange for FakeExchange {
    // Only a test that pauses time lets it pass.
    const KEEPALIVE: Option<Duration> = Some(Duration::from_secs(30));
    type Reader = FakeReader;
    type Writer = FakeWriter;
    fn split(self) -> (FakeReader, FakeWriter) {
        (FakeReader(self.incoming), self.outgoing)
    }
}

struct FakeReader(mpsc::UnboundedReceiver<String>);
impl ExchangeReader for FakeReader {
    async fn recv(&mut self) -> ExchangeResult<Option<Incoming>> {
        Ok(self.0.recv().await.map(|text| match text.as_str() {
            PONG => Incoming::Pong,
            _ => Incoming::Frame(ReceivableMessage(text)),
        }))
    }
}

/// Writes into a channel with room for a few frames; once `watching`, says
/// when a write has to wait for the test to read.
struct FakeWriter {
    frames: mpsc::Sender<String>,
    watching: Arc<AtomicBool>,
    waiting: Arc<Notify>,
}
impl ExchangeWriter for FakeWriter {
    async fn send(&mut self, frame: String) -> ExchangeResult<()> {
        if self.frames.capacity() == 0 && self.watching.load(Ordering::SeqCst) {
            self.waiting.notify_one();
        }
        let _ = self.frames.send(frame).await;
        Ok(())
    }

    async fn ping(&mut self) -> ExchangeResult<()> {
        let _ = self.frames.send(PING.to_string()).await;
        Ok(())
    }
}

/// The client's end of a session.
struct Peer {
    to_daemon: mpsc::UnboundedSender<String>,
    from_daemon: mpsc::Receiver<String>,
    next_id: u64,
    watching: Arc<AtomicBool>,
    waiting: Arc<Notify>,
}
impl Peer {
    fn send(&self, frame: &ClientFrame) {
        let text = serde_json::to_string(frame).unwrap();
        self.to_daemon.send(text).unwrap();
    }

    fn id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// Sends `command` with the next id, returning that id.
    fn request(&mut self, command: impl Into<Command>) -> u64 {
        let id = self.id();
        let request = Request {
            id: Some(id),
            command: command.into(),
        };
        self.send(&ClientFrame::Request(Box::new(request)));
        id
    }

    /// The next frame, whatever it is.
    async fn frame(&mut self) -> ServerMessage {
        let text = self.from_daemon.recv().await.expect("the session ended");
        serde_json::from_str::<Frame>(&text).unwrap().message
    }

    /// The next frame that isn't an invalidation.
    async fn next(&mut self) -> ServerMessage {
        loop {
            match self.frame().await {
                ServerMessage::Invalidate(_) | ServerMessage::Reset => continue,
                message => return message,
            }
        }
    }

    async fn reply(&mut self) -> Reply {
        match self.next().await {
            ServerMessage::Reply(reply) => reply,
            _ => panic!("expected a reply"),
        }
    }

    /// Waits for the next reply, which must be to `id`.
    async fn outcome(&mut self, id: u64) -> Outcome {
        let reply = self.reply().await;
        assert_eq!(reply.id, Some(id));
        reply.outcome
    }

    /// Sends `command` and waits for its reply, by which time every frame
    /// sent before it has been read.
    async fn round_trip(&mut self, command: impl Into<Command>) -> Outcome {
        let id = self.request(command);
        self.outcome(id).await
    }

    /// Subscribes to invalidations and waits for the answer.
    async fn subscribe(&mut self) {
        let id = self.id();
        self.send(&ClientFrame::Subscribe(Subscribe { id }));
        assert!(matches!(self.outcome(id).await, Outcome::Ok(_)));
    }

    /// Offers actor `cohesion` and waits for the answer.
    async fn offer(&mut self) -> Outcome {
        let id = self.id();
        self.send(&offer(id, &["cohesion"]));
        self.outcome(id).await
    }

    /// The clients online, once the session has read every frame sent
    /// before: a request's reply comes after them.
    async fn online(&mut self) -> Vec<i64> {
        let Outcome::Ok(ReplyData::Clients(clients)) = self.round_trip(ListClientsRequest).await
        else {
            panic!("expected the clients");
        };
        clients
            .iter()
            .filter(|listed| listed.online)
            .map(|listed| listed.client.id)
            .collect()
    }

    /// The id of the next frame, which must be a call to actor `cohesion`.
    async fn call_id(&mut self) -> u64 {
        match self.next().await {
            ServerMessage::Call {
                call_id,
                call: ClientCall::StopActorSession { actor, .. },
            } => {
                assert_eq!(actor, "cohesion");
                call_id
            }
            _ => panic!("expected a call"),
        }
    }

    /// Reads the session's last frame, which must say it was evicted for
    /// `reason`, and that the session then ends.
    async fn evicted(&mut self, reason: EvictionReason) {
        let last = self.next().await;
        assert!(matches!(last, ServerMessage::Evicted { reason: why } if why == reason));
        assert!(self.from_daemon.recv().await.is_none());
    }
}

fn context() -> CohesionContext {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    CohesionContext::new(container, Secrets::new(), test_server())
}

/// A client registered as `name`, whose certificate's fingerprint is its
/// name too.
async fn client(context: &CohesionContext, name: &str) -> Client {
    context
        .database
        .send(RegisterClientCommand {
            setup_key: context.secrets.begin_pairing(),
            fingerprint: name.to_string(),
            client_name: name.to_string(),
            address: "203.0.113.1".to_string(),
        })
        .await
        .unwrap()
}

/// A local client named `name`, as the Unix socket's handshake makes it.
async fn local(context: &CohesionContext, name: &str) -> Client {
    context
        .database
        .send(IdentifyLocalClientCommand {
            identity: LocalIdentity {
                key: name.to_string(),
                name: name.to_string(),
            },
        })
        .await
        .unwrap()
}

async fn open(context: &CohesionContext, client: &Client) -> Peer {
    open_with_room(context, client, ROOM).await
}

/// Opens a session for `client`, which its transport identified (WebSocket
/// for a certificate, the Unix socket for a local key), whose end holds
/// `room` unread frames, and reads its `Hello`.
async fn open_with_room(context: &CohesionContext, client: &Client, room: usize) -> Peer {
    let (to_daemon, incoming) = mpsc::unbounded_channel();
    let (frames, from_daemon) = mpsc::channel(room);
    let watching = Arc::new(AtomicBool::new(false));
    let waiting = Arc::new(Notify::new());
    let caller = Caller {
        transport: match client.key_kind {
            ClientKeyKind::Certificate => Transport::WebSocket,
            ClientKeyKind::Local => Transport::UnixSocket,
        },
        client: Some(CallerClient {
            id: client.id,
            name: client.client_name.clone(),
        }),
        address: None,
    };
    let outgoing = FakeWriter {
        frames,
        watching: watching.clone(),
        waiting: waiting.clone(),
    };
    let exchange = FakeExchange { incoming, outgoing };
    tokio::spawn(Session::new(exchange, context, caller, client.clone()).attach());
    let mut peer = Peer {
        to_daemon,
        from_daemon,
        next_id: 0,
        watching,
        waiting,
    };
    // It names the client the transport identified.
    let name = &client.client_name;
    assert!(matches!(peer.next().await, ServerMessage::Hello(hello) if &hello.client_name == name));
    peer
}

/// A session that offers, for a newly paired client named "Mac mini".
async fn client_session(context: &CohesionContext) -> (Client, Peer) {
    let client = client(context, "Mac mini").await;
    let mut peer = open(context, &client).await;
    assert!(matches!(peer.offer().await, Outcome::Ok(_)));
    (client, peer)
}

/// Offers the actors `actors`.
fn offer(id: u64, actors: &[&str]) -> ClientFrame {
    let actors = actors
        .iter()
        .map(|identifier| ActorDefinition {
            identifier: identifier.to_string(),
            product: "Claude Code".to_string(),
            scope: format!("/src/{identifier}"),
            modes: Vec::new(),
            models: Vec::new(),
        })
        .collect();
    ClientFrame::Hello(ClientHello {
        id,
        offers: ClientCapabilities { actors },
    })
}

/// Accepts call `call_id`.
fn accept(call_id: u64) -> ClientFrame {
    ClientFrame::CallResult(CallResult {
        call_id,
        outcome: ClientOutcome::Ok(ClientResult::Accepted),
    })
}

/// Refuses call `call_id`, saying `why`.
fn refuse(call_id: u64, why: &str) -> ClientFrame {
    ClientFrame::CallResult(CallResult {
        call_id,
        outcome: ClientOutcome::Error(why.to_string()),
    })
}

/// Stops actor_session 1 of client `client_id`'s actor `cohesion`, as the daemon
/// does.
fn call(context: &CohesionContext, client_id: i64) -> JoinHandle<Result<(), ClientCallError>> {
    let clients = context.capability_clients.clone();
    tokio::spawn(async move {
        let call = ActorCall {
            actor: "cohesion".to_string(),
            call: ClientCall::StopActorSession {
                actor: "cohesion".to_string(),
                session_id: 1,
            },
        };
        clients.call(client_id, call).await
    })
}

#[tokio::test]
async fn only_a_subscribed_session_is_sent_invalidations() {
    let context = context();
    let studio = local(&context, "Studio").await;
    let mut peer = open(&context, &studio).await;
    let create = |title: &str| CreateWorkspaceRequest {
        title: title.to_string(),
        emoji: None,
    };
    let home = peer.request(create("Home"));
    assert!(matches!(peer.frame().await, ServerMessage::Reply(r) if r.id == Some(home)));

    let subscribe = peer.id();
    peer.send(&ClientFrame::Subscribe(Subscribe { id: subscribe }));
    assert!(matches!(peer.frame().await, ServerMessage::Reply(r) if r.id == Some(subscribe)));
    let work = peer.request(create("Work"));
    let frames = [peer.frame().await, peer.frame().await];
    assert!(
        frames
            .iter()
            .any(|f| matches!(f, ServerMessage::Reply(r) if r.id == Some(work)))
    );
    assert!(
        frames
            .iter()
            .any(|f| matches!(f, ServerMessage::Invalidate(_)))
    );
}

#[tokio::test]
async fn a_local_client_offers_as_itself() {
    let context = context();
    let studio = local(&context, "Studio").await;
    let mut peer = open(&context, &studio).await;
    assert!(matches!(peer.offer().await, Outcome::Ok(_)));
    assert_eq!(peer.online().await, [studio.id]);

    // Its actor is named after it.
    let Outcome::Ok(ReplyData::Actors(actors)) = peer.round_trip(ListActorsRequest).await else {
        panic!("expected the actors");
    };
    let [actor] = &actors[..] else {
        panic!("expected one actor");
    };
    assert_eq!(actor.client_id, studio.id);
    assert_eq!(actor.title, "Claude Code on Studio");
}

#[tokio::test]
async fn a_call_is_one_frame_and_its_result_answers_it() {
    let context = context();
    let (client, mut peer) = client_session(&context).await;

    let pending = call(&context, client.id);
    let call_id = peer.call_id().await;
    // A result for a call nobody made is dropped, and the session goes on.
    peer.send(&refuse(call_id + 1000, "stray"));
    peer.online().await;
    assert!(!pending.is_finished());
    peer.send(&accept(call_id));
    assert_eq!(pending.await.unwrap(), Ok(()));
}

#[tokio::test]
async fn closing_the_session_fails_its_calls_and_unregisters_it() {
    let context = context();
    let (client, mut peer) = client_session(&context).await;

    let pending = call(&context, client.id);
    peer.call_id().await;
    drop(peer);
    let failed = pending.await.unwrap();
    assert_eq!(failed.unwrap_err(), ClientCallError::Offline);

    // Its client is listed offline.
    let Outcome::Ok(ReplyData::Clients(clients)) = Command::from(ListClientsRequest)
        .dispatch(&ClientRequest {
            context: &context,
            client: &client,
        })
        .await
    else {
        panic!("expected the clients");
    };
    assert!(
        matches!(clients.as_slice(), [listed] if listed.client.id == client.id && !listed.online)
    );
}

#[tokio::test]
async fn any_session_puts_its_client_online_until_it_closes() {
    let context = context();
    let phone = client(&context, "Phone").await;
    let mut session = open(&context, &phone).await;
    // It offers nothing, and is online all the same.
    assert_eq!(session.online().await, [phone.id]);
    drop(session);
    let mut studio = open(&context, &local(&context, "Studio").await).await;
    while studio.online().await.contains(&phone.id) {
        tokio::task::yield_now().await;
    }
}

#[tokio::test]
async fn requests_are_answered_in_order_while_a_call_waits() {
    let context = context();
    let studio = local(&context, "Studio").await;
    let mut peer = open(&context, &studio).await;
    assert!(matches!(peer.offer().await, Outcome::Ok(_)));
    let [client] = peer.online().await[..] else {
        panic!("expected one client");
    };

    let pending = call(&context, client);
    let call_id = peer.call_id().await;
    let created = peer.request(CreateWorkspaceRequest {
        title: "Home".to_string(),
        emoji: None,
    });
    let listed = peer.request(ListWorkspacesRequest);
    let first = peer.reply().await;
    let second = peer.reply().await;
    assert_eq!((first.id, second.id), (Some(created), Some(listed)));
    assert!(matches!(
        second.outcome,
        Outcome::Ok(ReplyData::Workspaces(workspaces)) if workspaces.len() == 1
    ));

    peer.send(&accept(call_id));
    assert_eq!(pending.await.unwrap(), Ok(()));
}

#[tokio::test]
async fn an_offer_is_refused_once_the_daemon_is_shutting_down() {
    let context = context();
    let studio = local(&context, "Studio").await;
    let mut peer = open(&context, &studio).await;
    context.work.close();
    assert!(matches!(peer.offer().await, Outcome::Error(_)));
    let refused = call(&context, studio.id).await.unwrap();
    assert_eq!(refused.unwrap_err(), ClientCallError::Offline);
}

#[tokio::test]
async fn a_result_is_read_while_a_write_waits_on_a_full_socket() {
    let context = context();
    let client = client(&context, "Mac mini").await;
    let mut peer = open_with_room(&context, &client, 1).await;
    assert!(matches!(peer.offer().await, Outcome::Ok(_)));
    let pending = call(&context, client.id);
    let call_id = peer.call_id().await;

    // More calls than the socket holds, unread, so the writer has to wait.
    peer.watching.store(true, Ordering::SeqCst);
    let _unread: Vec<_> = (0..3).map(|_| call(&context, client.id)).collect();
    peer.waiting.notified().await;
    peer.send(&accept(call_id));
    assert_eq!(pending.await.unwrap(), Ok(()));
}

#[tokio::test]
async fn offering_again_keeps_the_calls_waiting() {
    let context = context();
    let (client, mut peer) = client_session(&context).await;
    let pending = call(&context, client.id);
    let call_id = peer.call_id().await;

    let id = peer.id();
    peer.send(&offer(id, &["cohesion", "web"]));
    assert!(matches!(peer.outcome(id).await, Outcome::Ok(_)));
    peer.send(&refuse(call_id, "busy"));
    assert_eq!(
        pending.await.unwrap(),
        Err(ClientCallError::Failed("busy".to_string()))
    );

    // Both actors are the client's.
    let Outcome::Ok(ReplyData::Actors(actors)) = peer.round_trip(ListActorsRequest).await else {
        panic!("expected the actors");
    };
    assert_eq!(actors.len(), 2);
}

#[tokio::test]
async fn a_second_session_replaces_the_first() {
    let context = context();
    let (client, mut first) = client_session(&context).await;
    let mut second = open(&context, &client).await;
    assert!(matches!(second.offer().await, Outcome::Ok(_)));
    first.evicted(EvictionReason::Replaced).await;
    assert_eq!(second.online().await, [client.id]);
}

#[tokio::test]
async fn deleting_the_client_ends_its_session() {
    let context = context();
    let (client, mut peer) = client_session(&context).await;
    let deleted = Command::from(DeleteClientRequest { id: client.id })
        .dispatch(&ClientRequest {
            context: &context,
            client: &client,
        })
        .await;
    assert!(matches!(deleted, Outcome::Ok(_)));
    peer.evicted(EvictionReason::Revoked).await;
    assert!(!context.presence.online(client.id));
}

#[tokio::test]
async fn revoking_a_client_ends_every_session_it_has_open() {
    let context = context();
    let desk = client(&context, "Desk").await;
    let phone = client(&context, "Phone").await;
    // Sessions that never offer, as the web UI's and the iPhone's: two
    // subscribed, one not.
    let mut subscribed = open(&context, &phone).await;
    subscribed.subscribe().await;
    let mut second = open(&context, &phone).await;
    second.subscribe().await;
    let mut quiet = open(&context, &phone).await;
    let mut asking = open(&context, &desk).await;

    let revoked = asking
        .round_trip(DeleteClientRequest { id: phone.id })
        .await;
    assert!(matches!(revoked, Outcome::Ok(_)));
    subscribed.evicted(EvictionReason::Revoked).await;
    second.evicted(EvictionReason::Revoked).await;
    quiet.evicted(EvictionReason::Revoked).await;
    assert_eq!(asking.online().await, [desk.id]);
}

#[tokio::test]
async fn revoking_the_other_clients_keeps_the_one_asking() {
    let context = context();
    let desk = client(&context, "Desk").await;
    let phone = client(&context, "Phone").await;
    let (mac, mut serving) = client_session(&context).await;
    let mut other = open(&context, &phone).await;
    let mut asking = open(&context, &desk).await;
    let mut asking_too = open(&context, &desk).await;

    let revoked = asking.round_trip(RevokeOtherClientsRequest).await;
    let Outcome::Ok(ReplyData::RevokedClients(revoked)) = revoked else {
        panic!("expected the clients revoked");
    };
    let mut ids: Vec<i64> = revoked.iter().map(|client| client.id).collect();
    ids.sort();
    assert_eq!(ids, [phone.id, mac.id]);
    other.evicted(EvictionReason::Revoked).await;
    serving.evicted(EvictionReason::Revoked).await;
    // Both of the asking client's sessions carry on.
    assert_eq!(asking.online().await, [desk.id]);
    assert_eq!(asking_too.online().await, [desk.id]);
}

#[tokio::test]
async fn a_revoked_client_is_refused_however_it_asks() {
    let context = context();
    let phone = client(&context, "Phone").await;
    let caller = Caller {
        transport: Transport::Http,
        client: Some(CallerClient {
            id: phone.id,
            name: phone.client_name.clone(),
        }),
        address: None,
    };
    let asking = ClientRequest {
        context: &context,
        client: &phone,
    };
    let request = || Request {
        id: Some(1),
        command: ListWorkspacesRequest.into(),
    };
    let reply = handle_request(request(), "listWorkspaces", &asking, &caller).await;
    assert!(matches!(reply.outcome, Outcome::Ok(_)));

    context
        .database
        .send(DeleteClientCommand { id: phone.id })
        .await
        .unwrap();
    context.presence.revoke(phone.id);
    // A request over a connection opened before, as an HTTP one-off on a
    // kept-alive one, is refused.
    let reply = handle_request(request(), "listWorkspaces", &asking, &caller).await;
    assert!(matches!(
        reply.outcome,
        Outcome::Error(error) if error.code == ErrorCode::Unauthorized
    ));
    // A session it opens over such a connection ends at once.
    let mut late = open(&context, &phone).await;
    late.evicted(EvictionReason::Revoked).await;
}

#[test]
fn an_offer_must_list_well_formed_actors() {
    let actor = |identifier: &str, product: &str| ActorDefinition {
        identifier: identifier.to_string(),
        product: product.to_string(),
        scope: "/src".to_string(),
        modes: Vec::new(),
        models: Vec::new(),
    };
    let offer = |actors: Vec<ActorDefinition>| ClientCapabilities { actors };
    assert!(super::check_actors(&offer(vec![actor("a", "A"), actor("b", "B")])).is_ok());
    for actors in [
        vec![actor("", "A")],
        vec![actor("a", "A"), actor("a", "Again")],
        vec![actor("a", " ")],
    ] {
        assert!(super::check_actors(&offer(actors)).is_err());
    }
}

#[tokio::test]
async fn a_session_that_hears_nothing_ends_as_dropped() {
    let context = context();
    let (client, mut peer) = client_session(&context).await;
    // Offers in real time; from here on the clock only moves when the
    // session has nothing else to do.
    tokio::time::pause();

    // Answering each ping keeps it, well past two pings' time.
    for _ in 0..4 {
        assert_eq!(peer.from_daemon.recv().await.as_deref(), Some(PING));
        peer.to_daemon.send(PONG.to_string()).unwrap();
    }
    assert!(context.presence.online(client.id));

    // Silent, it is dropped, not evicted: the stream just ends, and the
    // client is offline.
    while let Some(frame) = peer.from_daemon.recv().await {
        assert_eq!(frame, PING);
    }
    assert!(!context.presence.online(client.id));
    let offline = call(&context, client.id).await.unwrap();
    assert_eq!(offline.unwrap_err(), ClientCallError::Offline);
}

fn test_server() -> cohesionkit::ServerAddress {
    cohesionkit::ServerAddress {
        host: std::net::IpAddr::from(std::net::Ipv4Addr::LOCALHOST).into(),
        port: 3000,
        fingerprint: String::new(),
    }
}
