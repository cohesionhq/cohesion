//! `CalendarStore`, `Calendar` and `CalendarEvent` in scripts. An event's
//! timing and invitees read and take the shapes a calendar backend's
//! `listEvents` returns (`BackendEventTiming`, `BackendInvitee`).

use cohesion_core::model::{
    BackendEventTiming, BackendInvitee, Calendar, CalendarColor, CalendarColorChoice,
    CalendarEvent, CalendarEventFields, CalendarStore, CalendarStoreBackend,
    IntelligenceAgentColor,
};
use cohesion_db::{
    CreateCalendarEventCommand, DeleteCalendarCommand, DeleteCalendarEventCommand,
    ListCalendarEventsCommand, RenameCalendarCommand, SetCalendarColorCommand,
    UpdateCalendarEventCommand,
};
use cohesion_macros::{JsOptions, js_entity};
use rquickjs::{Ctx, Exception, Value};

use crate::conversion::FromJsField;

/// What holds calendars: Cohesion itself, or an account of a calendar
/// backend, whose calendars sync with it both ways every 10 minutes.
///
/// Get them from `cohesion.calendarStores()`. `lastSyncedAt` and
/// `createdAt` are in Unix seconds; `syncFailure` says why the last sync
/// failed.
#[js_entity(CalendarStore)]
impl JsCalendarStore {
    /// The connector account whose backend holds the store's calendars, or
    /// `null` for a store only in Cohesion.
    #[qjs(get, rename = "accountId")]
    pub fn account_id(&self) -> Option<i64> {
        match self.inner.backend {
            CalendarStoreBackend::Local => None,
            CalendarStoreBackend::Account { account_id } => Some(account_id),
        }
    }
}

/// A calendar: a named, colored set of events, shared by every workspace.
///
/// Get them from `cohesion.calendars()` or `cohesion.createCalendar()`. A
/// calendar from a backend takes its title from the backend; its color is
/// Cohesion's own. `readOnly` calendars refuse new, changed and deleted
/// events.
#[js_entity(Calendar)]
impl JsCalendar {
    /// The calendar's color: `{ hex: "#rrggbb", nearest }`, where `nearest`
    /// is the palette color closest to it.
    #[qjs(get)]
    #[ts(returns = "CalendarColor")]
    pub fn color<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        json_value(&ctx, &self.inner.color)
    }

    /// Assigning saves immediately: a palette color's name (`"blue"`) or an
    /// exact `"#rrggbb"`. Anything else throws.
    #[qjs(set, rename = "color")]
    pub fn set_color<'js>(&mut self, ctx: Ctx<'js>, color: String) -> rquickjs::Result<()> {
        let color = color_choice(&ctx, &color)?;
        self.inner = self.write(
            &ctx,
            SetCalendarColorCommand {
                calendar_id: self.inner.id,
                color,
            },
        )?;
        Ok(())
    }

    /// Changes the title. An empty title throws, as does a backend's
    /// calendar. Not available in read-only contexts.
    pub fn rename<'js>(&mut self, ctx: Ctx<'js>, title: String) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            RenameCalendarCommand {
                calendar_id: self.inner.id,
                title,
            },
        )?;
        Ok(())
    }

    /// The calendar's events that overlap `from` to `to`, in Unix seconds,
    /// by when they start.
    pub fn events<'js>(
        &self,
        ctx: Ctx<'js>,
        options: CalendarEventListOptions,
    ) -> rquickjs::Result<Vec<JsCalendarEvent>> {
        let events = self.read(
            &ctx,
            ListCalendarEventsCommand {
                calendar_ids: Some(vec![self.inner.id]),
                from: options.from,
                to: options.to,
            },
        )?;
        Ok(events
            .into_iter()
            .map(|event| JsCalendarEvent::new(event, self.db.clone()))
            .collect())
    }

    /// Adds an event to the calendar and returns it; on a backend's
    /// calendar, the next sync pushes it. Throws on a read-only calendar,
    /// an empty title, or an event that ends before it starts. Not available
    /// in read-only contexts.
    #[qjs(rename = "createEvent")]
    pub fn create_event<'js>(
        &self,
        ctx: Ctx<'js>,
        options: CalendarEventOptions,
    ) -> rquickjs::Result<JsCalendarEvent> {
        let event = options.fields(&ctx)?;
        let event = self.write(
            &ctx,
            CreateCalendarEventCommand {
                calendar_id: self.inner.id,
                event,
            },
        )?;
        Ok(JsCalendarEvent::new(event, self.db.clone()))
    }

    /// Deletes the calendar and its events; a backend's calendar throws.
    /// Not available in read-only contexts.
    pub fn delete<'js>(&mut self, ctx: Ctx<'js>) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            DeleteCalendarCommand {
                calendar_id: self.inner.id,
            },
        )?;
        Ok(())
    }
}

/// An event on a calendar.
///
/// Get them from `Calendar.events()`, `cohesion.calendarEvents()` or
/// `Calendar.createEvent()`. `modified` is when it last changed, in
/// Cohesion or on its backend, in Unix seconds; `pendingPush` says a change
/// made in Cohesion waits for the next sync to reach its backend.
#[js_entity(CalendarEvent)]
impl JsCalendarEvent {
    /// When it happens: `{ kind: "timed", startsAt, endsAt, timeZone }` or
    /// `{ kind: "allDay", startDate, endDate }`, `endDate` inclusive.
    #[qjs(get)]
    #[ts(returns = "BackendEventTiming")]
    pub fn timing<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        json_value(&ctx, &BackendEventTiming::from(self.inner.timing.clone()))
    }

    /// Who is invited: `[{ email, name, response }]`.
    #[qjs(get)]
    #[ts(returns = "BackendInvitee[]")]
    pub fn invitees<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        let invitees: Vec<BackendInvitee> = self
            .inner
            .invitees
            .iter()
            .cloned()
            .map(Into::into)
            .collect();
        json_value(&ctx, &invitees)
    }

    /// Its own color, `{ hex, nearest }`, or `null` when it takes its
    /// calendar's.
    #[qjs(get)]
    #[ts(returns = "CalendarColor | null")]
    pub fn color<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        json_value(&ctx, &self.inner.color)
    }

    /// Replaces what the event says with `options`, as `createEvent` takes
    /// them; what is left out is cleared, except the title and timing, which
    /// it needs. On a backend's calendar, the next sync pushes it. Not
    /// available in read-only contexts.
    pub fn update<'js>(
        &mut self,
        ctx: Ctx<'js>,
        options: CalendarEventOptions,
    ) -> rquickjs::Result<()> {
        let event = options.fields(&ctx)?;
        self.inner = self.write(
            &ctx,
            UpdateCalendarEventCommand {
                event_id: self.inner.id,
                event,
            },
        )?;
        Ok(())
    }

    /// Deletes the event; on a backend's calendar, the next sync deletes it
    /// there too. Not available in read-only contexts.
    pub fn delete<'js>(&mut self, ctx: Ctx<'js>) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            DeleteCalendarEventCommand {
                event_id: self.inner.id,
            },
        )?;
        Ok(())
    }
}

/// Options for `Calendar.events()` and `cohesion.calendarEvents()`.
#[derive(JsOptions)]
pub struct CalendarEventListOptions {
    /// The range's start, in Unix seconds.
    pub from: i64,
    /// The range's end, in Unix seconds. An all-day event is in range when
    /// one of its days is one of the UTC days the range touches.
    pub to: i64,
}

/// What an event says, for `Calendar.createEvent()` and
/// `CalendarEvent.update()`, e.g.
/// `{ title: "Dentist", timing: { kind: "timed", startsAt, endsAt, timeZone: "Europe/London" } }`.
#[derive(JsOptions)]
pub struct CalendarEventOptions {
    /// May not be empty.
    pub title: String,
    /// When it happens.
    pub timing: BackendEventTiming,
    /// Where it happens; none when left out.
    pub location: Option<String>,
    /// Free text about it; empty when left out.
    pub description: Option<String>,
    /// Who is invited (`[{ email: "ada@example.com" }]`); none when left out.
    pub invitees: Option<Vec<BackendInvitee>>,
    /// A palette color's name (`"blue"`) or an exact `"#rrggbb"`; the
    /// calendar's color when left out.
    pub color: Option<String>,
}

impl CalendarEventOptions {
    fn fields(self, ctx: &Ctx<'_>) -> rquickjs::Result<CalendarEventFields> {
        Ok(CalendarEventFields {
            title: self.title,
            location: self.location.unwrap_or_default(),
            description: self.description.unwrap_or_default(),
            timing: self.timing.into(),
            invitees: self
                .invitees
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            color: self
                .color
                .map(|color| color_choice(ctx, &color))
                .transpose()?,
        })
    }
}

/// `color` as a script writes it: a palette color's name or an exact
/// `"#rrggbb"`. Anything else throws a `TypeError`.
pub(crate) fn color_choice(ctx: &Ctx<'_>, color: &str) -> rquickjs::Result<CalendarColorChoice> {
    if let Some(palette) = IntelligenceAgentColor::ALL
        .iter()
        .find(|palette| palette.as_str() == color)
    {
        return Ok(CalendarColorChoice::Palette { color: *palette });
    }
    if CalendarColor::parse(color).is_some() {
        return Ok(CalendarColorChoice::Exact {
            hex: color.to_string(),
        });
    }
    Err(Exception::throw_type(
        ctx,
        &format!("a color is a palette color's name or \"#rrggbb\", not \"{color}\""),
    ))
}

/// `value` as a plain object, as on the wire.
fn json_value<'js>(ctx: &Ctx<'js>, value: &impl serde::Serialize) -> rquickjs::Result<Value<'js>> {
    ctx.json_parse(serde_json::to_string(value).expect("calendar values serialize"))
}

/// Reads a contract type from a script's value, as the contract's JSON.
fn from_contract<'js, T: serde::de::DeserializeOwned>(
    ctx: &Ctx<'js>,
    value: Value<'js>,
) -> rquickjs::Result<T> {
    let json = serde_json::Value::from_js_field(ctx, value)?;
    serde_json::from_value(json).map_err(|error| Exception::throw_type(ctx, &error.to_string()))
}

/// `{ kind: "timed", startsAt, endsAt, timeZone }` or
/// `{ kind: "allDay", startDate, endDate }`.
impl FromJsField for BackendEventTiming {
    fn from_js_field<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> rquickjs::Result<Self> {
        from_contract(ctx, value)
    }
}

/// `{ email, name?, response? }`.
impl FromJsField for BackendInvitee {
    fn from_js_field<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> rquickjs::Result<Self> {
        from_contract(ctx, value)
    }
}
