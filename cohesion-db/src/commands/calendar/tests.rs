use cohesion_core::model::{
    BackendEvent, BackendEventTiming, BackendTimedEvent, CalendarColorChoice, CalendarEvent,
    CalendarEventFields, EventTiming, IntelligenceAgentColor, SearchEntityKind,
};

use crate::test_support::context;
use crate::{
    ApplyBackendEventCommand, CommandError, CreateCalendarCommand, CreateCalendarEventCommand,
    CreateCalendarStoreCommand, CreateWorkspaceCommand, DatabaseContext, DeleteCalendarCommand,
    DeleteCalendarEventCommand, DeleteCalendarStoreCommand, ForgetCalendarEventCommand,
    GetCalendarEventCommand, ListCalendarEventsCommand, ListCalendarsCommand,
    ListEventsToSyncCommand, RecordEventPushedCommand, SearchWorkspaceCommand,
    SetCalendarColorCommand, SyncedEvent, UpdateCalendarEventCommand,
};

/// 13 November 2026, 09:00 UTC.
const NINE: i64 = 1_794_560_400;
const HOUR: i64 = 3600;

/// A local store's calendar, in palette blue: its id.
fn calendar(db: &DatabaseContext) -> i64 {
    let store = db
        .send_blocking(CreateCalendarStoreCommand {
            title: "Home".to_string(),
        })
        .unwrap();
    db.send_blocking(CreateCalendarCommand {
        store_id: store.id,
        title: "Family".to_string(),
        color: CalendarColorChoice::Palette {
            color: IntelligenceAgentColor::Blue,
        },
    })
    .unwrap()
    .id
}

fn timed(title: &str, starts_at: i64, hours: i64) -> CalendarEventFields {
    CalendarEventFields {
        title: title.to_string(),
        location: "Kitchen".to_string(),
        description: String::new(),
        timing: EventTiming::Timed {
            starts_at,
            ends_at: starts_at + hours * HOUR,
            time_zone: "Europe/London".to_string(),
        },
        invitees: Vec::new(),
        color: None,
    }
}

fn all_day(title: &str, start: &str, end: &str) -> CalendarEventFields {
    CalendarEventFields {
        timing: EventTiming::AllDay {
            start_date: start.to_string(),
            end_date: end.to_string(),
        },
        ..timed(title, 0, 0)
    }
}

fn create(db: &DatabaseContext, calendar_id: i64, event: CalendarEventFields) -> CalendarEvent {
    db.send_blocking(CreateCalendarEventCommand { calendar_id, event })
        .unwrap()
}

fn listed(db: &DatabaseContext, from: i64, to: i64) -> Vec<String> {
    db.send_blocking(ListCalendarEventsCommand {
        calendar_ids: None,
        from,
        to,
    })
    .unwrap()
    .into_iter()
    .map(|event| event.title)
    .collect()
}

#[test]
fn events_list_by_when_they_overlap_the_range() {
    let db = context();
    let calendar_id = calendar(&db);
    create(&db, calendar_id, timed("Breakfast", NINE, 1));
    create(&db, calendar_id, timed("Lunch", NINE + 3 * HOUR, 1));
    create(
        &db,
        calendar_id,
        all_day("Holiday", "2026-11-13", "2026-11-14"),
    );
    create(
        &db,
        calendar_id,
        all_day("Last week", "2026-11-01", "2026-11-02"),
    );

    // The all-day event covers 13 November, so it starts first.
    assert_eq!(
        listed(&db, NINE, NINE + 4 * HOUR),
        ["Holiday", "Breakfast", "Lunch"]
    );
    // Touching an edge isn't overlapping; the next day still holds the
    // holiday.
    assert_eq!(listed(&db, NINE + HOUR, NINE + 2 * HOUR), ["Holiday"]);
    assert_eq!(listed(&db, NINE + 24 * HOUR, NINE + 25 * HOUR), ["Holiday"]);
}

#[test]
fn colors_are_exact_with_the_nearest_palette_color() {
    let db = context();
    let calendar_id = calendar(&db);
    let calendars = db.send_blocking(ListCalendarsCommand {}).unwrap();
    assert_eq!(calendars[0].color.hex, IntelligenceAgentColor::Blue.hex());

    let recolored = db
        .send_blocking(SetCalendarColorCommand {
            calendar_id,
            color: CalendarColorChoice::Exact {
                hex: "#D50000".to_string(),
            },
        })
        .unwrap();
    assert_eq!(recolored.color.hex, "#d50000");
    assert_eq!(recolored.color.nearest, IntelligenceAgentColor::Red);

    let mut fields = timed("Dentist", NINE, 1);
    fields.color = Some(CalendarColorChoice::Exact {
        hex: "#2e7d32".to_string(),
    });
    let event = create(&db, calendar_id, fields);
    let color = event.color.unwrap();
    assert_eq!(
        (color.hex.as_str(), color.nearest),
        ("#2e7d32", IntelligenceAgentColor::Green)
    );

    let mut fields = timed("Dentist", NINE, 1);
    fields.color = Some(CalendarColorChoice::Exact {
        hex: "green".to_string(),
    });
    assert!(matches!(
        db.send_blocking(CreateCalendarEventCommand {
            calendar_id,
            event: fields,
        }),
        Err(CommandError::Validation(_))
    ));
}

#[test]
fn the_schema_refuses_an_event_that_ends_before_it_starts() {
    let db = context();
    let calendar_id = calendar(&db);
    for event in [
        timed("Backwards", NINE, -1),
        all_day("Backwards", "2026-11-14", "2026-11-13"),
        all_day("Not a day", "2026-11-31", "2026-11-31"),
        timed("", NINE, 1),
    ] {
        assert!(matches!(
            db.send_blocking(CreateCalendarEventCommand { calendar_id, event }),
            Err(CommandError::Validation(_))
        ));
    }
    let mut no_zone = timed("No zone", NINE, 1);
    no_zone.timing = EventTiming::Timed {
        starts_at: NINE,
        ends_at: NINE,
        time_zone: String::new(),
    };
    assert!(
        db.send_blocking(CreateCalendarEventCommand {
            calendar_id,
            event: no_zone,
        })
        .is_err()
    );
}

#[test]
fn every_workspace_searches_events_while_they_are_live() {
    let db = context();
    let calendar_id = calendar(&db);
    let event = create(&db, calendar_id, timed("Piano lesson", NINE, 1));
    let workspace_id = db
        .send_blocking(CreateWorkspaceCommand {
            title: "w".to_string(),
            emoji: None,
        })
        .unwrap()
        .id;
    let search = |query: &str| {
        db.send_blocking(SearchWorkspaceCommand {
            workspace_id,
            query: query.to_string(),
        })
        .unwrap()
    };
    let hits = search("piano");
    assert_eq!(hits.len(), 1);
    assert!(matches!(
        hits[0].entity_kind,
        SearchEntityKind::CalendarEvent
    ));
    assert_eq!(
        (hits[0].entity_id, hits[0].workspace_id),
        (event.id, workspace_id)
    );
    // The location is searched as contents.
    assert_eq!(search("kitchen").len(), 1);

    db.send_blocking(DeleteCalendarEventCommand { event_id: event.id })
        .unwrap();
    assert!(search("piano").is_empty());
}

#[test]
fn deleting_a_calendar_or_its_store_takes_its_events() {
    let db = context();
    let calendar_id = calendar(&db);
    let event = create(&db, calendar_id, timed("Swim", NINE, 1));
    db.send_blocking(DeleteCalendarCommand { calendar_id })
        .unwrap();
    assert!(matches!(
        db.send_blocking(GetCalendarEventCommand { event_id: event.id }),
        Err(CommandError::NotFound(_))
    ));
    assert!(listed(&db, NINE, NINE + HOUR).is_empty());

    let calendar_id = calendar(&db);
    create(&db, calendar_id, timed("Swim", NINE, 1));
    let calendars = db.send_blocking(ListCalendarsCommand {}).unwrap();
    db.send_blocking(DeleteCalendarStoreCommand {
        store_id: calendars[0].store_id,
    })
    .unwrap();
    assert!(
        db.send_blocking(ListCalendarsCommand {})
            .unwrap()
            .is_empty()
    );
    assert!(listed(&db, NINE, NINE + HOUR).is_empty());
}

#[test]
fn a_local_stores_events_never_wait_to_be_pushed() {
    let db = context();
    let calendar_id = calendar(&db);
    let event = create(&db, calendar_id, timed("Swim", NINE, 1));
    assert!(!event.pending_push);
    let event = db
        .send_blocking(UpdateCalendarEventCommand {
            event_id: event.id,
            event: timed("Swim", NINE + HOUR, 1),
        })
        .unwrap();
    assert!(!event.pending_push);
    assert_eq!(event.revision, 1);
}

/// A backend's copy of an event, changed at `last_modified`.
fn remote(identifier: &str, title: &str, last_modified: i64) -> BackendEvent {
    BackendEvent {
        identifier: identifier.to_string(),
        last_modified,
        title: title.to_string(),
        location: String::new(),
        description: String::new(),
        timing: BackendEventTiming::Timed(BackendTimedEvent {
            starts_at: NINE,
            ends_at: NINE + HOUR,
            time_zone: "UTC".to_string(),
        }),
        invitees: Vec::new(),
        color: Some("not a color".to_string()),
    }
}

#[test]
fn a_syncs_writes_skip_an_event_changed_since_it_was_read() {
    let db = context();
    let calendar_id = calendar(&db);
    let pulled = db
        .send_blocking(ApplyBackendEventCommand {
            calendar_id,
            existing: None,
            remote: remote("r1", "", NINE),
        })
        .unwrap()
        .unwrap();
    // An empty title from a backend is Untitled; a color that isn't one is
    // none.
    assert_eq!(pulled.title, "Untitled");
    assert_eq!(pulled.color, None);
    assert_eq!(pulled.modified, NINE);
    let read = SyncedEvent::from(&pulled);

    // An edit made after the sync read the event wins over the sync's
    // writes, whatever second it lands in.
    db.send_blocking(UpdateCalendarEventCommand {
        event_id: pulled.id,
        event: timed("Edited", NINE, 1),
    })
    .unwrap();
    let overwritten = db
        .send_blocking(ApplyBackendEventCommand {
            calendar_id,
            existing: Some(read),
            remote: remote("r1", "Remote", NINE + 1),
        })
        .unwrap();
    assert!(overwritten.is_none());
    let recorded = db
        .send_blocking(RecordEventPushedCommand {
            event: read,
            remote_identifier: "r1".to_string(),
            remote_modified: NINE,
        })
        .unwrap();
    assert!(recorded.is_none());
    assert!(
        !db.send_blocking(ForgetCalendarEventCommand { event: read })
            .unwrap()
    );
    let event = db
        .send_blocking(GetCalendarEventCommand {
            event_id: pulled.id,
        })
        .unwrap();
    assert_eq!(event.title, "Edited");

    // As read now, the sync's write lands.
    let removed = db
        .send_blocking(ForgetCalendarEventCommand {
            event: SyncedEvent::from(&event),
        })
        .unwrap();
    assert!(removed);
    assert!(
        db.send_blocking(ListEventsToSyncCommand { calendar_id })
            .unwrap()
            .is_empty()
    );
}
