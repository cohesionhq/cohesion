//! Refreshing every followed feed on a timer from daemon start.

use std::time::Duration;

use cohesion_core::log::Module;
use cohesion_core::{log_error, log_info};
use cohesion_db::ListFeedsCommand;

use super::refresh_feeds;
use crate::cohesion_context::CohesionContext;

/// How long after start the first pass runs.
const FIRST_PASS: Duration = Duration::from_secs(60);
/// How long between passes.
const INTERVAL: Duration = Duration::from_secs(60 * 60);

impl CohesionContext {
    /// Reads every live feed a minute after start, then every hour, until
    /// the daemon shuts down; a pass under way then runs to its end.
    pub(crate) fn start_feed_refresh(&self) {
        let ctx = self.clone();
        self.work
            .every("feed refresh", FIRST_PASS, INTERVAL, move || {
                let ctx = ctx.clone();
                async move {
                    refresh_every_feed(&ctx).await;
                }
            });
    }
}

/// Reads every live feed at once. A feed that can't be read records
/// why on itself; a failure to store is logged, never ending the pass.
async fn refresh_every_feed(ctx: &CohesionContext) {
    let feeds = match ctx.database.send(ListFeedsCommand).await {
        Ok(feeds) => feeds,
        Err(e) => {
            log_error!(Module::Feeds, "Listing feeds failed: {e:?}");
            return;
        }
    };
    let count = feeds.len();
    let ids = feeds.iter().map(|feed| feed.id).collect::<Vec<_>>();
    for (id, result) in ids.into_iter().zip(refresh_feeds(ctx, feeds).await) {
        if let Err(e) = result {
            log_error!(Module::Feeds, "Refreshing feed {id} failed: {e:?}");
        }
    }
    log_info!(Module::Feeds, "Refreshed {count} feeds");
}
