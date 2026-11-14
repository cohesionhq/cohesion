//! The Calendar Account contract: what the global functions of a
//! `calendar:account` extension take and return. Fields are camelCase, as
//! the connector contract's are. Nothing here is particular to one kind of
//! service.

use cohesion_macros::{TsInterface, TsWire};
use serde::{Deserialize, Serialize};

use super::{EventInvitee, EventTiming, InviteeResponse};

/// A calendar as `listCalendars(credential)` returns it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire, TsInterface)]
pub struct BackendCalendar {
    /// The backend's stable identifier for the calendar.
    pub identifier: String,
    /// Its name; may not be empty.
    pub title: String,
    /// Its color, as `"#rrggbb"`, read at each sync as the calendar's
    /// declared color, bound to the nearest palette color. Missing, not a
    /// color, or from a backend whose `calendarColorSupport()` is `"none"`:
    /// no declared color, and a new calendar starts blue.
    #[serde(default)]
    pub color: Option<String>,
    /// Whether the backend refuses changes to its events, so Cohesion does
    /// too.
    #[serde(rename = "readOnly", default)]
    pub read_only: bool,
}

/// What `updateCalendar(identifier, changes, credential)` changes on a
/// calendar. Only called on a backend whose `calendarColorSupport()` is
/// `"exact"`, after its color was chosen in Cohesion.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire, TsInterface)]
pub struct BackendCalendarChanges {
    /// Its new color, as `"#rrggbb"`.
    pub color: String,
}

/// When a backend's event happens, told apart by `kind`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire, TsInterface)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BackendEventTiming {
    Timed(BackendTimedEvent),
    AllDay(BackendAllDayEvent),
}

/// `kind: "timed"`: from one instant to another.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire, TsInterface)]
pub struct BackendTimedEvent {
    /// When it starts, in Unix seconds.
    #[serde(rename = "startsAt")]
    pub starts_at: i64,
    /// When it ends, in Unix seconds; not before `startsAt`.
    #[serde(rename = "endsAt")]
    pub ends_at: i64,
    /// The IANA time zone it is shown in, such as `"Europe/London"`.
    #[serde(rename = "timeZone")]
    pub time_zone: String,
}

/// `kind: "allDay"`: whole days.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire, TsInterface)]
pub struct BackendAllDayEvent {
    /// The first day, as `"YYYY-MM-DD"`.
    #[serde(rename = "startDate")]
    pub start_date: String,
    /// The last day, inclusive, as `"YYYY-MM-DD"`. Many services store the
    /// day after; subtract it.
    #[serde(rename = "endDate")]
    pub end_date: String,
}

impl From<BackendEventTiming> for EventTiming {
    fn from(timing: BackendEventTiming) -> Self {
        match timing {
            BackendEventTiming::Timed(t) => EventTiming::Timed {
                starts_at: t.starts_at,
                ends_at: t.ends_at,
                time_zone: t.time_zone,
            },
            BackendEventTiming::AllDay(d) => EventTiming::AllDay {
                start_date: d.start_date,
                end_date: d.end_date,
            },
        }
    }
}

impl From<EventTiming> for BackendEventTiming {
    fn from(timing: EventTiming) -> Self {
        match timing {
            EventTiming::Timed {
                starts_at,
                ends_at,
                time_zone,
            } => Self::Timed(BackendTimedEvent {
                starts_at,
                ends_at,
                time_zone,
            }),
            EventTiming::AllDay {
                start_date,
                end_date,
            } => Self::AllDay(BackendAllDayEvent {
                start_date,
                end_date,
            }),
        }
    }
}

/// Someone invited to a backend's event.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire, TsInterface)]
pub struct BackendInvitee {
    /// Their email address.
    pub email: String,
    /// Their name, when the backend knows it.
    #[serde(default)]
    pub name: Option<String>,
    /// How they answered; `"needsAction"` when left out.
    #[serde(default)]
    pub response: InviteeResponse,
}

impl From<BackendInvitee> for EventInvitee {
    fn from(invitee: BackendInvitee) -> Self {
        Self {
            email: invitee.email,
            name: invitee.name,
            response: invitee.response,
        }
    }
}

impl From<EventInvitee> for BackendInvitee {
    fn from(invitee: EventInvitee) -> Self {
        Self {
            email: invitee.email,
            name: invitee.name,
            response: invitee.response,
        }
    }
}

/// What an event says, as Cohesion hands it to `createEvent` and
/// `updateEvent`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire, TsInterface)]
pub struct BackendEventFields {
    pub title: String,
    pub location: String,
    pub description: String,
    pub timing: BackendEventTiming,
    pub invitees: Vec<BackendInvitee>,
    /// Its own color, as `"#rrggbb"`; `null` when it takes its calendar's.
    /// Only a backend whose `calendarColorSupport()` is `"exact"` is sent
    /// one; any other is always sent `null`, and keeps the event's color as
    /// it is.
    pub color: Option<String>,
}

/// An event as `listEvents`, `createEvent` and `updateEvent` return it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire, TsInterface)]
pub struct BackendEvent {
    /// The backend's stable identifier for the event.
    pub identifier: String,
    /// When the event last changed on the backend, in Unix seconds. Sync
    /// compares it with when it last changed in Cohesion: the later one
    /// wins.
    #[serde(rename = "lastModified")]
    pub last_modified: i64,
    /// May not be empty.
    pub title: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub description: String,
    pub timing: BackendEventTiming,
    #[serde(default)]
    pub invitees: Vec<BackendInvitee>,
    /// Its own color, as `"#rrggbb"`, read as the event's declared color;
    /// missing or `null` when it takes its calendar's.
    #[serde(default)]
    pub color: Option<String>,
}
