//! Calendars: the stores that hold them (shown to people as accounts), the
//! calendars themselves and their events, and the palette colors they are
//! drawn in. Calendars are daemon-wide, like feeds: they belong to no
//! workspace, though each workspace may choose which it shows (its
//! perspective).

use cohesion_macros::{ColumnGroup, Fields, Model, TextEnum, TsWire};
use serde::{Deserialize, Serialize};

use super::IntelligenceAgentColor;

/// The palette color nearest to exact color `text`, written `"#rrggbb"` or
/// `"#rgb"` in either case; `None` when it isn't a color. A calendar or event
/// is drawn in a palette color, and an exact color a person or a backend
/// gives is bound to the nearest one.
pub fn nearest_palette_color(text: &str) -> Option<IntelligenceAgentColor> {
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
    Some(nearest(rgb(&digits)))
}

/// The color a client or script sets on a calendar or an event: a palette
/// color, or an exact one, which is bound to the palette color nearest it.
///
/// - `"palette"`: palette color `color`.
/// - `"exact"`: `hex`, as `"#rrggbb"` or `"#rgb"`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CalendarColorChoice {
    Palette { color: IntelligenceAgentColor },
    Exact { hex: String },
}

impl CalendarColorChoice {
    /// The palette color chosen; `None` when an exact color isn't one.
    pub fn palette_color(&self) -> Option<IntelligenceAgentColor> {
        match self {
            Self::Palette { color } => Some(*color),
            Self::Exact { hex } => nearest_palette_color(hex),
        }
    }
}

/// What colors a calendar backend keeps, as its extension's
/// `calendarColorSupport()` says; a store only in Cohesion is `"none"`, as
/// Cohesion owns its colors.
///
/// - `"exact"`: any `"#rrggbb"`. A color chosen in Cohesion is pushed to the
///   backend, as its palette color's exact value.
/// - `"declared"`: colors of its own, but not any color. The backend's color
///   is the declared one; one chosen in Cohesion stays in Cohesion.
/// - `"none"`: no colors. Cohesion owns them, and ignores any it is sent.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum CalendarColorSupport {
    Exact,
    #[default]
    Declared,
    None,
}

impl CalendarColorSupport {
    /// The palette color a backend declares with `color`: none when it sends
    /// none, sends something that isn't a color, or keeps no colors.
    pub fn declared(self, color: Option<&str>) -> Option<IntelligenceAgentColor> {
        match self {
            Self::None => None,
            Self::Exact | Self::Declared => color.and_then(nearest_palette_color),
        }
    }
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

/// The title a store only in Cohesion takes until it is named.
pub const LOCAL_STORE_TITLE: &str = "Cohesion";

/// What holds calendars, shown to people as an account: Cohesion itself, or
/// a connector account of a calendar backend, whose calendars sync with it
/// both ways. There is one store only in Cohesion from the start, titled
/// `"Cohesion"`, and people may add more.
#[derive(Clone, Debug, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "calendar_stores", backup = Included)]
pub struct CalendarStore {
    /// Unique, stable identifier.
    pub id: i64,
    /// The store's name: the one a person gave it, or else its backend's,
    /// which is `"Cohesion"` for a store only in Cohesion and the connector
    /// account's description for a backend's, kept up to date at each sync.
    pub title: String,
    /// Whether a person named the store, so `title` is theirs rather than
    /// its backend's.
    pub named_by_user: bool,
    /// Where its calendars live.
    #[model(group)]
    #[fields(skip)]
    pub backend: CalendarStoreBackend,
    /// What colors its backend keeps, read at each sync; always `"none"` for
    /// a store only in Cohesion.
    pub color_support: CalendarColorSupport,
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
///   `calendar:account`. An account backs at most one store, and removing
///   the account removes the store and its calendars.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ColumnGroup, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CalendarStoreBackend {
    Local,
    Account { account_id: i64 },
}

/// A calendar: a named, colored set of events in a store.
///
/// Its color is a palette color. `declared_color` is the one its backend
/// gives it, and `selected_color` the one it is drawn in: a new calendar
/// takes the declared one, and whenever the declared one changes the
/// selected one follows it. A color chosen in Cohesion changes the selected
/// one, and is pushed to a backend whose `color_support` is `"exact"`.
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
    /// The color it and its events are drawn in, unless an event has its
    /// own.
    pub selected_color: IntelligenceAgentColor,
    /// The color its backend gives it, bound to the nearest palette color;
    /// `null` in a local store, or when the backend gives none.
    pub declared_color: Option<IntelligenceAgentColor>,
    /// Whether `selected_color` changed in Cohesion since it last synced,
    /// so the next sync pushes it. Only ever `true` when its store's
    /// `color_support` is `"exact"`.
    pub color_pending_push: bool,
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

/// An event on a calendar. Its colors work as its calendar's do.
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
    /// The color it is drawn in; `null` to take its calendar's. Follows
    /// `declared_color` whenever that changes, as a calendar's does.
    pub selected_color: Option<IntelligenceAgentColor>,
    /// The color its backend gives it, bound to the nearest palette color;
    /// `null` in a local store, or when it takes its calendar's there.
    pub declared_color: Option<IntelligenceAgentColor>,
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
    /// The color it is drawn in, bound to the nearest palette color;
    /// `null` to take the calendar's.
    pub selected_color: Option<CalendarColorChoice>,
}

/// That workspace `workspace_id` chose which calendars it shows: those its
/// [`CalendarPerspectiveCalendar`] rows name. A workspace without one shows
/// every calendar.
#[derive(Clone, Debug, Serialize, Deserialize, Model)]
#[model(table = "calendar_perspectives", backup = Included)]
pub struct CalendarPerspectiveRow {
    pub id: i64,
    pub workspace_id: i64,
}

/// A calendar in a workspace's perspective.
#[derive(Clone, Debug, Serialize, Deserialize, Model)]
#[model(table = "calendar_perspective_calendars", backup = Included)]
pub struct CalendarPerspectiveCalendar {
    pub id: i64,
    pub perspective_id: i64,
    pub calendar_id: i64,
}

/// Which calendars a workspace shows: its perspective.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TsWire)]
pub struct CalendarPerspective {
    pub workspace_id: i64,
    /// The live calendars it shows, by id; `null` when it hasn't chosen, so
    /// it shows every calendar, those added later included.
    pub calendar_ids: Option<Vec<i64>>,
}

#[cfg(test)]
mod tests;
