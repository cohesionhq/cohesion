//! Reading the followed feeds: on request, and every hour from
//! daemon start, every feed at once. A feed is read on a blocking thread,
//! skipping the items it already has, then stored by one database command.

mod feed_refresh;
mod read_feed;

pub(crate) use read_feed::{read, refresh_feed, refresh_feeds};
