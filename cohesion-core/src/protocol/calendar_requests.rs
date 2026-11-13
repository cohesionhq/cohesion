//! Requests about calendars: their stores, the calendars and their events.
//! Calendars belong to no workspace.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

use crate::model::{CalendarColorChoice, CalendarEventFields};

/// Lists every calendar store: Cohesion's own, then each backend account's,
/// in the order they were added.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListCalendarStoresRequest;

/// Creates a local calendar store titled `title`, which may not be empty. A
/// backend account's store is made by the daemon, once the account's
/// extension holds `calendar:backend`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct CreateCalendarStoreRequest {
    pub title: String,
}

/// Deletes local calendar store `store_id` with its calendars and their
/// events. A backend account's store goes with the account.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DeleteCalendarStoreRequest {
    pub store_id: i64,
}

/// Lists every live calendar, by store, then title ignoring case.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListCalendarsRequest;

/// Creates a calendar titled `title`, which may not be empty, in local
/// store `store_id`. A backend's calendars come from the backend.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct CreateCalendarRequest {
    pub store_id: i64,
    pub title: String,
    pub color: CalendarColorChoice,
}

/// Retitles live calendar `calendar_id`, of a local store, to `title`,
/// which may not be empty.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RenameCalendarRequest {
    pub calendar_id: i64,
    pub title: String,
}

/// Sets live calendar `calendar_id`'s color, in any store. A backend's
/// calendar keeps it in Cohesion: it isn't pushed.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetCalendarColorRequest {
    pub calendar_id: i64,
    pub color: CalendarColorChoice,
}

/// Deletes live calendar `calendar_id`, of a local store, with its events.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DeleteCalendarRequest {
    pub calendar_id: i64,
}

/// Lists the live events on calendars `calendar_ids` (every calendar when
/// `null`) that overlap `from` to `to`, in Unix seconds, by when they
/// start. An all-day event overlaps when one of its days is one of the UTC
/// days the range touches.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct ListCalendarEventsRequest {
    pub calendar_ids: Option<Vec<i64>>,
    pub from: i64,
    pub to: i64,
}

/// Fetches live event `event_id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct GetCalendarEventRequest {
    pub event_id: i64,
}

/// Creates an event on live calendar `calendar_id`; a backend's calendar
/// pushes it at its next sync. A read-only calendar refuses it.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct CreateCalendarEventRequest {
    pub calendar_id: i64,
    pub event: CalendarEventFields,
}

/// Replaces what live event `event_id` says; a backend's event pushes it at
/// its next sync. A read-only calendar refuses it.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct UpdateCalendarEventRequest {
    pub event_id: i64,
    pub event: CalendarEventFields,
}

/// Deletes live event `event_id`; a backend's event is deleted there at its
/// next sync. A read-only calendar refuses it.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DeleteCalendarEventRequest {
    pub event_id: i64,
}

/// Syncs every backend account's calendar store with its backend now,
/// both ways.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SyncCalendarsRequest;

/// What a sync did, over every store it reached.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, TsWire)]
pub struct CalendarSyncSummary {
    /// How many stores synced in full.
    pub synced: i64,
    /// How many stores failed; each one's `sync_failure` says why.
    pub failed: i64,
    /// Events written in Cohesion from their backend: added, changed or
    /// removed.
    pub pulled: i64,
    /// Events written to their backend: added, changed or deleted.
    pub pushed: i64,
    /// Events changed both in Cohesion and on their backend since the last
    /// sync, settled by which changed later.
    pub conflicts: i64,
}
