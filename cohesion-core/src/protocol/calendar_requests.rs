//! Requests about calendars: their stores (shown as accounts), the calendars
//! and their events, and which calendars a workspace shows. Calendars belong
//! to no workspace.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

use crate::model::{CalendarColorChoice, CalendarEventFields};

/// Lists every calendar store: those only in Cohesion, then each backend
/// account's, in the order they were added.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListCalendarStoresRequest;

/// Creates a calendar store only in Cohesion, named `title`, which may not
/// be empty. A backend account's store is made by the daemon, once the
/// account's extension holds `calendar:account`.
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

/// Names store `store_id` `title`, which may not be empty, in place of its
/// backend's name; `null` gives it its backend's name back.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RenameCalendarStoreRequest {
    pub store_id: i64,
    pub title: Option<String>,
}

/// Lists every live calendar, by store, then title ignoring case.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListCalendarsRequest;

/// Creates a calendar titled `title`, which may not be empty, in local
/// store `store_id`, colored `color`'s nearest palette color. A backend's
/// calendars come from the backend. Made from workspace `workspace_id`, it
/// joins that workspace's perspective, if it has one.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct CreateCalendarRequest {
    pub store_id: i64,
    pub title: String,
    pub color: CalendarColorChoice,
    pub workspace_id: Option<i64>,
}

/// Retitles live calendar `calendar_id`, of a local store, to `title`,
/// which may not be empty.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RenameCalendarRequest {
    pub calendar_id: i64,
    pub title: String,
}

/// Sets live calendar `calendar_id`'s selected color, in any store, to
/// `color`'s nearest palette color. A backend whose `color_support` is
/// `"exact"` has it pushed at the next sync; any other keeps its own.
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

/// Fetches workspace `workspace_id`'s perspective: which calendars it shows.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct GetCalendarPerspectiveRequest {
    pub workspace_id: i64,
}

/// Sets workspace `workspace_id`'s perspective to calendars `calendar_ids`,
/// each a live calendar; `null` forgets it, so the workspace shows every
/// calendar.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetCalendarPerspectiveRequest {
    pub workspace_id: i64,
    pub calendar_ids: Option<Vec<i64>>,
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
