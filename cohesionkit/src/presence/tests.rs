use cohesion_core::invalidation::Invalidation;
use cohesion_db::{Container, DatabaseContext, Secrets};

use super::Presence;

fn database() -> DatabaseContext {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    DatabaseContext::new(container, Secrets::new(), ())
}

#[tokio::test]
async fn a_client_is_online_from_its_first_session_until_its_last_closes() {
    let database = database();
    let mut changes = database.subscribe();
    let presence = Presence::new(database);
    assert!(!presence.online(1));

    let first = presence.join(1);
    let second = presence.join(1);
    assert!(presence.online(1));
    drop(first);
    assert!(presence.online(1));
    drop(second);
    assert!(!presence.online(1));

    // Coming online and going offline are each told once.
    for _ in 0..2 {
        let batch = changes.try_recv().unwrap();
        assert_eq!(batch.keys, [Invalidation::Clients]);
    }
    assert!(changes.try_recv().is_err());
}

#[tokio::test]
async fn revoking_a_client_ends_its_sessions_and_any_that_open_after() {
    let presence = Presence::new(database());
    let mut first = presence.join(1);
    let mut second = presence.join(1);
    let mut other = presence.join(2);
    presence.revoke(1);

    first.revoked().await;
    second.revoked().await;
    // One that joins after, as a connection accepted before can, ends too.
    presence.join(1).revoked().await;
    assert!(presence.revoked(1));

    assert!(!presence.revoked(2));
    let still = tokio::time::timeout(std::time::Duration::from_millis(10), other.revoked());
    assert!(still.await.is_err());
}
