use cohesion_core::model::{
    BackendEvent, BackendEventTiming, BackendTimedEvent, CalendarColorChoice, CalendarColorSupport,
    CalendarEvent, CalendarEventFields, CalendarStoreBackend, EventTiming, IntelligenceAgentColor,
    SearchEntityKind,
};

use crate::test_support::context;
use crate::{
    ApplyBackendEventCommand, CommandError, CreateCalendarCommand, CreateCalendarEventCommand,
    CreateCalendarStoreCommand, CreateWorkspaceCommand, DatabaseContext, DeleteCalendarCommand,
    DeleteCalendarEventCommand, DeleteCalendarStoreCommand, ForgetCalendarEventCommand,
    GetCalendarEventCommand, GetCalendarPerspectiveCommand, ListCalendarEventsCommand,
    ListCalendarStoresCommand, ListCalendarsCommand, ListEventsToSyncCommand,
    RecordEventPushedCommand, RenameCalendarStoreCommand, SearchWorkspaceCommand,
    SetCalendarColorCommand, SetCalendarPerspectiveCommand, SyncedEvent,
    UpdateCalendarEventCommand,
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
        workspace_id: None,
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
        selected_color: None,
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
fn an_exact_color_binds_to_the_nearest_palette_color() {
    let db = context();
    let calendar_id = calendar(&db);
    let calendars = db.send_blocking(ListCalendarsCommand {}).unwrap();
    assert_eq!(calendars[0].selected_color, IntelligenceAgentColor::Blue);
    assert_eq!(calendars[0].declared_color, None);

    let recolored = db
        .send_blocking(SetCalendarColorCommand {
            calendar_id,
            color: CalendarColorChoice::Exact {
                hex: "#D50000".to_string(),
            },
        })
        .unwrap();
    assert_eq!(recolored.selected_color, IntelligenceAgentColor::Red);
    // A local store's calendar has nothing to push to.
    assert!(!recolored.color_pending_push);

    let mut fields = timed("Dentist", NINE, 1);
    fields.selected_color = Some(CalendarColorChoice::Exact {
        hex: "#2e7d32".to_string(),
    });
    let event = create(&db, calendar_id, fields);
    assert_eq!(event.selected_color, Some(IntelligenceAgentColor::Green));

    let mut fields = timed("Dentist", NINE, 1);
    fields.selected_color = Some(CalendarColorChoice::Exact {
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
fn cohesion_starts_with_a_store_of_its_own_that_can_be_named() {
    let db = context();
    let stores = db.send_blocking(ListCalendarStoresCommand {}).unwrap();
    assert_eq!(stores.len(), 1);
    let cohesion = &stores[0];
    assert_eq!(cohesion.title, "Cohesion");
    assert!(!cohesion.named_by_user);
    assert_eq!(cohesion.backend, CalendarStoreBackend::Local);
    assert_eq!(cohesion.color_support, CalendarColorSupport::None);

    let named = db
        .send_blocking(RenameCalendarStoreCommand {
            store_id: cohesion.id,
            title: Some("Personal".to_string()),
        })
        .unwrap();
    assert_eq!(
        (named.title.as_str(), named.named_by_user),
        ("Personal", true)
    );
    assert!(matches!(
        db.send_blocking(RenameCalendarStoreCommand {
            store_id: cohesion.id,
            title: Some(String::new()),
        }),
        Err(CommandError::Validation(_))
    ));
    let reset = db
        .send_blocking(RenameCalendarStoreCommand {
            store_id: cohesion.id,
            title: None,
        })
        .unwrap();
    assert_eq!(
        (reset.title.as_str(), reset.named_by_user),
        ("Cohesion", false)
    );

    // A store made by hand is named by its maker.
    let home = db
        .send_blocking(CreateCalendarStoreCommand {
            title: "Home".to_string(),
        })
        .unwrap();
    assert!(home.named_by_user);
}

fn workspace(db: &DatabaseContext) -> i64 {
    db.send_blocking(CreateWorkspaceCommand {
        title: "w".to_string(),
        emoji: None,
    })
    .unwrap()
    .id
}

#[test]
fn a_workspace_shows_every_calendar_until_it_chooses() {
    let db = context();
    let family = calendar(&db);
    let workspace_id = workspace(&db);
    let perspective = |db: &DatabaseContext| {
        db.send_blocking(GetCalendarPerspectiveCommand { workspace_id })
            .unwrap()
            .calendar_ids
    };
    assert_eq!(perspective(&db), None);

    let store_id = db.send_blocking(ListCalendarsCommand {}).unwrap()[0].store_id;
    let make = |db: &DatabaseContext, title: &str, workspace_id: Option<i64>| {
        db.send_blocking(CreateCalendarCommand {
            store_id,
            title: title.to_string(),
            color: CalendarColorChoice::Palette {
                color: IntelligenceAgentColor::Green,
            },
            workspace_id,
        })
        .unwrap()
        .id
    };
    // Without a perspective, a calendar made in the workspace leaves it
    // showing every calendar.
    let work = make(&db, "Work", Some(workspace_id));
    assert_eq!(perspective(&db), None);

    let set = db
        .send_blocking(SetCalendarPerspectiveCommand {
            workspace_id,
            calendar_ids: Some(vec![work, work]),
        })
        .unwrap();
    assert_eq!(set.calendar_ids, Some(vec![work]));

    // With one, a calendar made in the workspace joins it; one made
    // elsewhere doesn't.
    let gym = make(&db, "Gym", Some(workspace_id));
    make(&db, "Elsewhere", None);
    assert_eq!(perspective(&db), Some(vec![work, gym]));

    // A deleted calendar leaves it; an unknown one is refused.
    db.send_blocking(DeleteCalendarCommand { calendar_id: gym })
        .unwrap();
    assert_eq!(perspective(&db), Some(vec![work]));
    assert!(matches!(
        db.send_blocking(SetCalendarPerspectiveCommand {
            workspace_id,
            calendar_ids: Some(vec![family, gym]),
        }),
        Err(CommandError::Validation(_))
    ));

    // An empty choice shows nothing; forgetting it shows everything again.
    let empty = db
        .send_blocking(SetCalendarPerspectiveCommand {
            workspace_id,
            calendar_ids: Some(Vec::new()),
        })
        .unwrap();
    assert_eq!(empty.calendar_ids, Some(Vec::new()));
    db.send_blocking(SetCalendarPerspectiveCommand {
        workspace_id,
        calendar_ids: None,
    })
    .unwrap();
    assert_eq!(perspective(&db), None);
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
    let workspace_id = workspace(&db);
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
            color_support: CalendarColorSupport::Declared,
        })
        .unwrap()
        .unwrap();
    // An empty title from a backend is Untitled; a color that isn't one is
    // none.
    assert_eq!(pulled.title, "Untitled");
    assert_eq!(pulled.declared_color, None);
    assert_eq!(pulled.selected_color, None);
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
            color_support: CalendarColorSupport::Declared,
        })
        .unwrap();
    assert!(overwritten.is_none());
    let recorded = db
        .send_blocking(RecordEventPushedCommand {
            event: read,
            remote_identifier: "r1".to_string(),
            remote_modified: NINE,
            declared: None,
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

#[test]
fn an_events_selected_color_follows_its_declared_one_when_that_changes() {
    let db = context();
    let calendar_id = calendar(&db);
    let colored = |identifier: &str, color: Option<&str>, last_modified: i64| BackendEvent {
        color: color.map(str::to_string),
        ..remote(identifier, "Standup", last_modified)
    };
    let apply = |existing: Option<SyncedEvent>, remote: BackendEvent, support| {
        db.send_blocking(ApplyBackendEventCommand {
            calendar_id,
            existing,
            remote,
            color_support: support,
        })
        .unwrap()
        .unwrap()
    };
    let declared = CalendarColorSupport::Declared;
    let event = apply(None, colored("r1", Some("#d50000"), NINE), declared);
    assert_eq!(event.declared_color, Some(IntelligenceAgentColor::Red));
    assert_eq!(event.selected_color, Some(IntelligenceAgentColor::Red));

    // A color chosen in Cohesion changes the selected one only.
    let mut fields = timed("Standup", NINE, 1);
    fields.selected_color = Some(CalendarColorChoice::Palette {
        color: IntelligenceAgentColor::Teal,
    });
    let event = db
        .send_blocking(UpdateCalendarEventCommand {
            event_id: event.id,
            event: fields,
        })
        .unwrap();
    assert_eq!(event.declared_color, Some(IntelligenceAgentColor::Red));
    assert_eq!(event.selected_color, Some(IntelligenceAgentColor::Teal));

    // The same declared color keeps the choice; a new one overrides it.
    let event = apply(
        Some(SyncedEvent::from(&event)),
        colored("r1", Some("#d50000"), NINE + 1),
        declared,
    );
    assert_eq!(event.selected_color, Some(IntelligenceAgentColor::Teal));
    let event = apply(
        Some(SyncedEvent::from(&event)),
        colored("r1", Some("#2e7d32"), NINE + 2),
        declared,
    );
    assert_eq!(event.selected_color, Some(IntelligenceAgentColor::Green));

    // A backend without colors declares none, whatever it sends.
    let event = apply(
        None,
        colored("r2", Some("#d50000"), NINE),
        CalendarColorSupport::None,
    );
    assert_eq!((event.declared_color, event.selected_color), (None, None));
}
