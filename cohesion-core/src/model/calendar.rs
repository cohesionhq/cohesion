//! Calendars: the stores that hold them, the calendars themselves and their
//! events, and the exact colors they are drawn in. Calendars are
//! daemon-wide, like feeds: they belong to no workspace.

use cohesion_macros::{ColumnGroup, Fields, Model, TextEnum, TsInterface, TsWire};
use serde::{Deserialize, Serialize};

use super::IntelligenceAgentColor;
use crate::value::{FromValue, IntoValue, Value, ValueError};

/// An exact color, stored as `"#rrggbb"`, with the palette color nearest to
/// it.
///
/// A color chosen in the apps is a palette color's exact value; one a
/// calendar backend sends can be any color, and `nearest` is how a client
/// that only draws the palette shows it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, TsWire, TsInterface)]
pub struct CalendarColor {
    /// The exact color, as `"#rrggbb"` in lowercase.
    pub hex: String,
    /// The palette color nearest to `hex`.
    pub nearest: IntelligenceAgentColor,
}

impl CalendarColor {
    /// The exact value of palette color `color`.
    pub fn palette(color: IntelligenceAgentColor) -> Self {
        Self {
            hex: color.hex().to_string(),
            nearest: color,
        }
    }

    /// `text` as a color: `"#rrggbb"` or `"#rgb"`, in either case. `None`
    /// for anything else.
    pub fn parse(text: &str) -> Option<Self> {
        let digits = text.strip_prefix('#')?;
        if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let digits = match digits.len() {
            6 => digits.to_ascii_lowercase(),
            3 => digits
                .chars()
                .flat_map(|c| [c, c])
                .collect::<String>()
                .to_ascii_lowercase(),
            _ => return None,
        };
        let rgb = rgb(&digits);
        Some(Self {
            hex: format!("#{digits}"),
            nearest: nearest(rgb),
        })
    }

    /// What a client chose: a palette color, or an exact one, refused when
    /// it isn't a color.
    pub fn chosen(choice: &CalendarColorChoice) -> Option<Self> {
        match choice {
            CalendarColorChoice::Palette { color } => Some(Self::palette(*color)),
            CalendarColorChoice::Exact { hex } => Self::parse(hex),
        }
    }
}

/// The color a client sets on a calendar or an event. The apps' picker
/// offers the palette; scripts and backends may give any exact color.
///
/// - `"palette"`: palette color `color`, stored as its exact value.
/// - `"exact"`: `hex`, as `"#rrggbb"` or `"#rgb"`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CalendarColorChoice {
    Palette { color: IntelligenceAgentColor },
    Exact { hex: String },
}

/// Six lowercase hex digits as red, green and blue.
fn rgb(digits: &str) -> [u8; 3] {
    let channel = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).expect("hex digits");
    [channel(0), channel(2), channel(4)]
}

/// The palette color nearest to `rgb`, by distance in Oklab, where equal
/// distances look about equally different.
fn nearest(rgb: [u8; 3]) -> IntelligenceAgentColor {
    let target = oklab(rgb);
    IntelligenceAgentColor::ALL
        .iter()
        .copied()
        .min_by(|a, b| {
            let distance = |color: &IntelligenceAgentColor| {
                let other = oklab(self::rgb(&color.hex()[1..]));
                (0..3).map(|i| (target[i] - other[i]).powi(2)).sum::<f64>()
            };
            distance(a).total_cmp(&distance(b))
        })
        .expect("the palette isn't empty")
}

/// An sRGB color in Oklab.
fn oklab([r, g, b]: [u8; 3]) -> [f64; 3] {
    let linear = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let (r, g, b) = (linear(r), linear(g), linear(b));
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    [
        0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s,
    ]
}

impl IntoValue for CalendarColor {
    fn into_value(self) -> Value {
        Value::Text(self.hex)
    }
}

/// The schema's `CHECK` keeps a stored color `"#rrggbb"`.
impl FromValue for CalendarColor {
    fn from_value(value: Value) -> Result<Self, ValueError> {
        let text = String::from_value(value)?;
        Self::parse(&text).ok_or_else(|| ValueError::Invalid(format!("{text} isn't a color")))
    }
}

/// What holds calendars: Cohesion itself, or an account of a calendar
/// backend, which calendars sync with both ways.
#[derive(Clone, Debug, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "calendar_stores", backup = Included)]
pub struct CalendarStore {
    /// Unique, stable identifier.
    pub id: i64,
    /// The store's name: for a backend's, the account's description when it
    /// was added.
    pub title: String,
    /// Where its calendars live.
    #[model(group)]
    #[fields(skip)]
    pub backend: CalendarStoreBackend,
    /// When it last synced with its backend in full, in Unix seconds;
    /// `null` for a local store, or one that hasn't yet.
    pub last_synced_at: Option<i64>,
    /// Why its last sync failed; `null` when it worked, or for a local store.
    pub sync_failure: Option<String>,
    /// When the store was added, in Unix seconds.
    pub created_at: i64,
}

/// Where a store's calendars live. The table's check gives every row
/// exactly one of these.
///
/// - `"local"`: only in Cohesion.
/// - `"account"`: connector account `account_id`, whose extension holds
///   `calendar:backend`. An account backs at most one store, and removing
///   the account removes the store and its calendars.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ColumnGroup, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CalendarStoreBackend {
    Local,
    Account { account_id: i64 },
}

/// A calendar: a named, colored set of events in a store.
#[derive(Clone, Debug, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "calendars", backup = Included)]
pub struct Calendar {
    /// Unique, stable identifier.
    pub id: i64,
    /// Id of the store that holds it.
    pub store_id: i64,
    /// Whether the calendar has been deleted. Listings skip deleted
    /// calendars, so this is normally `false`.
    pub deleted: bool,
    /// Never empty. A backend's calendar takes its backend's title at each
    /// sync.
    pub title: String,
    /// The color its events take unless they have their own. A backend's
    /// calendar starts in the backend's color; after that it is Cohesion's.
    #[fields(skip)]
    pub color: CalendarColor,
    /// The backend's identifier for it; `null` in a local store.
    #[fields(skip)]
    pub remote_identifier: Option<String>,
    /// Whether its backend refuses changes to it, so Cohesion does too.
    /// Always `false` in a local store.
    pub read_only: bool,
    /// When the calendar was added, in Unix seconds.
    pub created_at: i64,
    /// When its title or color last changed, in Unix seconds.
    pub modified: i64,
}

/// When an event happens. The table's check gives every row exactly one of
/// these.
///
/// - `"timed"`: from `starts_at` to `ends_at`, in Unix seconds, in IANA time
///   zone `time_zone` (such as `"Europe/London"`), which says how to show it.
/// - `"allDay"`: whole days, from `start_date` to `end_date` inclusive, as
///   `"YYYY-MM-DD"`, the same wherever they are read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ColumnGroup, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EventTiming {
    Timed {
        starts_at: i64,
        ends_at: i64,
        time_zone: String,
    },
    AllDay {
        start_date: String,
        end_date: String,
    },
}

/// How an invitee answered an event's invitation; stored in the event's
/// JSON.
///
/// - `"needsAction"`: hasn't answered.
/// - `"accepted"`: is coming.
/// - `"declined"`: isn't coming.
/// - `"tentative"`: might come.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum InviteeResponse {
    #[default]
    NeedsAction,
    Accepted,
    Declined,
    Tentative,
}

/// Someone invited to an event.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, TsWire)]
pub struct EventInvitee {
    /// Their email address; never empty.
    pub email: String,
    /// Their name, when it is known.
    #[serde(default)]
    pub name: Option<String>,
    /// How they answered.
    #[serde(default)]
    pub response: InviteeResponse,
}

/// An event on a calendar.
#[derive(Clone, Debug, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "calendar_events", backup = Included)]
pub struct CalendarEvent {
    /// Unique, stable identifier.
    pub id: i64,
    /// Id of the calendar it is on.
    pub calendar_id: i64,
    /// Whether the event has been deleted. Listings skip deleted events, so
    /// this is normally `false`.
    pub deleted: bool,
    /// Never empty.
    pub title: String,
    /// Where it happens, as written; `""` when it says nowhere.
    pub location: String,
    /// Free text about it, as plain text / Markdown.
    pub description: String,
    /// When it happens.
    #[model(group)]
    #[fields(skip)]
    pub timing: EventTiming,
    /// Who is invited, in the order they were given.
    #[model(json)]
    #[fields(skip)]
    pub invitees: Vec<EventInvitee>,
    /// Its own color; `null` to take its calendar's.
    #[fields(skip)]
    pub color: Option<CalendarColor>,
    /// The backend's identifier for it; `null` in a local store, or until
    /// it is first pushed.
    #[fields(skip)]
    pub remote_identifier: Option<String>,
    /// The backend's `lastModified` for it when it last synced, in Unix
    /// seconds.
    #[fields(skip)]
    pub remote_modified: Option<i64>,
    /// Whether it changed in Cohesion since it last synced, so the next sync
    /// pushes it. Always `false` in a local store.
    pub pending_push: bool,
    /// Counts its changes, so a sync writes over an event only as it read
    /// it.
    #[fields(skip)]
    pub revision: i64,
    /// When the event was added, in Unix seconds.
    pub created_at: i64,
    /// When it last changed, in Cohesion or from its backend, in Unix
    /// seconds.
    pub modified: i64,
}

/// What an event says, as a client or script creates or replaces it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, TsWire)]
pub struct CalendarEventFields {
    /// May not be empty.
    pub title: String,
    pub location: String,
    pub description: String,
    /// A timed event may not end before it starts, nor an all-day one end
    /// on a day before it starts.
    pub timing: EventTiming,
    pub invitees: Vec<EventInvitee>,
    /// `null` to take the calendar's color.
    pub color: Option<CalendarColorChoice>,
}

#[cfg(test)]
mod tests;
