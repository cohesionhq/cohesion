//! How many rows one page of a paginated listing holds.

/// How many rows a page holds when the request doesn't say.
const DEFAULT_PAGE_SIZE: u64 = 50;
/// The most rows one page holds.
const MAX_PAGE_SIZE: u64 = 500;

/// The rows a page asking for `limit` holds.
pub(crate) fn page_size(limit: Option<u64>) -> u64 {
    limit.unwrap_or(DEFAULT_PAGE_SIZE).min(MAX_PAGE_SIZE)
}
