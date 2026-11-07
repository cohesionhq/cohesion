//! The clients serving actors now: each one's session, what it offers,
//! how much of its limits each of its actors has used, and the calls the
//! daemon is waiting on it to answer. Sessions register here when they
//! offer; everything else reaches a client only through
//! [`CapabilityClients::call`]. Whether a client is online is
//! [`Presence`](crate::Presence)'s to say, for every session.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use cohesion_core::protocol::{
    ActorCall, ActorUsage, ClientCapabilities, ClientOutcome, ClientResult, EvictionReason,
    ServerError, ServerMessage,
};
use tokio::sync::{mpsc, oneshot};

use crate::capability_store::CapabilityStore;

/// Every session that offers, by client id. A cheap clone; clones share it.
#[derive(Clone)]
pub struct CapabilityClients {
    inner: Arc<Inner>,
}

struct Inner {
    /// Holds the rows that sync the offers.
    store: Arc<dyn CapabilityStore>,
    /// Held while a session's offers are written to the store, so the rows
    /// are the last session to hold a client's slot's, however the syncs
    /// of two racing sessions interleave.
    syncing: tokio::sync::Mutex<()>,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    sessions: HashMap<i64, Registration>,
    /// Tells a client's sessions apart, so a replaced one leaving doesn't
    /// unregister the one that replaced it.
    next_session: u64,
    next_call: u64,
    /// Orders usage reports, so the latest of a pool's stands for it.
    next_report: u64,
}

/// One session serving a client's offers.
struct Registration {
    session: u64,
    offers: ClientCapabilities,
    /// What each of its actors last reported of its usage, by identifier;
    /// only actors it offers, and none until one reports. It goes with the
    /// session.
    usage: HashMap<String, Report>,
    /// The session's outbound queue, which its `Call` frames join.
    sender: mpsc::UnboundedSender<ServerMessage>,
    /// Calls sent and not yet answered, by call id. Dropping one fails its
    /// caller as offline.
    pending: HashMap<u64, oneshot::Sender<ClientOutcome>>,
    /// Tells the session why it no longer serves the client, which ends it.
    evict: oneshot::Sender<EvictionReason>,
}
/// One actor's usage report, and where it falls among every report.
struct Report {
    usage: ActorUsage,
    order: u64,
}

/// The plan limits an actor's report draws on: the account it names, or,
/// naming none, the actor's own.
#[derive(PartialEq, Eq, Hash)]
enum PoolKey<'a> {
    Shared(&'a str),
    Own(i64, &'a str),
}

impl<'a> PoolKey<'a> {
    fn of(client_id: i64, actor: &'a str, report: &'a Report) -> Self {
        match &report.usage.pool {
            Some(pool) => Self::Shared(&pool.id),
            None => Self::Own(client_id, actor),
        }
    }
}

/// How much of one pool's plan limits is used: the actors drawing on it,
/// each by its client's id and identifier, and the latest report among
/// them, which stands for all.
#[derive(Debug, Clone, PartialEq)]
pub struct PooledUsage {
    pub actors: Vec<(i64, String)>,
    pub usage: ActorUsage,
}

impl State {
    /// Every actor's report of every client served now, with the client's
    /// id and the actor's identifier.
    fn reports(&self) -> impl Iterator<Item = (i64, &str, &Report)> {
        self.sessions.iter().flat_map(|(client_id, registration)| {
            registration
                .usage
                .iter()
                .map(|(actor, report)| (*client_id, actor.as_str(), report))
        })
    }
}

impl Registration {
    /// Ends the session, failing its pending calls as offline.
    fn evict(self, reason: EvictionReason) {
        let _ = self.evict.send(reason);
    }
}

impl CapabilityClients {
    pub fn new(store: Arc<dyn CapabilityStore>) -> Self {
        Self {
            inner: Arc::new(Inner {
                store,
                syncing: tokio::sync::Mutex::new(()),
                state: Mutex::default(),
            }),
        }
    }

    /// Makes the session whose outbound queue is `sender` the one serving
    /// `client_id`, with `offers`, and syncs them into the store. A session
    /// already serving that client is evicted as replaced: its pending calls
    /// fail as offline, and it ends. The session takes the client's slot
    /// first, so the store only ever holds what the session serving the
    /// client last offered; when syncing fails, it gives the slot up. A
    /// session offering again for its own client calls
    /// [`ClientSessionHandle::offer`] instead.
    #[must_use = "the session is unregistered when the handle is dropped"]
    pub async fn register(
        &self,
        client_id: i64,
        offers: ClientCapabilities,
        sender: mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<ClientSessionHandle, ServerError> {
        let (evict, evicted) = oneshot::channel();
        let (session, replaced) = {
            let mut state = self.state();
            let session = state.next_session;
            state.next_session += 1;
            let registration = Registration {
                session,
                offers: offers.clone(),
                usage: HashMap::new(),
                sender,
                pending: HashMap::new(),
                evict,
            };
            (session, state.sessions.insert(client_id, registration))
        };
        if let Some(replaced) = replaced {
            replaced.evict(EvictionReason::Replaced);
        }
        // Dropped on failure, which gives the slot up.
        let handle = ClientSessionHandle {
            clients: self.clone(),
            client_id,
            session,
            evicted,
        };
        self.sync(client_id, session, offers).await?;
        Ok(handle)
    }

    /// Writes `offers` to the store, if `session` still holds `client_id`'s
    /// slot once it is that session's turn to write.
    async fn sync(
        &self,
        client_id: i64,
        session: u64,
        offers: ClientCapabilities,
    ) -> Result<(), ServerError> {
        let _turn = self.inner.syncing.lock().await;
        if !self.holds(client_id, session) {
            return Err(ServerError::bad_request(
                "This session no longer serves this client.",
            ));
        }
        self.inner.store.sync(client_id, offers).await
    }

    fn holds(&self, client_id: i64, session: u64) -> bool {
        self.state()
            .sessions
            .get(&client_id)
            .is_some_and(|registration| registration.session == session)
    }

    /// Asks client `client_id`'s actor to carry out `call` and waits up to
    /// [`ActorCall::DEADLINE`] for its answer. Fails as offline at once when
    /// no session serves the client, and when the session serving it goes
    /// away before answering; fails as not offered when the client is
    /// connected but doesn't offer the actor.
    pub async fn call(&self, client_id: i64, call: ActorCall) -> Result<(), ClientCallError> {
        let (call_id, sender, answer) = {
            let mut state = self.state();
            let call_id = state.next_call;
            state.next_call += 1;
            let Some(registration) = state.sessions.get_mut(&client_id) else {
                return Err(ClientCallError::Offline);
            };
            if !registration.offers.hosts(&call.actor) {
                return Err(ClientCallError::NotOffered);
            }
            let (answer_tx, answer) = oneshot::channel();
            registration.pending.insert(call_id, answer_tx);
            (call_id, registration.sender.clone(), answer)
        };
        // However the wait ends, even with this future dropped, the call stops
        // waiting; once answered, there is nothing left to forget.
        let _waiting = Waiting {
            clients: self,
            client_id,
            call_id,
        };
        let exchange = async move {
            let frame = ServerMessage::Call {
                call_id,
                call: call.call,
            };
            sender.send(frame).map_err(|_| ClientCallError::Offline)?;
            answer.await.map_err(|_| ClientCallError::Offline)
        };
        match tokio::time::timeout(ActorCall::DEADLINE, exchange).await {
            Ok(Ok(ClientOutcome::Ok(ClientResult::Accepted))) => Ok(()),
            Ok(Ok(ClientOutcome::Error(message))) => Err(ClientCallError::Failed(message)),
            Ok(Err(offline)) => Err(offline),
            Err(_) => Err(ClientCallError::TimedOut),
        }
    }

    /// Whether client `client_id`'s actor `actor` could be called now: the
    /// client is online and offers it. Nothing is sent.
    pub fn reachable(&self, client_id: i64, actor: &str) -> Result<(), ClientCallError> {
        match self.state().sessions.get(&client_id) {
            None => Err(ClientCallError::Offline),
            Some(registration) if !registration.offers.hosts(actor) => {
                Err(ClientCallError::NotOffered)
            }
            Some(_) => Ok(()),
        }
    }

    /// The usage of each pool the actors of every client served now last
    /// reported, in no order: one an account they share, and one for each
    /// actor whose limits are its own.
    pub fn usage(&self) -> Vec<PooledUsage> {
        let state = self.state();
        let mut pools: HashMap<PoolKey, (u64, PooledUsage)> = HashMap::new();
        for (client_id, actor, report) in state.reports() {
            let (latest, pool) = pools
                .entry(PoolKey::of(client_id, actor, report))
                .or_insert_with(|| {
                    let usage = report.usage.clone();
                    let actors = Vec::new();
                    (report.order, PooledUsage { actors, usage })
                });
            pool.actors.push((client_id, actor.to_string()));
            if report.order > *latest {
                *latest = report.order;
                pool.usage = report.usage.clone();
            }
        }
        pools.into_values().map(|(_, pool)| pool).collect()
    }

    /// The usage of the pool client `client_id`'s actor `identifier` last
    /// reported drawing on, while the client is served: the latest report
    /// among the pool's actors. Empty when it reported none.
    pub fn actor_usage(&self, client_id: i64, identifier: &str) -> ActorUsage {
        let state = self.state();
        let Some(own) = state
            .sessions
            .get(&client_id)
            .and_then(|registration| registration.usage.get(identifier))
        else {
            return ActorUsage::default();
        };
        let pool = PoolKey::of(client_id, identifier, own);
        state
            .reports()
            .filter(|(client_id, actor, report)| PoolKey::of(*client_id, actor, report) == pool)
            .map(|(_, _, report)| report)
            .max_by_key(|report| report.order)
            .unwrap_or(own)
            .usage
            .clone()
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.inner.state.lock().unwrap()
    }
}

/// A call [`CapabilityClients::call`] is waiting on; dropping it stops waiting.
#[must_use = "the call stops being waited on when this is dropped"]
struct Waiting<'a> {
    clients: &'a CapabilityClients,
    client_id: i64,
    call_id: u64,
}
impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        if let Some(registration) = self.clients.state().sessions.get_mut(&self.client_id) {
            registration.pending.remove(&self.call_id);
        }
    }
}

/// Why a call to a client got no result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientCallError {
    /// No session serves the client, or the one serving it went away first.
    Offline,
    /// The client is connected but doesn't offer what was asked (any longer).
    NotOffered,
    /// The client answered that it couldn't, and why, as people read it.
    Failed(String),
    /// The client didn't answer before the deadline.
    TimedOut,
}

impl ClientCallError {
    /// What the caller is told, the client (or what it serves) named as
    /// `client`: "Mac mini is offline.", a validation error the user can
    /// act on, or "Mac mini didn’t answer in time.".
    pub fn naming(self, client: &str) -> ServerError {
        match self {
            Self::Offline => ServerError::validation(None, format!("{client} is offline.")),
            Self::NotOffered => {
                ServerError::validation(None, format!("{client} no longer offers this."))
            }
            Self::TimedOut => ServerError::upstream(format!("{client} didn’t answer in time.")),
            Self::Failed(message) => ServerError::upstream(message),
        }
    }
}

/// A session's registration as `client_id`'s client session. Dropping it
/// unregisters the session, failing its pending calls as offline, unless
/// another session has replaced it since.
#[must_use = "the session is unregistered when this is dropped"]
pub struct ClientSessionHandle {
    clients: CapabilityClients,
    client_id: i64,
    session: u64,
    evicted: oneshot::Receiver<EvictionReason>,
}

/// Why a session's offer was not taken.
pub enum OfferError {
    /// The session no longer serves the client: it was evicted.
    Evicted,
    /// The offers could not be synced. The session no longer serves the
    /// client either.
    Sync(ServerError),
}

impl ClientSessionHandle {
    /// Replaces what this session offers, keeping its pending calls, and
    /// syncs it into the store; when that fails, the session gives its
    /// client up.
    pub async fn offer(&self, offers: ClientCapabilities) -> Result<(), OfferError> {
        let holds = {
            let mut state = self.clients.state();
            match state.sessions.get_mut(&self.client_id) {
                Some(registration) if registration.session == self.session => {
                    // An actor no longer offered takes its usage with it.
                    registration.usage.retain(|actor, _| {
                        offers
                            .actors
                            .iter()
                            .any(|offered| offered.identifier == *actor)
                    });
                    registration.offers = offers.clone();
                    true
                }
                _ => false,
            }
        };
        if !holds {
            return Err(OfferError::Evicted);
        }
        match self
            .clients
            .sync(self.client_id, self.session, offers)
            .await
        {
            Ok(()) => Ok(()),
            Err(error) => {
                self.unregister();
                Err(OfferError::Sync(error))
            }
        }
    }

    /// Keeps `usage` as what this session's actor `actor` has used, in
    /// place of what it reported before, and as the latest of its pool's;
    /// one listing no limits clears it, pool and all. Fails,
    /// saying why, when the session no longer serves the client or doesn't
    /// offer the actor.
    pub fn report_usage(&self, actor: String, usage: ActorUsage) -> Result<(), String> {
        let mut state = self.clients.state();
        let order = state.next_report;
        state.next_report += 1;
        let Some(registration) = state
            .sessions
            .get_mut(&self.client_id)
            .filter(|registration| registration.session == self.session)
        else {
            return Err("This session no longer serves this client.".to_string());
        };
        if !registration
            .offers
            .actors
            .iter()
            .any(|offered| offered.identifier == actor)
        {
            return Err(format!("This client offers no actor {actor:?}."));
        }
        if usage.plan.is_empty() {
            registration.usage.remove(&actor);
        } else {
            registration.usage.insert(actor, Report { usage, order });
        }
        Ok(())
    }

    /// Gives up the client's slot, failing the session's pending calls as
    /// offline, unless another session has taken it.
    fn unregister(&self) {
        let removed = {
            let mut state = self.clients.state();
            match state.sessions.get(&self.client_id) {
                Some(registration) if registration.session == self.session => {
                    state.sessions.remove(&self.client_id)
                }
                _ => None,
            }
        };
        // Dropped outside the lock, failing its pending calls.
        drop(removed);
    }

    /// Answers this session's call `call_id` with `outcome`. False when the
    /// session has no such call pending: it never sent one, the caller gave
    /// up, or it was already answered.
    pub fn complete(&self, call_id: u64, outcome: ClientOutcome) -> bool {
        let answer = {
            let mut state = self.clients.state();
            match state.sessions.get_mut(&self.client_id) {
                Some(registration) if registration.session == self.session => {
                    registration.pending.remove(&call_id)
                }
                _ => None,
            }
        };
        answer.is_some_and(|answer| answer.send(outcome).is_ok())
    }

    /// Resolves once this session no longer serves the client, with why:
    /// another session replaced it. A revoked client's sessions end through
    /// [`Presence`](crate::Presence), dropping this.
    pub async fn evicted(&mut self) -> EvictionReason {
        match (&mut self.evicted).await {
            Ok(reason) => reason,
            // Only the registry going away drops it unsent.
            Err(_) => std::future::pending().await,
        }
    }
}

impl Drop for ClientSessionHandle {
    fn drop(&mut self) {
        self.unregister();
    }
}

#[cfg(test)]
mod tests;
