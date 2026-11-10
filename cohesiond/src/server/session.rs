//! A long-lived connection: replies to requests, pushes invalidations once
//! the client subscribes, and, once it offers actors, carries the daemon's
//! calls into it.
//!
//! A session splits its connection and runs in three parts. The reader
//! (the session's own task) reads frames: it completes a `CallResult` at
//! once, handles a `Subscribe` or a `Hello`, and queues each request, so it
//! keeps reading while a command runs. The handler runs the queued requests
//! side by side and replies to each as it finishes, so replies come back
//! in any order, matched by id; a client that needs one request to follow
//! another waits for the first's reply. The writer drains one outbound queue
//! (replies, and the registry's calls) with the invalidations, numbering
//! every frame, so a send waiting on a full socket never stops the reading.
//! Over a transport with a keepalive the writer also pings, and a session
//! that hears nothing back for two pings' time ends as dropped, so a client
//! whose connection went half-open goes offline, and a client reconnects
//! rather than leaving turns to wait it out. While the session lasts, its
//! client is online; once the client is revoked, the session ends at once,
//! however it connected and whether or not it offers.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use cohesionkit::invalidation::Invalidation;
use cohesionkit::protocol::{
    AcceptedResponse, ActorSessionUpdate, ActorUsage, ActorUsageUpdate, CallResult,
    ClientCapabilities, ClientFrame, ClientHello, EvictionReason, Frame, Hello, Outcome,
    PROTOCOL_VERSION, Reply, ReplyData, Request, ServerError, ServerMessage, Subscribe, Vocabulary,
};
use cohesionkit::{
    Caller, Client, ClientRequest, ClientSessionHandle, CohesionContext, InvalidationBatch,
    OfferError, PresenceHandle, Reported, log_debug, log_info, log_warn, web_bundle_hash,
};
use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;
use tokio::sync::broadcast::Receiver;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::{Instant, Interval, MissedTickBehavior};

use super::request_handler::{bad_frame, encode, handle_request, module_for};
use super::transport::{
    BadFrame, Exchange, ExchangeError, ExchangeReader, ExchangeWriter, Incoming,
};

/// How many requests may run at once, and how many more may wait for the
/// handler before the session stops reading.
const QUEUED_REQUESTS: usize = 64;

/// How many actor session updates may wait to be taken, in order, before the
/// session stops reading.
const QUEUED_ACTOR_UPDATES: usize = 64;

/// The most plan limits an actor's usage may list.
const PLAN_USAGE_LIMIT: usize = 8;

/// The longest label of a plan limit, in bytes.
const PLAN_USAGE_LABEL_LIMIT: usize = 64;

/// The longest id of a usage pool, in bytes.
const USAGE_POOL_ID_LIMIT: usize = 256;

/// The longest name of a usage pool, in bytes.
const USAGE_POOL_NAME_LIMIT: usize = 128;

/// How long an evicted session waits for its last frames to be written.
const DRAIN: Duration = Duration::from_secs(5);

/// What the handler answers, in order.
enum Queued {
    Request(Box<Request>, String),
    /// A reply the reader already has: a subscribe's, an offer's, or a
    /// bad frame's.
    Answered(Box<Reply>),
}

pub struct Session<Conn: Exchange> {
    connection: Conn,
    context: CohesionContext,
    /// Who the session is with; named in its log lines.
    caller: Caller,
    /// The client its transport identified.
    client: Client,
}

/// Why a session ends.
enum Closed {
    /// The client went away; carries the error when it didn't leave cleanly.
    Disconnected(Option<String>),
    /// The daemon stopped publishing invalidations.
    Stopped,
    /// The handler stopped: a command panicked.
    HandlerFailed,
    /// The registry dropped this client's session, or the client was
    /// revoked.
    Evicted(EvictionReason),
}

impl<Conn: Exchange> Session<Conn> {
    /// A session with `client`, which the transport already identified
    /// (`caller` says how it connected); its log lines name the caller.
    pub fn new(
        connection: Conn,
        context: &CohesionContext,
        caller: Caller,
        client: Client,
    ) -> Self {
        Self {
            connection,
            context: context.clone(),
            caller,
            client,
        }
    }

    pub async fn attach(self) {
        let module = module_for(self.caller.transport);
        log_info!(module, "{}: session opened", self.caller);
        let (reader, writer) = self.connection.split();
        let (outbound, queue) = mpsc::unbounded_channel();
        let hello = ServerMessage::Hello(Hello {
            protocol: PROTOCOL_VERSION,
            server: format!("cohesiond/{}", cohesionkit::VERSION),
            client_name: self.client.client_name.clone(),
            web_bundle_hash: web_bundle_hash(),
            vocabulary: Box::new(Vocabulary::new()),
        });
        let _ = outbound.send(hello);
        let (subscription, subscribed) = oneshot::channel();
        let mut writer = tokio::spawn(write(
            writer,
            queue,
            Invalidations::Unsubscribed(subscribed),
            Conn::KEEPALIVE,
        ));
        let (requests, inbox) = mpsc::channel(QUEUED_REQUESTS);
        let mut handler = tokio::spawn(answer(
            inbox,
            outbound.clone(),
            self.context.clone(),
            self.client.clone(),
            self.caller.clone(),
        ));
        let (actor_updates, updates) = mpsc::channel(QUEUED_ACTOR_UPDATES);
        let reporting = tokio::spawn(report(
            updates,
            outbound.clone(),
            self.context.clone(),
            self.caller.clone(),
            self.client.id,
        ));
        let mut reading = Reading {
            reader,
            // The client is online until the session stops reading.
            present: self.context.presence.join(self.client.id),
            context: self.context,
            caller: self.caller,
            client_id: self.client.id,
            outbound,
            requests,
            actor_updates,
            subscription: Some(subscription),
            offering: None,
            keepalive: Conn::KEEPALIVE,
            heard: Instant::now(),
        };
        let closed = reading.run(&mut writer, &mut handler).await;
        let Reading {
            present,
            context,
            caller,
            client_id,
            outbound,
            requests,
            actor_updates,
            offering,
            ..
        } = reading;
        // Takes the client offline unless another session has it, unregisters
        // a client, failing the calls it hadn't answered,
        // and lets the handler stop once it has answered what it holds.
        drop(present);
        let served = offering.is_some();
        drop(offering);
        if served {
            // Its actors' usage went with it.
            context.database.publish(Invalidation::ActorUsage);
        }
        drop(requests);
        // The updates it sent are taken; then, unless another session
        // serves the client now, none of its actors' sessions goes on.
        drop(actor_updates);
        let _ = reporting.await;
        if served && !matches!(closed, Closed::Evicted(EvictionReason::Replaced)) {
            context.actor_sessions.client_left(client_id).await;
        }
        match closed {
            Closed::Evicted(reason) => {
                // The frames already queued, replies among them, then why.
                let _ = outbound.send(ServerMessage::Evicted { reason });
                drop(outbound);
                let _ = tokio::time::timeout(DRAIN, writer).await;
                let why = match reason {
                    EvictionReason::Replaced => "another session serves this client",
                    EvictionReason::Revoked => "this client was revoked",
                };
                log_info!(module, "{caller}: session closed; {why}");
            }
            Closed::Disconnected(Some(e)) => {
                writer.abort();
                log_warn!(module, "{caller}: session closed: {e}");
            }
            Closed::HandlerFailed => {
                writer.abort();
                log_warn!(
                    module,
                    "{caller}: session closed; a command failed unexpectedly"
                );
            }
            Closed::Disconnected(None) | Closed::Stopped => {
                writer.abort();
                log_info!(module, "{caller}: session closed");
            }
        }
    }
}

/// The session's reading side and what it hands frames to.
struct Reading<R: ExchangeReader> {
    reader: R,
    /// Keeps the client online, and says when it is revoked.
    present: PresenceHandle,
    context: CohesionContext,
    caller: Caller,
    client_id: i64,
    /// The writer's queue, which a `Hello` hands the registry for calls.
    outbound: mpsc::UnboundedSender<ServerMessage>,
    /// Requests waiting for the handler.
    requests: mpsc::Sender<Queued>,
    /// Actor session updates waiting to be taken, in the order they came.
    actor_updates: mpsc::Sender<ActorSessionUpdate>,
    /// Hands the writer the invalidations once the client subscribes; taken
    /// then.
    subscription: Option<oneshot::Sender<Receiver<Arc<InvalidationBatch>>>>,
    /// Set once the session offers: it serves a client's offers.
    offering: Option<ClientSessionHandle>,
    /// The transport's ping interval, if it pings.
    keepalive: Option<Duration>,
    /// When the peer was last heard from, a frame or a pong.
    heard: Instant,
}

impl<R: ExchangeReader> Reading<R> {
    /// When the session gives its client up for gone: two pings' time after
    /// it last heard from it, over a transport that pings; never over one
    /// that reports a hang-up itself.
    fn silent_after(&self) -> Option<Instant> {
        let every = self.keepalive?;
        Some(self.heard + every * 2)
    }

    /// Reads until the session ends, or its writer or handler stops.
    async fn run(
        &mut self,
        writer: &mut JoinHandle<Result<(), Closed>>,
        handler: &mut JoinHandle<()>,
    ) -> Closed {
        loop {
            let silent_after = self.silent_after();
            tokio::select! {
                biased;
                () = self.present.revoked() => return Closed::Evicted(EvictionReason::Revoked),
                reason = evicted(&mut self.offering) => return Closed::Evicted(reason),
                () = at(silent_after) => {
                    return Closed::Disconnected(Some(
                        "heard nothing from the client, not even a pong".to_string(),
                    ));
                }
                // Before the reader closes, the writer only stops on its own.
                written = &mut *writer => return match written {
                    Ok(Err(closed)) => closed,
                    _ => Closed::Stopped,
                },
                _ = &mut *handler => return Closed::HandlerFailed,
                incoming = self.reader.recv() => match incoming {
                    Ok(Some(incoming)) => {
                        self.heard = Instant::now();
                        if let Incoming::Frame(message) = incoming
                            && self.read(message.frame()).await.is_err()
                        {
                            return Closed::HandlerFailed;
                        }
                    }
                    Ok(None) => return Closed::Disconnected(None),
                    Err(ExchangeError::ConnectionError(e)) => return Closed::Disconnected(Some(e)),
                },
            }
        }
    }

    /// Handles one frame read off the connection. Fails when the handler
    /// has stopped, so nothing queued would be answered.
    async fn read(&mut self, frame: Result<(ClientFrame, String), BadFrame>) -> Result<(), ()> {
        let queued = match frame {
            Ok((ClientFrame::Request(request), kind)) => Queued::Request(request, kind),
            Ok((ClientFrame::Subscribe(subscribe), _)) => {
                Queued::Answered(Box::new(self.subscribe(subscribe)))
            }
            Ok((ClientFrame::Hello(hello), _)) => {
                Queued::Answered(Box::new(self.offer(hello).await))
            }
            Ok((ClientFrame::CallResult(result), _)) => {
                self.complete(result);
                return Ok(());
            }
            Ok((ClientFrame::ActorSessionUpdate(update), _)) => {
                if self.offering.is_none() {
                    let error = ServerError::bad_request("This session serves no client.");
                    Queued::Answered(Box::new(Reply {
                        id: Some(update.id),
                        outcome: Outcome::Error(error),
                    }))
                } else {
                    return self.actor_updates.send(update).await.map_err(|_| ());
                }
            }
            Ok((ClientFrame::ActorUsage(update), _)) => {
                Queued::Answered(Box::new(self.report_usage(update)))
            }
            Err(bad) => Queued::Answered(Box::new(bad_frame(bad, &self.caller))),
        };
        self.requests.send(queued).await.map_err(|_| ())
    }

    /// Keeps what one of the client's actors reports of its usage while the
    /// session serves the client, and answers `accepted` or why not.
    fn report_usage(&self, ActorUsageUpdate { id, actor, usage }: ActorUsageUpdate) -> Reply {
        let reported = check_usage(&usage).and_then(|()| match &self.offering {
            Some(offering) => offering.report_usage(actor, usage),
            None => Err("This session serves no client.".to_string()),
        });
        let outcome = match reported {
            Ok(()) => {
                self.context.database.publish(Invalidation::ActorUsage);
                Outcome::Ok(ReplyData::Accepted(AcceptedResponse {}))
            }
            Err(error) => {
                log_debug!(
                    module_for(self.caller.transport),
                    "{}: refused an actor's usage: {error}",
                    self.caller
                );
                Outcome::Error(ServerError::bad_request(error))
            }
        };
        Reply {
            id: Some(id),
            outcome,
        }
    }

    /// Starts the writer forwarding invalidations, raised from now on, and
    /// answers `accepted`.
    fn subscribe(&mut self, subscribe: Subscribe) -> Reply {
        if let Some(subscription) = self.subscription.take() {
            let _ = subscription.send(self.context.database.subscribe());
            log_debug!(
                module_for(self.caller.transport),
                "{}: subscribed to invalidations",
                self.caller
            );
        }
        Reply {
            id: Some(subscribe.id),
            outcome: Outcome::Ok(ReplyData::Accepted(AcceptedResponse {})),
        }
    }

    /// Makes this session the offering client's, and answers
    /// `accepted` or why not. It is daemon work, as a command is, so
    /// shutting down waits for it; once the daemon is shutting down, every
    /// offer is refused.
    async fn offer(&mut self, hello: ClientHello) -> Reply {
        let module = module_for(self.caller.transport);
        let id = hello.id;
        // A clone, so the work can borrow the session mutably.
        let work = self.context.work.clone();
        let label = format!("{}: offer", self.caller);
        let offered = work
            .run_new(label, async {
                match self.serve(hello).await {
                    Ok(()) => {
                        log_info!(module, "{}: serving client {}", self.caller, self.client_id);
                        Outcome::Ok(ReplyData::Accepted(AcceptedResponse {}))
                    }
                    Err(error) => {
                        log_warn!(module, "{}: offer refused: {}", self.caller, error.message);
                        Outcome::Error(error)
                    }
                }
            })
            .await;
        let outcome = offered.unwrap_or_else(|shutting_down| {
            log_debug!(module, "{}: offer refused; shutting down", self.caller);
            Outcome::Error(shutting_down.into())
        });
        Reply {
            id: Some(id),
            outcome,
        }
    }

    /// Registers the actors `hello` lists for the session's client, whose
    /// rows the registry brings in step with it. Saying hello again only
    /// replaces the offers, keeping the calls it hasn't answered.
    async fn serve(&mut self, hello: ClientHello) -> Result<(), ServerError> {
        check_actors(&hello.offers)?;
        let Some(offering) = &self.offering else {
            let handle = self
                .context
                .capability_clients
                .register(self.client_id, hello.offers, self.outbound.clone())
                .await?;
            self.offering = Some(handle);
            return Ok(());
        };
        match offering.offer(hello.offers).await {
            Ok(()) => Ok(()),
            Err(OfferError::Evicted) => Err(ServerError::bad_request(
                "This session no longer serves this client.",
            )),
            Err(OfferError::Sync(error)) => {
                // The registry gave the client up with it.
                self.offering = None;
                Err(error)
            }
        }
    }

    /// Answers a call this session was sent; a result for any other id is
    /// dropped.
    fn complete(&self, result: CallResult) {
        let completed = self
            .offering
            .as_ref()
            .is_some_and(|offering| offering.complete(result.call_id, result.outcome));
        if !completed {
            log_debug!(
                module_for(self.caller.transport),
                "{}: dropped the result of call {}, which isn't waiting",
                self.caller,
                result.call_id
            );
        }
    }
}

/// Takes the actor session updates the session's client sends, one at a time
/// in the order they came, as daemon work, and answers each: with the
/// message stored, `accepted`, or why not.
async fn report(
    mut updates: mpsc::Receiver<ActorSessionUpdate>,
    outbound: mpsc::UnboundedSender<ServerMessage>,
    context: CohesionContext,
    caller: Caller,
    client_id: i64,
) {
    let module = module_for(caller.transport);
    while let Some(ActorSessionUpdate {
        id,
        session_id: actor_session_id,
        subagent,
        report,
    }) = updates.recv().await
    {
        let label = format!("{caller}: actor session {actor_session_id} update");
        let reported = context
            .work
            .run_new(
                label,
                context
                    .actor_sessions
                    .report(client_id, actor_session_id, subagent, report),
            )
            .await;
        let outcome = match reported {
            Ok(Ok(Reported::Message(message))) => {
                Outcome::Ok(ReplyData::ActorSessionMessage(message))
            }
            Ok(Ok(Reported::Subagent(subagent))) => {
                Outcome::Ok(ReplyData::ActorSession(Box::new(subagent)))
            }
            Ok(Ok(Reported::Accepted)) => Outcome::Ok(ReplyData::Accepted(AcceptedResponse {})),
            Ok(Err(error)) => {
                log_warn!(
                    module,
                    "{caller}: refused an update of actor session {actor_session_id}: {}",
                    error.message
                );
                Outcome::Error(error)
            }
            Err(shutting_down) => Outcome::Error(shutting_down.into()),
        };
        let _ = outbound.send(ServerMessage::Reply(Reply {
            id: Some(id),
            outcome,
        }));
    }
}

/// Resolves at `deadline`; never without one.
async fn at(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

/// Resolves at `pings`' next tick; never without one.
async fn tick(pings: &mut Option<Interval>) {
    match pings {
        Some(pings) => {
            pings.tick().await;
        }
        None => std::future::pending().await,
    }
}

/// Resolves, with why, once the session's offering registration is dropped;
/// never, until it offers.
async fn evicted(offering: &mut Option<ClientSessionHandle>) -> EvictionReason {
    match offering {
        Some(offering) => offering.evicted().await,
        None => std::future::pending().await,
    }
}

/// What a client offers must be well formed: each actor a non-empty
/// identifier, unique among its actors, a product, a scope, and modes and
/// models each with an identifier of its own.
fn check_actors(ClientCapabilities { actors }: &ClientCapabilities) -> Result<(), ServerError> {
    let invalid =
        |message: String| Err(ServerError::validation(Some("actors".to_string()), message));
    let mut seen = HashSet::new();
    for actor in actors {
        let identifier = actor.identifier.as_str();
        if identifier.is_empty() {
            return invalid("An actor needs an identifier.".to_string());
        }
        if !seen.insert(identifier) {
            return invalid(format!("Two actors are identified {identifier:?}."));
        }
        if actor.product.trim().is_empty() || actor.scope.trim().is_empty() {
            return invalid(format!("Actor {identifier:?} needs a product and a scope."));
        }
        let mut modes = HashSet::new();
        let mut models = HashSet::new();
        let distinct = actor
            .modes
            .iter()
            .all(|mode| !mode.identifier.is_empty() && modes.insert(mode.identifier.as_str()))
            && actor.models.iter().all(|model| {
                let mut efforts = HashSet::new();
                !model.identifier.is_empty()
                    && models.insert(model.identifier.as_str())
                    && model.efforts.iter().all(|effort| {
                        !effort.identifier.is_empty() && efforts.insert(effort.identifier.as_str())
                    })
            });
        if !distinct {
            return invalid(format!(
                "Every mode, model and effort of actor {identifier:?} needs an identifier of its own."
            ));
        }
    }
    Ok(())
}

/// An actor's usage must make sense: at most [`PLAN_USAGE_LIMIT`] plan
/// limits, each labelled in at most [`PLAN_USAGE_LABEL_LIMIT`] bytes and
/// used from 0 to 1, and a pool, if any, with an id of at most
/// [`USAGE_POOL_ID_LIMIT`] bytes and a name of at most
/// [`USAGE_POOL_NAME_LIMIT`].
fn check_usage(usage: &ActorUsage) -> Result<(), String> {
    if let Some(pool) = &usage.pool {
        if pool.id.trim().is_empty() || pool.id.len() > USAGE_POOL_ID_LIMIT {
            return Err(format!(
                "A usage pool needs an id of at most {USAGE_POOL_ID_LIMIT} bytes."
            ));
        }
        if pool.name.trim().is_empty() || pool.name.len() > USAGE_POOL_NAME_LIMIT {
            return Err(format!(
                "A usage pool needs a name of at most {USAGE_POOL_NAME_LIMIT} bytes."
            ));
        }
    }
    if usage.plan.len() > PLAN_USAGE_LIMIT {
        return Err(format!(
            "An actor lists at most {PLAN_USAGE_LIMIT} plan limits."
        ));
    }
    for limit in &usage.plan {
        if limit.label.trim().is_empty() || limit.label.len() > PLAN_USAGE_LABEL_LIMIT {
            return Err(format!(
                "A plan limit needs a label of at most {PLAN_USAGE_LABEL_LIMIT} bytes."
            ));
        }
        if !(0.0..=1.0).contains(&limit.used) {
            return Err(format!(
                "Plan limit {:?} is used from 0 to 1, not {}.",
                limit.label, limit.used
            ));
        }
    }
    Ok(())
}

/// Runs queued requests side by side, up to [`QUEUED_REQUESTS`] at once,
/// and sends each reply as it is ready, so a long one holds up nothing
/// behind it. Once the reader goes away it finishes what it started.
async fn answer(
    mut inbox: mpsc::Receiver<Queued>,
    outbound: mpsc::UnboundedSender<ServerMessage>,
    context: CohesionContext,
    client: Client,
    caller: Caller,
) {
    let mut running = FuturesUnordered::new();
    let mut reading = true;
    loop {
        let reply = tokio::select! {
            queued = inbox.recv(), if reading && running.len() < QUEUED_REQUESTS => match queued {
                Some(Queued::Request(request, kind)) => {
                    let (context, client, caller) = (&context, &client, &caller);
                    running.push(async move {
                        let asking = ClientRequest { context, client };
                        handle_request(*request, &kind, &asking, caller).await
                    });
                    continue;
                }
                Some(Queued::Answered(reply)) => *reply,
                None => {
                    reading = false;
                    continue;
                }
            },
            Some(reply) = running.next() => reply,
            else => return,
        };
        // A reply to a client that has gone is dropped.
        let _ = outbound.send(ServerMessage::Reply(reply));
    }
}

/// The invalidations a session's writer forwards: none until the client
/// subscribes.
enum Invalidations {
    /// Waiting for the reader to hand over a subscription.
    Unsubscribed(oneshot::Receiver<Receiver<Arc<InvalidationBatch>>>),
    Subscribed(Receiver<Arc<InvalidationBatch>>),
    /// The reader went away without subscribing.
    Never,
}
impl Invalidations {
    /// The next frame to forward; never, while unsubscribed. Cancel safe.
    async fn next(&mut self) -> Result<ServerMessage, Closed> {
        loop {
            match self {
                Self::Unsubscribed(subscribed) => match subscribed.await {
                    Ok(receiver) => *self = Self::Subscribed(receiver),
                    Err(_) => *self = Self::Never,
                },
                Self::Never => std::future::pending().await,
                Self::Subscribed(receiver) => {
                    return match receiver.recv().await {
                        Ok(batch) => Ok(ServerMessage::Invalidate((*batch).clone())),
                        Err(RecvError::Lagged(_)) => Ok(ServerMessage::Reset),
                        Err(RecvError::Closed) => Err(Closed::Stopped),
                    };
                }
            }
        }
    }
}

/// Writes the outbound queue and the invalidations, in turn, and pings
/// every `keepalive`, until a write fails, the daemon stops publishing, or
/// it has written an `Evicted` frame.
async fn write<W: ExchangeWriter>(
    mut writer: W,
    mut queue: mpsc::UnboundedReceiver<ServerMessage>,
    mut invalidations: Invalidations,
    keepalive: Option<Duration>,
) -> Result<(), Closed> {
    let failed = |ExchangeError::ConnectionError(e)| Closed::Disconnected(Some(e));
    let mut pings = keepalive.map(|every| {
        let mut pings = tokio::time::interval_at(Instant::now() + every, every);
        // A ping held up behind a full socket isn't made up for.
        pings.set_missed_tick_behavior(MissedTickBehavior::Delay);
        pings
    });
    let mut seq = 0;
    loop {
        let message = tokio::select! {
            biased;
            Some(message) = queue.recv() => message,
            message = invalidations.next() => message?,
            () = tick(&mut pings) => {
                writer.ping().await.map_err(failed)?;
                continue;
            }
        };
        let last = matches!(message, ServerMessage::Evicted { .. });
        let frame = Frame { seq, message };
        seq += 1;
        writer.send(encode(&frame)).await.map_err(failed)?;
        if last {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests;
