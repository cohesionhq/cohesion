//! Which clients are connected now: every session counts its client in
//! for as long as it lasts, whether or not it offers anything. This is the
//! one place presence is known; the clients list reports it, and a client's
//! actors are reachable while its client is online. It also
//! knows which clients were revoked while the daemon runs, so every session
//! of one ends at once and nothing it asks is carried out.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use cohesion_core::invalidation::Invalidation;
use cohesion_db::DatabaseContext;
use tokio::sync::watch;

/// The open sessions per client. A cheap clone; clones share it.
#[derive(Clone)]
pub struct Presence {
    inner: Arc<Inner>,
}

struct Inner {
    /// Told when a client comes online or goes offline.
    database: DatabaseContext,
    sessions: Mutex<HashMap<i64, usize>>,
    /// The clients revoked since the daemon started; each session watches
    /// it for its own. A client revoked earlier can't connect at all.
    revoked: watch::Sender<HashSet<i64>>,
}

impl Presence {
    pub fn new(database: DatabaseContext) -> Self {
        Self {
            inner: Arc::new(Inner {
                database,
                sessions: Mutex::default(),
                revoked: watch::Sender::default(),
            }),
        }
    }

    /// Counts a session with `client_id` in until the returned handle is
    /// dropped, which [`revoked`](PresenceHandle::revoked) says to end once
    /// the client is revoked. The client's first session brings it online.
    #[must_use = "the session counts out of presence when the handle is dropped"]
    pub fn join(&self, client_id: i64) -> PresenceHandle {
        let first = {
            let mut sessions = self.sessions();
            let open = sessions.entry(client_id).or_default();
            *open += 1;
            *open == 1
        };
        if first {
            self.inner.database.publish(Invalidation::Clients);
        }
        PresenceHandle {
            presence: self.clone(),
            client_id,
            revoked: self.inner.revoked.subscribe(),
        }
    }

    /// Ends every session of `client_id`, which was just revoked, and any
    /// that opens after; call it once the revocation is written.
    pub fn revoke(&self, client_id: i64) {
        self.inner.revoked.send_modify(|revoked| {
            revoked.insert(client_id);
        });
    }

    /// Whether `client_id` was revoked while the daemon runs, so nothing it
    /// asks may be carried out, though a session of it hasn't ended yet.
    pub fn revoked(&self, client_id: i64) -> bool {
        self.inner.revoked.borrow().contains(&client_id)
    }

    /// Whether `client_id` has a session open now.
    pub fn online(&self, client_id: i64) -> bool {
        self.sessions().contains_key(&client_id)
    }

    /// Every client with a session open now.
    pub fn online_clients(&self) -> Vec<i64> {
        self.sessions().keys().copied().collect()
    }

    fn sessions(&self) -> std::sync::MutexGuard<'_, HashMap<i64, usize>> {
        self.inner.sessions.lock().unwrap()
    }
}

/// A session's place in [`Presence`]; dropping it counts the session out,
/// and the client's last session takes it offline.
#[must_use = "the session counts out of presence when this is dropped"]
pub struct PresenceHandle {
    presence: Presence,
    client_id: i64,
    revoked: watch::Receiver<HashSet<i64>>,
}

impl PresenceHandle {
    /// Resolves once the session's client is revoked, at once if it
    /// already was. Cancel safe.
    pub async fn revoked(&mut self) {
        let client_id = self.client_id;
        // The sender lives in the presence this handle holds, so it never
        // closes.
        let _ = self
            .revoked
            .wait_for(|revoked| revoked.contains(&client_id))
            .await;
    }
}

impl Drop for PresenceHandle {
    fn drop(&mut self) {
        let last = {
            let mut sessions = self.presence.sessions();
            let open = sessions
                .get_mut(&self.client_id)
                .expect("a handle's client is counted until it drops");
            *open -= 1;
            let last = *open == 0;
            if last {
                sessions.remove(&self.client_id);
            }
            last
        };
        if last {
            self.presence.inner.database.publish(Invalidation::Clients);
        }
    }
}

#[cfg(test)]
mod tests;
