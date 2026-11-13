//! Calling a `calendar:backend` extension's sync functions on its scoped
//! worker, typed by the Calendar Backend contract, each with the account's
//! fresh credential.

use cohesion_core::model::{
    BackendCalendar, BackendEvent, BackendEventFields, CalendarEvent, Credential,
    ExtensionPermissionKind,
};
use cohesion_core::protocol::ServerError;
use cohesion_js::ScriptArgument;

use crate::connectors::{Connection, call_connector, json_argument};
use crate::extension_manager::{ExtensionFn, ExtensionManager};

/// `listCalendars(credential)`: the account's calendars.
struct ListCalendarsFn<'a> {
    credential: &'a Credential,
}
impl ExtensionFn for ListCalendarsFn<'_> {
    const NAME: &'static str = "listCalendars";
    const REQUIRES: ExtensionPermissionKind = ExtensionPermissionKind::CalendarBackend;
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
    const REQUIRES: ExtensionPermissionKind = ExtensionPermissionKind::CalendarBackend;
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
    const REQUIRES: ExtensionPermissionKind = ExtensionPermissionKind::CalendarBackend;
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
    const REQUIRES: ExtensionPermissionKind = ExtensionPermissionKind::CalendarBackend;
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
    const REQUIRES: ExtensionPermissionKind = ExtensionPermissionKind::CalendarBackend;
    type Output = ();
    fn arguments(self) -> Vec<ScriptArgument> {
        vec![
            ScriptArgument::String(self.calendar),
            ScriptArgument::String(self.identifier),
            json_argument(self.credential),
        ]
    }
}

/// What `event` says, as a backend takes it.
fn fields(event: &CalendarEvent) -> BackendEventFields {
    BackendEventFields {
        title: event.title.clone(),
        location: event.location.clone(),
        description: event.description.clone(),
        timing: event.timing.clone().into(),
        invitees: event.invitees.iter().cloned().map(Into::into).collect(),
        color: event.color.as_ref().map(|color| color.hex.clone()),
    }
}

/// A connected account's calendar backend.
pub(super) struct Backend<'a> {
    pub(super) extensions: &'a ExtensionManager,
    pub(super) connection: Connection,
}

impl Backend<'_> {
    async fn call<F: ExtensionFn>(&self, function: F) -> Result<F::Output, ServerError> {
        call_connector(self.extensions, &self.connection.extension, function).await
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
            event: fields(event),
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
            event: fields(event),
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
