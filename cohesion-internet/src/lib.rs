//! Reading the web: fetching an article's page and reducing it to the
//! Markdown of its main content ([`article_snapshot`]), and reading an RSS,
//! Atom or JSON feed's items ([`feed`]), all through one HTTP client and
//! one pipeline from HTML to Markdown, Coherence, Cohesion's own parser. It
//! blocks, so callers run it off the database thread and off an async
//! runtime. It knows nothing of the database or scripts; cohesion-js hands
//! its fetches to scripts that may make them, and cohesionkit to the
//! daemon's requests.

pub mod article_snapshot;
mod coherence;
pub mod feed;
mod fetch;
mod pipeline;
mod published_time;

use published_time::published_time;
