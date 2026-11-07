use std::cell::Cell;

use cohesion_core::invalidation::Invalidation;

use super::Watched;
use crate::test_support::context;

#[tokio::test]
async fn a_watch_skips_changes_it_is_not_watching_for() {
    let database = context();
    let mut watch = database.watch();
    database.publish(Invalidation::Workspace(1));
    database.publish(Invalidation::ActorSession(7));
    database.publish(Invalidation::ActorSession(3));

    assert_eq!(
        watch
            .changed(|key| *key == Invalidation::ActorSession(3))
            .await,
        Watched::Changed
    );
    // The batches before the one it waited for are gone, not queued.
    database.publish(Invalidation::Workspace(1));
    database.publish(Invalidation::ActorSession(3));
    assert_eq!(
        watch
            .changed(|key| *key == Invalidation::ActorSession(3))
            .await,
        Watched::Changed
    );
}

#[tokio::test]
async fn missed_changes_count_as_a_change() {
    let database = context();
    let mut watch = database.watch();
    for _ in 0..300 {
        database.publish(Invalidation::Workspace(1));
    }
    assert_eq!(watch.changed(|_| false).await, Watched::Changed);
}

#[tokio::test]
async fn a_watch_closes_with_the_database() {
    let database = context();
    let mut watch = database.watch();
    drop(database);
    assert_eq!(watch.changed(|_| true).await, Watched::Closed);
}

#[tokio::test]
async fn until_reads_again_on_each_change_it_touches_until_settled() {
    let database = context();
    let mut watch = database.watch();
    let reads = Cell::new(0);
    // Publish what the second and third reads wait on before any read, so
    // the watch hears it whenever it asks.
    database.publish(Invalidation::Workspace(1));
    database.publish(Invalidation::ActorSession(1));
    database.publish(Invalidation::ActorSession(2));

    let settled = watch
        .until(
            || {
                reads.set(reads.get() + 1);
                let read = reads.get();
                async move { read }
            },
            |read| *read == 3,
            |read, key| *key == Invalidation::ActorSession(*read as i64),
        )
        .await;
    assert_eq!(settled, 3);
}
