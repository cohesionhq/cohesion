//! Calling a `calendar:account` extension's sync functions on its scoped
//! worker, typed by the Calendar Backend contract, each with the account's
//! fresh credential.

use cohesion_core::model::{
    BackendCalendar, BackendCalendarChanges, BackendEvent, BackendEventFields,
    CalendarColorSupport, CalendarEvent, Credential, ExtensionPermissionKind,
    IntelligenceAgentColor,
};
use cohesion_core::protocol::ServerError;
use cohesion_js::{ScriptArgument, ScriptError};

use crate::connectors::{Connection, call_connector, connector_error, json_argument};
use crate::extension_manager::{ExtensionError, ExtensionFn, ExtensionManager};

/// `calendarColorSupport()`: what colors the backend keeps. Optional: one
/// that doesn't define it is `"declared"`.
struct ColorSupportFn;
impl ExtensionFn for ColorSupportFn {
    const NAME: &'static str = "calendarColorSupport";
    const REQUIRES: ExtensionPermissionKind = ExtensionPermissionKind::CalendarAccount;
    type Output = CalendarColorSupport;
    fn arguments(self) -> Vec<ScriptArgument> {
        Vec::new()
    }
}

/// `updateCalendar(identifier, changes, credential)`: changes a calendar
/// as it was changed in Cohesion, returning the backend's copy. Optional:
/// only a backend whose color support is `"exact"` is called, and one that
/// doesn't define it keeps its own color.
struct UpdateCalendarFn<'a> {
    identifier: String,
    changes: BackendCalendarChanges,
    credential: &'a Credential,
}
impl ExtensionFn for UpdateCalendarFn<'_> {
    const NAME: &'static str = "updateCalendar";
    const REQUIRES: ExtensionPermissionKind = ExtensionPermissionKind::CalendarAccount;
    type Output = BackendCalendar;
    fn arguments(self) -> Vec<ScriptArgument> {
        vec![
            ScriptArgument::String(self.identifier),
            json_argument(&self.changes),
            json_argument(self.credential),
        ]
    }
}

/// `listCalendars(credential)`: the account's calendars.
struct ListCalendarsFn<'a> {
    credential: &'a Credential,
}
impl ExtensionFn for ListCalendarsFn<'_> {
    const NAME: &'static str = "listCalendars";
    const REQUIRES: ExtensionPermissionKind = ExtensionPermissionKind::CalendarAccount;
    type Output = Vec<BackendCalendar>;
    fn arguments(self) -> Vec<ScriptArgument> {
        vec![json_argument(self.credential)]
    }
}

/// `listEvents(calendar, credential)`: every event of calendar `calendar`
/// that Cohesion holds.
struct ListEventsFn<'a> {
    calendar: String,
    credential: &'a Credential,
}
impl ExtensionFn for ListEventsFn<'_> {
    const NAME: &'static str = "listEvents";
    const REQUIRES: ExtensionPermissionKind = ExtensionPermissionKind::CalendarAccount;
    type Output = Vec<BackendEvent>;
    fn arguments(self) -> Vec<ScriptArgument> {
        vec![
            ScriptArgument::String(self.calendar),
            json_argument(self.credential),
        ]
    }
}

/// `createEvent(calendar, event, credential)`: adds an event made in
/// Cohesion, returning the backend's copy.
struct CreateEventFn<'a> {
    calendar: String,
    event: BackendEventFields,
    credential: &'a Credential,
}
impl ExtensionFn for CreateEventFn<'_> {
    const NAME: &'static str = "createEvent";
    const REQUIRES: ExtensionPermissionKind = ExtensionPermissionKind::CalendarAccount;
    type Output = BackendEvent;
    fn arguments(self) -> Vec<ScriptArgument> {
        vec![
            ScriptArgument::String(self.calendar),
            json_argument(&self.event),
            json_argument(self.credential),
        ]
    }
}

/// `updateEvent(calendar, identifier, event, credential)`: replaces the
/// event with what it says in Cohesion, returning the backend's copy.
struct UpdateEventFn<'a> {
    calendar: String,
    identifier: String,
    event: BackendEventFields,
    credential: &'a Credential,
}
impl ExtensionFn for UpdateEventFn<'_> {
    const NAME: &'static str = "updateEvent";
    const REQUIRES: ExtensionPermissionKind = ExtensionPermissionKind::CalendarAccount;
    type Output = BackendEvent;
    fn arguments(self) -> Vec<ScriptArgument> {
        vec![
            ScriptArgument::String(self.calendar),
            ScriptArgument::String(self.identifier),
            json_argument(&self.event),
            json_argument(self.credential),
        ]
    }
}

/// `deleteEvent(calendar, identifier, credential)`: deletes an event
/// deleted in Cohesion.
struct DeleteEventFn<'a> {
    calendar: String,
    identifier: String,
    credential: &'a Credential,
}
impl ExtensionFn for DeleteEventFn<'_> {
    const NAME: &'static str = "deleteEvent";
    const REQUIRES: ExtensionPermissionKind = ExtensionPermissionKind::CalendarAccount;
    type Output = ();
    fn arguments(self) -> Vec<ScriptArgument> {
        vec![
            ScriptArgument::String(self.calendar),
            ScriptArgument::String(self.identifier),
            json_argument(self.credential),
        ]
    }
}

/// What `event` says, as a backend keeping colors as `support` says takes
/// it: only one that keeps any color is sent the event's own.
fn fields(event: &CalendarEvent, support: CalendarColorSupport) -> BackendEventFields {
    BackendEventFields {
        title: event.title.clone(),
        location: event.location.clone(),
        description: event.description.clone(),
        timing: event.timing.clone().into(),
        invitees: event.invitees.iter().cloned().map(Into::into).collect(),
        color: match support {
            CalendarColorSupport::Exact => {
                event.selected_color.map(|color| color.hex().to_string())
            }
            CalendarColorSupport::Declared | CalendarColorSupport::None => None,
        },
    }
}

/// A connected account's calendar backend.
pub(super) struct Backend<'a> {
    pub(super) extensions: &'a ExtensionManager,
    pub(super) connection: Connection,
    /// What colors it keeps, from [`color_support`](Backend::color_support).
    pub(super) color_support: CalendarColorSupport,
}

impl Backend<'_> {
    async fn call<F: ExtensionFn>(&self, function: F) -> Result<F::Output, ServerError> {
        call_connector(self.extensions, &self.connection.extension, function).await
    }

    /// What colors the backend keeps: what its `calendarColorSupport()`
    /// returns, or `"declared"` when it doesn't define one.
    pub(super) async fn color_support(
        extensions: &ExtensionManager,
        connection: &Connection,
    ) -> Result<CalendarColorSupport, ServerError> {
        match extensions
            .call(connection.extension.id, ColorSupportFn)
            .await
        {
            Ok(support) => Ok(support),
            Err(ExtensionError::Script(ScriptError::MissingFunction(_))) => {
                Ok(CalendarColorSupport::Declared)
            }
            Err(e) => Err(connector_error(
                &connection.extension,
                ColorSupportFn::NAME,
                e,
            )),
        }
    }

    /// Pushes `color`, chosen in Cohesion, to calendar `identifier`: the
    /// backend's copy, or `None` when it doesn't define `updateCalendar`.
    pub(super) async fn recolor(
        &self,
        identifier: &str,
        color: IntelligenceAgentColor,
    ) -> Result<Option<BackendCalendar>, ServerError> {
        let function = UpdateCalendarFn {
            identifier: identifier.to_string(),
            changes: BackendCalendarChanges {
                color: color.hex().to_string(),
            },
            credential: &self.connection.credential,
        };
        match self
            .extensions
            .call(self.connection.extension.id, function)
            .await
        {
            Ok(calendar) => Ok(Some(calendar)),
            Err(ExtensionError::Script(ScriptError::MissingFunction(_))) => Ok(None),
            Err(e) => Err(connector_error(
                &self.connection.extension,
                UpdateCalendarFn::NAME,
                e,
            )),
        }
    }

    pub(super) async fn calendars(&self) -> Result<Vec<BackendCalendar>, ServerError> {
        self.call(ListCalendarsFn {
            credential: &self.connection.credential,
        })
        .await
    }

    pub(super) async fn events(&self, calendar: &str) -> Result<Vec<BackendEvent>, ServerError> {
        self.call(ListEventsFn {
            calendar: calendar.to_string(),
            credential: &self.connection.credential,
        })
        .await
    }

    pub(super) async fn create(
        &self,
        calendar: &str,
        event: &CalendarEvent,
    ) -> Result<BackendEvent, ServerError> {
        self.call(CreateEventFn {
            calendar: calendar.to_string(),
            event: fields(event, self.color_support),
            credential: &self.connection.credential,
        })
        .await
    }

    pub(super) async fn update(
        &self,
        calendar: &str,
        identifier: &str,
        event: &CalendarEvent,
    ) -> Result<BackendEvent, ServerError> {
        self.call(UpdateEventFn {
            calendar: calendar.to_string(),
            identifier: identifier.to_string(),
            event: fields(event, self.color_support),
            credential: &self.connection.credential,
        })
        .await
    }

    pub(super) async fn delete(&self, calendar: &str, identifier: &str) -> Result<(), ServerError> {
        self.call(DeleteEventFn {
            calendar: calendar.to_string(),
            identifier: identifier.to_string(),
            credential: &self.connection.credential,
        })
        .await
    }
}
