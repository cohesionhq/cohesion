//! What the transports note at the daemon's door: a refused pairing,
//! recorded in the security activity at most once an hour per address, and
//! when and from where each client last connected.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use cohesion_core::model::SecurityEventKind;
use cohesion_core::{Client, log::Module, log_error};
use cohesion_db::{
    ClientSeenCommand, DatabaseCommand, DatabaseContext, RecordSecurityEventCommand,
};

/// How long a refused pairing from one address is recorded once.
const REPORT_EVERY: Duration = Duration::from_secs(60 * 60);
/// The most addresses remembered as recorded at once.
const REPORT_CAPACITY: usize = 4096;
/// How often, at most, a client's last connection is rewritten while its
/// address stays the same, in seconds.
const SEEN_EVERY: i64 = 60;

/// The door's record. A cheap clone; clones share it.
#[derive(Clone)]
pub struct Security {
    inner: Arc<Inner>,
}

struct Inner {
    database: DatabaseContext,
    /// When a refused pairing from each address was last recorded.
    reported: Mutex<HashMap<IpAddr, Instant>>,
}

impl Security {
    pub fn new(database: DatabaseContext) -> Self {
        Self {
            inner: Arc::new(Inner {
                database,
                reported: Mutex::default(),
            }),
        }
    }

    /// A pairing attempt from `ip` was refused: recorded, unless one from
    /// it was within the last hour.
    pub fn pairing_refused(&self, ip: IpAddr) {
        let ip = ip.to_canonical();
        let now = Instant::now();
        {
            let mut reported = self.inner.reported.lock().unwrap();
            if reported
                .get(&ip)
                .is_some_and(|&at| now.duration_since(at) < REPORT_EVERY)
            {
                return;
            }
            if reported.len() >= REPORT_CAPACITY {
                reported.retain(|_, at| now.duration_since(*at) < REPORT_EVERY);
            }
            if reported.len() >= REPORT_CAPACITY {
                return;
            }
            reported.insert(ip, now);
        }
        self.spawn(RecordSecurityEventCommand {
            kind: SecurityEventKind::PairingRefused,
            client: None,
            address: Some(ip.to_string()),
        });
    }

    /// Notes that `client` connected now, from `address` (`None` on this
    /// computer). Skipped while its last connection is recent and from the
    /// same address, so reconnecting costs no write.
    pub fn seen(&self, client: &Client, address: Option<IpAddr>) {
        let address = address.map(|ip| ip.to_canonical().to_string());
        let recent = client
            .last_seen_at
            .is_some_and(|at| unix_now() - at < SEEN_EVERY);
        if recent && client.last_address == address {
            return;
        }
        self.spawn(ClientSeenCommand {
            id: client.id,
            address,
        });
    }

    /// Writes `command` without waiting for it, so a transport isn't held
    /// up by the database.
    fn spawn<C>(&self, command: C)
    where
        C: DatabaseCommand + Send + 'static,
        C::Response: Send,
    {
        let database = self.inner.database.clone();
        tokio::spawn(async move {
            if let Err(e) = database.send(command).await {
                log_error!(Module::Server, "Couldn’t record a connection: {e:?}");
            }
        });
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs() as i64)
}
