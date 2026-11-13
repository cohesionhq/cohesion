//! Calendars: calling a `calendar:backend` extension's sync functions with
//! an account's credential, and the pass that syncs each account's calendar
//! store with its backend, both ways.

mod backend_calls;
mod calendar_sync;

pub(crate) use calendar_sync::sync_calendars;

#[cfg(test)]
mod tests;
