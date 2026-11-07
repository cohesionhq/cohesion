use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use cohesion_core::model::ActorDefinition;
use cohesion_core::protocol::{
    ActorCall, ActorUsage, ClientCall, ClientCapabilities, ClientOutcome, ClientResult,
    EvictionReason, PlanUsage, ServerError, ServerMessage, UsagePool,
};
use tokio::sync::{Notify, Semaphore, mpsc};

use super::{CapabilityClients, ClientCallError, OfferError};
use crate::capability_store::{CapabilityStore, SyncFuture};

/// A store that records what it is asked, with no database behind it.
#[derive(Default)]
struct FakeStore {
    synced: Mutex<Vec<(i64, ClientCapabilities)>>,
    /// Makes every sync fail.
    fail: AtomicBool,
    /// When set, a sync waits for a permit, after telling `entered`.
    gate: Option<Semaphore>,
    entered: Notify,
}
/// The registry's store, sharing what it records with the test.
struct Shared(Arc<FakeStore>);
impl CapabilityStore for Shared {
    fn sync(&self, client_id: i64, offers: ClientCapabilities) -> SyncFuture {
        let store = self.0.clone();
        Box::pin(async move {
            store.entered.notify_one();
            if let Some(gate) = &store.gate {
                gate.acquire().await.unwrap().forget();
            }
            if store.fail.load(Ordering::SeqCst) {
                return Err(ServerError::internal());
            }
            store.synced.lock().unwrap().push((client_id, offers));
            Ok(())
        })
    }
}

fn store() -> Arc<FakeStore> {
    Arc::new(FakeStore::default())
}

fn clients_over(store: &Arc<FakeStore>) -> CapabilityClients {
    CapabilityClients::new(Arc::new(Shared(store.clone())))
}

fn clients() -> CapabilityClients {
    clients_over(&store())
}

/// Offers the actors `identifiers`.
fn actors(identifiers: &[&str]) -> ClientCapabilities {
    ClientCapabilities {
        actors: identifiers
            .iter()
            .map(|identifier| ActorDefinition {
                identifier: identifier.to_string(),
                product: "Claude Code".to_string(),
                scope: format!("/src/{identifier}"),
                modes: Vec::new(),
                models: Vec::new(),
            })
            .collect(),
    }
}

/// An actor's five-hour limit, `tenths` of it used.
fn plan(tenths: u8) -> ActorUsage {
    ActorUsage {
        plan: vec![PlanUsage {
            label: "5-hour limit".to_string(),
            used: f64::from(tenths) / 10.0,
            resets_at: None,
        }],
        pool: None,
    }
}

/// `plan(tenths)`, drawing on account `id`.
fn pooled(tenths: u8, id: &str) -> ActorUsage {
    ActorUsage {
        pool: Some(UsagePool {
            id: id.to_string(),
            name: format!("{id}@example.com"),
        }),
        ..plan(tenths)
    }
}

/// The pools `clients` hold, each its actors' identifiers and its usage, in
/// a fixed order.
fn pools(clients: &CapabilityClients) -> Vec<(Vec<String>, ActorUsage)> {
    let mut pools: Vec<_> = clients
        .usage()
        .into_iter()
        .map(|pool| {
            let mut actors: Vec<String> = pool
                .actors
                .into_iter()
                .map(|(client_id, actor)| format!("{client_id}/{actor}"))
                .collect();
            actors.sort();
            (actors, pool.usage)
        })
        .collect();
    pools.sort_by(|a, b| a.0.cmp(&b.0));
    pools
}

/// Stops session 1 of actor `actor`.
fn stop(actor: &str) -> ActorCall {
    ActorCall {
        actor: actor.to_string(),
        call: ClientCall::StopActorSession {
            actor: actor.to_string(),
            session_id: 1,
        },
    }
}

/// Whether a session serves client 1: a call to it fails as not offered
/// rather than offline.
async fn served(clients: &CapabilityClients) -> bool {
    clients.call(1, stop("unoffered")).await != Err(ClientCallError::Offline)
}

/// The id of the next `Call` frame on `frames`.
async fn next_call(frames: &mut mpsc::UnboundedReceiver<ServerMessage>) -> u64 {
    match frames.recv().await {
        Some(ServerMessage::Call { call_id, .. }) => call_id,
        _ => panic!("expected a call frame"),
    }
}

/// Serves `client_id` with `offers`; the frames sent to the session arrive
/// on the returned receiver.
async fn serve(
    clients: &CapabilityClients,
    client_id: i64,
    offers: ClientCapabilities,
) -> (
    super::ClientSessionHandle,
    mpsc::UnboundedReceiver<ServerMessage>,
) {
    let (sender, frames) = mpsc::unbounded_channel();
    let session = clients.register(client_id, offers, sender).await.unwrap();
    (session, frames)
}

/// Stops session 1 of client `client_id`'s actor `actor`, in the background.
fn stop_with(
    clients: &CapabilityClients,
    client_id: i64,
    actor: &str,
) -> tokio::task::JoinHandle<Result<(), ClientCallError>> {
    let clients = clients.clone();
    let actor = actor.to_string();
    tokio::spawn(async move { clients.call(client_id, stop(&actor)).await })
}

#[tokio::test]
async fn an_answered_call_returns_the_result() {
    let clients = clients();
    let (session, mut frames) = serve(&clients, 1, actors(&["cohesion"])).await;
    let call = stop_with(&clients, 1, "cohesion");
    let call_id = next_call(&mut frames).await;
    assert!(session.complete(call_id, ClientOutcome::Ok(ClientResult::Accepted)));
    assert_eq!(call.await.unwrap(), Ok(()));
    // An answered call can't be answered again.
    assert!(!session.complete(call_id, ClientOutcome::Error("again".to_string())));
}

#[tokio::test]
async fn a_second_session_replaces_the_first_and_fails_its_calls() {
    let store = store();
    let clients = clients_over(&store);
    let (mut first, mut first_frames) = serve(&clients, 1, actors(&["cohesion"])).await;
    let call = stop_with(&clients, 1, "cohesion");
    let call_id = next_call(&mut first_frames).await;

    let (second, _second_frames) = serve(&clients, 1, actors(&["web"])).await;
    assert_eq!(call.await.unwrap(), Err(ClientCallError::Offline));
    assert_eq!(first.evicted().await, EvictionReason::Replaced);
    assert!(!first.complete(call_id, ClientOutcome::Error("late".to_string())));
    assert!(matches!(
        first.offer(actors(&["cohesion"])).await,
        Err(OfferError::Evicted)
    ));
    // The store holds what the session serving the client offered.
    let last = store.synced.lock().unwrap().last().unwrap().1.clone();
    assert_eq!(last, actors(&["web"]));

    // The replaced session leaving doesn't unregister the one that replaced it.
    drop(first);
    assert!(served(&clients).await);
    drop(second);
    assert!(!served(&clients).await);
}

#[tokio::test]
async fn a_session_replaced_while_syncing_is_synced_before_its_replacement() {
    let store = Arc::new(FakeStore {
        gate: Some(Semaphore::new(0)),
        ..Default::default()
    });
    let clients = clients_over(&store);
    let (sender, _frames) = mpsc::unbounded_channel();
    let first = tokio::spawn({
        let clients = clients.clone();
        async move { clients.register(1, actors(&["cohesion"]), sender).await }
    });
    // The first session's sync is under way when the second takes the slot.
    store.entered.notified().await;
    let (sender, _frames) = mpsc::unbounded_channel();
    let second = tokio::spawn({
        let clients = clients.clone();
        async move { clients.register(1, actors(&["web"]), sender).await }
    });
    store.gate.as_ref().unwrap().add_permits(2);
    let _first = first.await.unwrap().unwrap();
    let _second = second.await.unwrap().unwrap();
    // Whichever session the store heard last is the one the registry serves.
    let last = store.synced.lock().unwrap().last().unwrap().1.clone();
    assert_eq!(last, actors(&["web"]));
    assert_eq!(
        clients.call(1, stop("cohesion")).await,
        Err(ClientCallError::NotOffered)
    );
}

#[tokio::test]
async fn a_call_is_refused_when_the_client_is_away_or_does_not_offer_it() {
    let clients = clients();
    assert_eq!(
        clients.call(1, stop("cohesion")).await,
        Err(ClientCallError::Offline)
    );
    let (_session, mut frames) = serve(&clients, 1, actors(&["cohesion"])).await;
    // An actor the client doesn't offer.
    assert_eq!(
        clients.call(1, stop("web")).await,
        Err(ClientCallError::NotOffered)
    );
    assert_eq!(
        clients.reachable(1, "web"),
        Err(ClientCallError::NotOffered)
    );
    assert_eq!(clients.reachable(1, "cohesion"), Ok(()));
    assert!(frames.try_recv().is_err());
}

#[tokio::test(start_paused = true)]
async fn a_call_nobody_answers_ends_at_its_deadline() {
    let clients = clients();
    let (session, mut frames) = serve(&clients, 1, actors(&["cohesion"])).await;
    let result = clients.call(1, stop("cohesion")).await;
    assert_eq!(result, Err(ClientCallError::TimedOut));
    // The frame went out, but the call no longer waits for its answer.
    let call_id = next_call(&mut frames).await;
    assert!(!session.complete(call_id, ClientOutcome::Error("late".to_string())));
}

#[tokio::test]
async fn offering_again_keeps_the_calls_waiting_and_syncs_the_new_offers() {
    let store = store();
    let clients = clients_over(&store);
    let (session, mut frames) = serve(&clients, 1, actors(&["cohesion"])).await;
    let call = stop_with(&clients, 1, "cohesion");
    let call_id = next_call(&mut frames).await;
    let offers = actors(&["cohesion", "web"]);
    session.offer(offers.clone()).await.ok().unwrap();
    assert_eq!(store.synced.lock().unwrap().last().unwrap().1, offers);
    assert!(session.complete(call_id, ClientOutcome::Ok(ClientResult::Accepted)));
    assert!(call.await.unwrap().is_ok());
    // The new actor answers calls too.
    let call = stop_with(&clients, 1, "web");
    next_call(&mut frames).await;
    call.abort();
}

#[tokio::test]
async fn a_session_whose_offers_cannot_be_synced_does_not_serve() {
    let store = store();
    let clients = clients_over(&store);
    store.fail.store(true, Ordering::SeqCst);
    let (sender, _frames) = mpsc::unbounded_channel();
    assert!(
        clients
            .register(1, actors(&["cohesion"]), sender)
            .await
            .is_err()
    );
    assert!(!served(&clients).await);

    // Nor does one that was serving and offers again.
    store.fail.store(false, Ordering::SeqCst);
    let (session, _frames) = serve(&clients, 1, actors(&["cohesion"])).await;
    store.fail.store(true, Ordering::SeqCst);
    assert!(matches!(
        session.offer(actors(&["web"])).await,
        Err(OfferError::Sync(_))
    ));
    assert!(!served(&clients).await);
}

#[tokio::test]
async fn a_call_whose_caller_gives_up_stops_waiting() {
    let clients = clients();
    let (session, mut frames) = serve(&clients, 1, actors(&["cohesion"])).await;
    let call = stop_with(&clients, 1, "cohesion");
    let call_id = next_call(&mut frames).await;
    call.abort();
    assert!(call.await.unwrap_err().is_cancelled());
    assert!(!session.complete(call_id, ClientOutcome::Error("late".to_string())));
}

#[tokio::test]
async fn an_actors_usage_lasts_while_its_client_offers_it() {
    let clients = clients();
    let (first, _frames) = serve(&clients, 1, actors(&["cohesion", "web"])).await;
    assert!(
        first
            .report_usage("elsewhere".to_string(), plan(1))
            .is_err()
    );
    first.report_usage("cohesion".to_string(), plan(1)).unwrap();
    first.report_usage("cohesion".to_string(), plan(2)).unwrap();
    first.report_usage("web".to_string(), plan(3)).unwrap();
    assert_eq!(
        pools(&clients),
        vec![
            (vec!["1/cohesion".to_string()], plan(2)),
            (vec!["1/web".to_string()], plan(3)),
        ]
    );

    // An empty report clears it, and an actor no longer offered takes it with it.
    first
        .report_usage("web".to_string(), ActorUsage::default())
        .unwrap();
    first.report_usage("web".to_string(), plan(3)).unwrap();
    assert!(first.offer(actors(&["web"])).await.is_ok());
    assert_eq!(pools(&clients), vec![(vec!["1/web".to_string()], plan(3))]);
    first
        .report_usage("web".to_string(), ActorUsage::default())
        .unwrap();
    assert!(clients.usage().is_empty());

    // A new session starts with none, and a replaced one can't report.
    first.report_usage("web".to_string(), plan(4)).unwrap();
    let (second, _second_frames) = serve(&clients, 1, actors(&["web"])).await;
    assert!(clients.usage().is_empty());
    assert!(first.report_usage("web".to_string(), plan(5)).is_err());
    second.report_usage("web".to_string(), plan(6)).unwrap();
    drop(second);
    assert!(clients.usage().is_empty());
}

#[tokio::test]
async fn actors_naming_one_pool_share_its_latest_usage() {
    let clients = clients();
    let (first, _frames) = serve(&clients, 1, actors(&["cohesion", "web"])).await;
    let (second, _second_frames) = serve(&clients, 2, actors(&["notes"])).await;
    first
        .report_usage("cohesion".to_string(), pooled(1, "a"))
        .unwrap();
    second
        .report_usage("notes".to_string(), pooled(2, "a"))
        .unwrap();
    first.report_usage("web".to_string(), plan(5)).unwrap();

    // Across clients, the latest report stands for the pool; an actor naming none is its own.
    let shared = vec!["1/cohesion".to_string(), "2/notes".to_string()];
    assert_eq!(
        pools(&clients),
        vec![
            (shared.clone(), pooled(2, "a")),
            (vec!["1/web".to_string()], plan(5)),
        ]
    );
    assert_eq!(clients.actor_usage(1, "cohesion"), pooled(2, "a"));
    assert_eq!(clients.actor_usage(1, "web"), plan(5));
    first
        .report_usage("cohesion".to_string(), pooled(3, "a"))
        .unwrap();
    assert_eq!(clients.actor_usage(2, "notes"), pooled(3, "a"));

    // An actor leaving takes its report with it, and the one before stands.
    drop(first);
    assert_eq!(
        pools(&clients),
        vec![(vec!["2/notes".to_string()], pooled(2, "a"))]
    );
    assert_eq!(clients.actor_usage(2, "notes"), pooled(2, "a"));
}
