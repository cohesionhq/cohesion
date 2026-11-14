use cohesion_core::model::{
    CalendarColorChoice, CalendarColorSupport, CalendarEvent, CalendarEventFields, CalendarStore,
    CalendarStoreBackend, EventTiming, ExtensionPermissionKind, IntelligenceAgentColor,
};
use cohesion_core::protocol::{
    CompleteConnectorAuthenticationRequest, Local, Route, SetExtensionPermissionRequest,
    SyncCalendarsRequest,
};
use cohesion_db::{
    Container, CreateCalendarEventCommand, DeleteCalendarEventCommand, ListCalendarEventsCommand,
    ListCalendarStoresCommand, ListCalendarsCommand, Secrets, SetCalendarColorCommand,
    UpdateCalendarEventCommand,
};
use serde_json::{Value, json};

use crate::CohesionContext;

fn context() -> CohesionContext {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    CohesionContext::new(
        container,
        Secrets::new(),
        cohesion_core::ServerAddress {
            host: std::net::IpAddr::from(std::net::Ipv4Addr::LOCALHOST).into(),
            port: 3000,
            fingerprint: String::new(),
        },
    )
}

/// A `calendar:account` extension holding one calendar, Work, in memory,
/// with one event, Standup (`e1`), changed at 1000. What it pushes is
/// stamped 1000 too. It records each call it answers until `taken()` hands
/// them over; `remote()` returns its events, and the other helpers change
/// them as someone using the service would.
const CALENDAR_EXTENSION: &str = "function manifest() {
       return { identifier: 'com.example.calendar', name: 'Calendar', version: 1,
                permissions: ['oauth', 'calendar:account'], hooks: [] };
     }
     function serviceDescription() { return { title: 'Calendar', unit: 'Account' }; }
     function authenticationURL() { return 'https://example.com/auth'; }
     function completeAuthentication(code) { return { authorization: code }; }
     function getAccountDescription(credential) {
       return { identifier: credential.authorization, description: 'ada@example.com' };
     }
     let calls = [], next = 1;
     const events = {
       e1: { identifier: 'e1', lastModified: 1000, title: 'Standup', location: 'Room 1',
             timing: { kind: 'timed', startsAt: 5000, endsAt: 5600, timeZone: 'Europe/London' },
             invitees: [{ email: 'ada@example.com', response: 'accepted' }], color: '#d50000' },
     };
     let workColor = '#039be5';
     function listCalendars(credential) {
       calls.push('calendars:' + credential.authorization);
       return [{ identifier: 'work', title: 'Work', color: workColor }];
     }
     function recoloredThere() { workColor = '#0b8043'; return null; }
     function listEvents(calendar) { calls.push('list:' + calendar); return Object.values(events); }
     function createEvent(calendar, event) {
       calls.push('create');
       const identifier = 'n' + next++;
       events[identifier] = { ...event, identifier, lastModified: 1000 };
       return events[identifier];
     }
     function updateEvent(calendar, identifier, event) {
       calls.push('update:' + identifier);
       events[identifier] = { ...event, identifier, lastModified: 1000 };
       return events[identifier];
     }
     function deleteEvent(calendar, identifier) {
       calls.push('delete:' + identifier);
       delete events[identifier];
       return null;
     }
     function taken() { const t = calls; calls = []; return t; }
     function remote() { return events; }
     function editedEarly() { events.e1 = { ...events.e1, title: 'Early', lastModified: 2000 }; return null; }
     function editedLate() {
       events.e1 = { ...events.e1, title: 'Late', lastModified: 4000000000 };
       return null;
     }
     function deletedThere() { delete events.e1; return null; }";

/// Installs `script` with `oauth` on, signs in, and turns `calendar:account`
/// on when `backend`: the extension's id.
async fn signed_in(ctx: &CohesionContext, script: &str, backend: bool) -> i64 {
    let extension_id = ctx
        .extensions
        .install(script.to_string(), true)
        .await
        .unwrap()
        .id;
    let mut permissions = vec![ExtensionPermissionKind::OAuth];
    if backend {
        permissions.push(ExtensionPermissionKind::CalendarAccount);
    }
    for permission in permissions {
        ctx.extensions
            .set_permission(SetExtensionPermissionRequest {
                extension_id,
                permission,
                enabled: true,
            })
            .await
            .unwrap();
    }
    Route::<Local, _>::route(
        ctx,
        CompleteConnectorAuthenticationRequest {
            extension_id,
            code: "token".to_string(),
        },
    )
    .await
    .unwrap();
    extension_id
}

/// Calls `function` of extension `extension_id`, without arguments.
async fn call(ctx: &CohesionContext, extension_id: i64, function: &'static str) -> Value {
    ctx.extensions
        .scoped(extension_id, None, move |sandbox| {
            sandbox.call_returning(function)
        })
        .await
        .unwrap()
}

async fn sync(ctx: &CohesionContext) -> (i64, i64, i64, i64, i64) {
    let summary = Route::<Local, _>::route(ctx, SyncCalendarsRequest)
        .await
        .unwrap();
    (
        summary.synced,
        summary.failed,
        summary.pulled,
        summary.pushed,
        summary.conflicts,
    )
}

async fn events(ctx: &CohesionContext) -> Vec<CalendarEvent> {
    ctx.database
        .send(ListCalendarEventsCommand {
            calendar_ids: None,
            from: 0,
            // 1 January 2100.
            to: 4_102_444_800,
        })
        .await
        .unwrap()
}

/// The event Cohesion holds for the backend's `e1`.
async fn standup(ctx: &CohesionContext) -> CalendarEvent {
    events(ctx)
        .await
        .into_iter()
        .find(|event| event.remote_identifier.as_deref() == Some("e1"))
        .unwrap()
}

fn fields(title: &str) -> CalendarEventFields {
    CalendarEventFields {
        title: title.to_string(),
        location: String::new(),
        description: String::new(),
        timing: EventTiming::AllDay {
            start_date: "2026-11-13".to_string(),
            end_date: "2026-11-13".to_string(),
        },
        invitees: Vec::new(),
        selected_color: None,
    }
}

/// The store of the account, the last listed after Cohesion's own.
async fn account_store(ctx: &CohesionContext) -> CalendarStore {
    let stores = ctx
        .database
        .send(ListCalendarStoresCommand {})
        .await
        .unwrap();
    assert_eq!(stores.len(), 2);
    assert_eq!(stores[0].backend, CalendarStoreBackend::Local);
    stores[1].clone()
}

async fn edit(ctx: &CohesionContext, event_id: i64, title: &str) {
    ctx.database
        .send(UpdateCalendarEventCommand {
            event_id,
            event: fields(title),
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn a_sync_pulls_the_accounts_calendars_and_pushes_what_changed_here() {
    let ctx = context();
    let extension_id = signed_in(&ctx, CALENDAR_EXTENSION, true).await;

    assert_eq!(sync(&ctx).await, (1, 0, 1, 0, 0));
    let store = account_store(&ctx).await;
    assert!(matches!(
        store.backend,
        CalendarStoreBackend::Account { .. }
    ));
    assert_eq!(store.title, "ada@example.com");
    // Without `calendarColorSupport`, a backend's colors are declared ones.
    assert_eq!(store.color_support, CalendarColorSupport::Declared);
    assert!(store.last_synced_at.is_some());
    let calendars = ctx.database.send(ListCalendarsCommand {}).await.unwrap();
    assert_eq!(calendars[0].title, "Work");
    assert_eq!(
        calendars[0].declared_color,
        Some(IntelligenceAgentColor::Blue)
    );
    assert_eq!(calendars[0].selected_color, IntelligenceAgentColor::Blue);
    let pulled = standup(&ctx).await;
    assert_eq!(pulled.title, "Standup");
    assert_eq!(pulled.invitees[0].email, "ada@example.com");
    assert_eq!(pulled.declared_color, Some(IntelligenceAgentColor::Red));
    assert_eq!(pulled.selected_color, Some(IntelligenceAgentColor::Red));
    assert_eq!(
        call(&ctx, extension_id, "taken").await,
        json!(["calendars:token", "list:work"])
    );

    // A new event and an edit wait, then are pushed.
    let created = ctx
        .database
        .send(CreateCalendarEventCommand {
            calendar_id: calendars[0].id,
            event: fields("Lunch"),
        })
        .await
        .unwrap();
    assert!(created.pending_push);
    edit(&ctx, pulled.id, "Standup, moved").await;
    assert_eq!(sync(&ctx).await, (1, 0, 0, 2, 0));
    assert_eq!(
        call(&ctx, extension_id, "taken").await,
        json!(["calendars:token", "list:work", "update:e1", "create"])
    );
    let remote = call(&ctx, extension_id, "remote").await;
    assert_eq!(remote["e1"]["title"], "Standup, moved");
    assert_eq!(remote["n1"]["title"], "Lunch");
    assert_eq!(remote["n1"]["timing"]["kind"], "allDay");
    assert!(events(&ctx).await.iter().all(|event| !event.pending_push));

    // Nothing changed since: nothing moves.
    assert_eq!(sync(&ctx).await, (1, 0, 0, 0, 0));
}

#[tokio::test]
async fn a_conflict_goes_to_the_later_change() {
    let ctx = context();
    let extension_id = signed_in(&ctx, CALENDAR_EXTENSION, true).await;
    sync(&ctx).await;
    let event = standup(&ctx).await;

    // Changed here now, and there at 2000: here is later, and is pushed.
    edit(&ctx, event.id, "Here").await;
    call(&ctx, extension_id, "editedEarly").await;
    assert_eq!(sync(&ctx).await, (1, 0, 0, 1, 1));
    let remote = call(&ctx, extension_id, "remote").await;
    assert_eq!(remote["e1"]["title"], "Here");

    // Changed here now, and there in 2096: there is later, and is pulled.
    edit(&ctx, event.id, "Here again").await;
    call(&ctx, extension_id, "editedLate").await;
    assert_eq!(sync(&ctx).await, (1, 0, 1, 0, 1));
    let event = standup(&ctx).await;
    assert_eq!(event.title, "Late");
    assert!(!event.pending_push);
    assert_eq!(event.modified, 4_000_000_000);
}

#[tokio::test]
async fn deletions_reach_the_other_side() {
    let ctx = context();
    let extension_id = signed_in(&ctx, CALENDAR_EXTENSION, true).await;
    sync(&ctx).await;
    let event = standup(&ctx).await;

    // Deleted here: deleted there.
    ctx.database
        .send(DeleteCalendarEventCommand { event_id: event.id })
        .await
        .unwrap();
    call(&ctx, extension_id, "taken").await;
    assert_eq!(sync(&ctx).await, (1, 0, 0, 1, 0));
    assert_eq!(
        call(&ctx, extension_id, "taken").await,
        json!(["calendars:token", "list:work", "delete:e1"])
    );
    assert_eq!(call(&ctx, extension_id, "remote").await, json!({}));
    assert!(events(&ctx).await.is_empty());

    // Deleted there, even with a change here waiting: deleted here.
    let ctx = context();
    let extension_id = signed_in(&ctx, CALENDAR_EXTENSION, true).await;
    sync(&ctx).await;
    let event = standup(&ctx).await;
    edit(&ctx, event.id, "Here").await;
    call(&ctx, extension_id, "deletedThere").await;
    assert_eq!(sync(&ctx).await, (1, 0, 1, 0, 0));
    assert!(events(&ctx).await.is_empty());
}

#[tokio::test]
async fn an_account_without_calendar_account_has_no_store() {
    let ctx = context();
    let extension_id = signed_in(&ctx, CALENDAR_EXTENSION, false).await;
    assert_eq!(sync(&ctx).await, (0, 0, 0, 0, 0));
    let stores = ctx
        .database
        .send(ListCalendarStoresCommand {})
        .await
        .unwrap();
    // Only Cohesion's own.
    assert_eq!(stores.len(), 1);
    assert_eq!(stores[0].backend, CalendarStoreBackend::Local);
    assert_eq!(call(&ctx, extension_id, "taken").await, json!([]));
}

/// Sets calendar `calendar_id`'s selected color to `color` from Cohesion.
async fn choose(ctx: &CohesionContext, calendar_id: i64, color: IntelligenceAgentColor) {
    ctx.database
        .send(SetCalendarColorCommand {
            calendar_id,
            color: CalendarColorChoice::Palette { color },
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn a_color_chosen_here_stays_here_on_a_backend_with_colors_of_its_own() {
    let ctx = context();
    let extension_id = signed_in(&ctx, CALENDAR_EXTENSION, true).await;
    sync(&ctx).await;
    let work = ctx.database.send(ListCalendarsCommand {}).await.unwrap()[0].clone();

    choose(&ctx, work.id, IntelligenceAgentColor::Purple).await;
    call(&ctx, extension_id, "taken").await;
    sync(&ctx).await;
    let work = ctx.database.send(ListCalendarsCommand {}).await.unwrap()[0].clone();
    assert_eq!(work.selected_color, IntelligenceAgentColor::Purple);
    assert!(!work.color_pending_push);
    assert_eq!(
        call(&ctx, extension_id, "taken").await,
        json!(["calendars:token", "list:work"])
    );

    // The backend's color changing there overrides the choice.
    call(&ctx, extension_id, "recoloredThere").await;
    sync(&ctx).await;
    let work = ctx.database.send(ListCalendarsCommand {}).await.unwrap()[0].clone();
    assert_eq!(work.declared_color, Some(IntelligenceAgentColor::Green));
    assert_eq!(work.selected_color, IntelligenceAgentColor::Green);
}

#[tokio::test]
async fn a_color_chosen_here_is_pushed_to_a_backend_that_keeps_any_color() {
    let script = format!(
        "{CALENDAR_EXTENSION}
         function calendarColorSupport() {{ return 'exact'; }}
         function updateCalendar(identifier, changes) {{
           calls.push('recolor:' + identifier + ':' + changes.color);
           workColor = changes.color;
           return {{ identifier, title: 'Work', color: workColor }};
         }}"
    );
    let ctx = context();
    let extension_id = signed_in(&ctx, &script, true).await;
    sync(&ctx).await;
    assert_eq!(
        account_store(&ctx).await.color_support,
        CalendarColorSupport::Exact
    );
    let work = ctx.database.send(ListCalendarsCommand {}).await.unwrap()[0].clone();

    choose(&ctx, work.id, IntelligenceAgentColor::Purple).await;
    let waiting = ctx.database.send(ListCalendarsCommand {}).await.unwrap()[0].clone();
    assert!(waiting.color_pending_push);
    call(&ctx, extension_id, "taken").await;
    sync(&ctx).await;
    let purple = IntelligenceAgentColor::Purple.hex();
    assert_eq!(
        call(&ctx, extension_id, "taken").await,
        json!([
            "calendars:token",
            format!("recolor:work:{purple}"),
            "list:work"
        ])
    );
    let work = ctx.database.send(ListCalendarsCommand {}).await.unwrap()[0].clone();
    assert_eq!(work.declared_color, Some(IntelligenceAgentColor::Purple));
    assert_eq!(work.selected_color, IntelligenceAgentColor::Purple);
    assert!(!work.color_pending_push);

    // An event's own color is pushed as its palette color's exact value.
    let event = standup(&ctx).await;
    let mut teal = fields("Standup");
    teal.selected_color = Some(CalendarColorChoice::Palette {
        color: IntelligenceAgentColor::Teal,
    });
    ctx.database
        .send(UpdateCalendarEventCommand {
            event_id: event.id,
            event: teal,
        })
        .await
        .unwrap();
    sync(&ctx).await;
    let remote = call(&ctx, extension_id, "remote").await;
    assert_eq!(remote["e1"]["color"], IntelligenceAgentColor::Teal.hex());
    let event = standup(&ctx).await;
    assert_eq!(event.declared_color, Some(IntelligenceAgentColor::Teal));
}
