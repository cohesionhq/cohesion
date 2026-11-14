//! Syncing calendar stores with their backends, both ways: on a timer from
//! daemon start, and on request. One pass runs at a time.
//!
//! For each account whose extension holds `calendar:account`, the pass
//! makes sure the account has a store, reads what colors the backend keeps
//! (`calendarColorSupport`), brings the store's calendars in line with
//! `listCalendars`, pushes a calendar color chosen in Cohesion to a backend
//! that keeps any color (`updateCalendar`), then for each calendar lists its
//! events and settles each one:
//!
//! - Changed only on the backend (its `lastModified` moved since the last
//!   sync), or new there: written in Cohesion.
//! - Changed only in Cohesion: pushed with `createEvent`, `updateEvent` or
//!   `deleteEvent`.
//! - Changed on both: the later change wins, the backend's `lastModified`
//!   against Cohesion's `modified`; Cohesion's wins a tie.
//! - No longer listed by the backend: removed from Cohesion, even with an
//!   unpushed change, since the backend gives no time for a deletion.
//!
//! A failure ends that store's pass, which is recorded on the store and
//! logged; the other stores carry on.

use std::collections::HashMap;
use std::time::Duration;

use cohesion_core::log::Module;
use cohesion_core::model::{
    AccountHost, BackendEvent, Calendar, CalendarColorSupport, CalendarEvent, CalendarStore,
    ConnectorAccount, ExtensionPermissionKind,
};
use cohesion_core::protocol::{CalendarSyncSummary, ServerError};
use cohesion_core::{log_error, log_info};
use cohesion_db::{
    ApplyBackendEventCommand, EnsureCalendarStoreCommand, ForgetCalendarEventCommand,
    ListConnectorAccountRowsCommand, ListEventsToSyncCommand, RecordCalendarColorPushedCommand,
    RecordCalendarSyncCommand, RecordEventPushedCommand, SyncBackendCalendarsCommand, SyncedEvent,
};

use super::backend_calls::Backend;
use crate::cohesion_context::CohesionContext;
use crate::connectors::connect;
use crate::extension_manager::ExtensionError;

/// How long after start the first pass runs.
const FIRST_PASS: Duration = Duration::from_secs(60);
/// How long between passes.
const INTERVAL: Duration = Duration::from_secs(10 * 60);

impl CohesionContext {
    /// Syncs every calendar store a minute after start, then every 10
    /// minutes, until the daemon shuts down.
    pub(crate) fn start_calendar_sync(&self) {
        let ctx = self.clone();
        self.work
            .every("calendar sync", FIRST_PASS, INTERVAL, move || {
                let ctx = ctx.clone();
                async move {
                    sync_calendars(&ctx).await;
                }
            });
    }
}

/// Syncs every backend account's store, waiting for a pass under way to
/// end first.
pub(crate) async fn sync_calendars(ctx: &CohesionContext) -> CalendarSyncSummary {
    let _pass = ctx.calendar_sync.lock().await;
    let mut summary = CalendarSyncSummary::default();
    let accounts = match ctx.database.send(ListConnectorAccountRowsCommand {}).await {
        Ok(accounts) => accounts,
        Err(e) => {
            log_error!(
                Module::Extensions,
                "Listing connector accounts failed: {e:?}"
            );
            return summary;
        }
    };
    for account in accounts {
        let AccountHost::Extension { extension_id, .. } = account.host else {
            continue;
        };
        match ctx
            .extensions
            .granted(extension_id, ExtensionPermissionKind::CalendarAccount)
            .await
        {
            Ok(_) => {}
            Err(ExtensionError::NotGranted(_)) => continue,
            Err(e) => {
                log_error!(
                    Module::Extensions,
                    "Reading the extension of connector account {} failed: {e:?}",
                    account.id
                );
                continue;
            }
        }
        sync_account(ctx, &account, &mut summary).await;
    }
    log_info!(
        Module::Extensions,
        "Calendar sync: {} synced, {} failed, {} pulled, {} pushed, {} conflicts.",
        summary.synced,
        summary.failed,
        summary.pulled,
        summary.pushed,
        summary.conflicts
    );
    summary
}

/// Syncs `account`'s store, made first if it has none, and records how it
/// went.
async fn sync_account(
    ctx: &CohesionContext,
    account: &ConnectorAccount,
    summary: &mut CalendarSyncSummary,
) {
    let store = match ctx
        .database
        .send(EnsureCalendarStoreCommand {
            account_id: account.id,
            title: account.description.clone(),
            color_support: None,
        })
        .await
    {
        Ok(store) => store,
        Err(e) => {
            log_error!(
                Module::Extensions,
                "Making the calendar store of connector account {} failed: {e:?}",
                account.id
            );
            summary.failed += 1;
            return;
        }
    };
    let mut tally = CalendarSyncSummary::default();
    let outcome = sync_store(ctx, account, &store, &mut tally).await;
    summary.pulled += tally.pulled;
    summary.pushed += tally.pushed;
    summary.conflicts += tally.conflicts;
    let failure = match outcome {
        Ok(()) => {
            summary.synced += 1;
            None
        }
        Err(e) => {
            summary.failed += 1;
            log_error!(
                Module::Extensions,
                "Syncing the calendars of {} failed: {}",
                account.description,
                e.message
            );
            Some(e.message)
        }
    };
    let recorded = ctx
        .database
        .send(RecordCalendarSyncCommand {
            store_id: store.id,
            failure,
        })
        .await;
    if let Err(e) = recorded {
        log_error!(
            Module::Extensions,
            "Recording calendar store {}'s sync failed: {e:?}",
            store.id
        );
    }
}

/// Syncs `store`, `account`'s, counting what it does in `tally`.
async fn sync_store(
    ctx: &CohesionContext,
    account: &ConnectorAccount,
    store: &CalendarStore,
    tally: &mut CalendarSyncSummary,
) -> Result<(), ServerError> {
    let connection = connect(
        &ctx.extensions,
        &ctx.database,
        account,
        ExtensionPermissionKind::CalendarAccount,
    )
    .await?;
    let color_support = Backend::color_support(&ctx.extensions, &connection).await?;
    if color_support != store.color_support {
        ctx.database
            .send(EnsureCalendarStoreCommand {
                account_id: account.id,
                title: account.description.clone(),
                color_support: Some(color_support),
            })
            .await?;
    }
    let backend = Backend {
        extensions: &ctx.extensions,
        connection,
        color_support,
    };
    let calendars = backend.calendars().await?;
    let calendars = ctx
        .database
        .send(SyncBackendCalendarsCommand {
            store_id: store.id,
            calendars,
        })
        .await?;
    for calendar in calendars {
        push_color(ctx, &backend, &calendar).await?;
        sync_calendar(ctx, &backend, &calendar, tally).await?;
    }
    Ok(())
}

/// Pushes `calendar`'s color, chosen in Cohesion, to a backend that keeps
/// any color. One that doesn't define `updateCalendar` keeps its own: the
/// chosen color stays in Cohesion and stops waiting.
async fn push_color(
    ctx: &CohesionContext,
    backend: &Backend<'_>,
    calendar: &Calendar,
) -> Result<(), ServerError> {
    if !calendar.color_pending_push || backend.color_support != CalendarColorSupport::Exact {
        return Ok(());
    }
    let Some(identifier) = calendar.remote_identifier.as_deref() else {
        return Ok(());
    };
    let declared = match backend.recolor(identifier, calendar.selected_color).await? {
        Some(remote) => backend.color_support.declared(remote.color.as_deref()),
        None => calendar.declared_color,
    };
    ctx.database
        .send(RecordCalendarColorPushedCommand {
            calendar_id: calendar.id,
            pushed: calendar.selected_color,
            declared,
        })
        .await?;
    Ok(())
}

/// Settles every event of `calendar` with `backend`, as the module says.
async fn sync_calendar(
    ctx: &CohesionContext,
    backend: &Backend<'_>,
    calendar: &Calendar,
    tally: &mut CalendarSyncSummary,
) -> Result<(), ServerError> {
    let Some(identifier) = calendar.remote_identifier.as_deref() else {
        return Ok(());
    };
    let mut remote: HashMap<String, BackendEvent> = backend
        .events(identifier)
        .await?
        .into_iter()
        .map(|event| (event.identifier.clone(), event))
        .collect();
    let local = ctx
        .database
        .send(ListEventsToSyncCommand {
            calendar_id: calendar.id,
        })
        .await?;
    for event in local {
        let listed = event
            .remote_identifier
            .as_ref()
            .and_then(|id| remote.remove(id));
        settle(ctx, backend, calendar, identifier, event, listed, tally).await?;
    }
    for (_, event) in remote {
        apply(ctx, backend, calendar, None, event, tally).await?;
    }
    Ok(())
}

/// Settles `event` with `listed`, the backend's copy of it, if it listed
/// one.
async fn settle(
    ctx: &CohesionContext,
    backend: &Backend<'_>,
    calendar: &Calendar,
    identifier: &str,
    event: CalendarEvent,
    listed: Option<BackendEvent>,
    tally: &mut CalendarSyncSummary,
) -> Result<(), ServerError> {
    let read = SyncedEvent::from(&event);
    let Some(remote_identifier) = event.remote_identifier.clone() else {
        // Made in Cohesion and never pushed.
        let created = backend.create(identifier, &event).await?;
        tally.pushed += 1;
        return pushed(ctx, backend, read, created).await;
    };
    let Some(listed) = listed else {
        // The backend deleted it.
        if ctx
            .database
            .send(ForgetCalendarEventCommand { event: read })
            .await?
        {
            tally.pulled += 1;
        }
        return Ok(());
    };
    let remote_changed = event.remote_modified != Some(listed.last_modified);
    if !event.pending_push {
        if remote_changed {
            apply(ctx, backend, calendar, Some(read), listed, tally).await?;
        }
        return Ok(());
    }
    if remote_changed {
        tally.conflicts += 1;
        if listed.last_modified > event.modified {
            return apply(ctx, backend, calendar, Some(read), listed, tally).await;
        }
    }
    tally.pushed += 1;
    if event.deleted {
        backend.delete(identifier, &remote_identifier).await?;
        ctx.database
            .send(ForgetCalendarEventCommand { event: read })
            .await?;
        return Ok(());
    }
    let updated = backend
        .update(identifier, &remote_identifier, &event)
        .await?;
    pushed(ctx, backend, read, updated).await
}

/// Writes the backend's copy `remote` over `existing`, or as a new event.
async fn apply(
    ctx: &CohesionContext,
    backend: &Backend<'_>,
    calendar: &Calendar,
    existing: Option<SyncedEvent>,
    remote: BackendEvent,
    tally: &mut CalendarSyncSummary,
) -> Result<(), ServerError> {
    let written = ctx
        .database
        .send(ApplyBackendEventCommand {
            calendar_id: calendar.id,
            existing,
            remote,
            color_support: backend.color_support,
        })
        .await?;
    if written.is_some() {
        tally.pulled += 1;
    }
    Ok(())
}

/// Records that the change `read` stood for reached the backend as
/// `remote`.
async fn pushed(
    ctx: &CohesionContext,
    backend: &Backend<'_>,
    read: SyncedEvent,
    remote: BackendEvent,
) -> Result<(), ServerError> {
    ctx.database
        .send(RecordEventPushedCommand {
            event: read,
            remote_identifier: remote.identifier,
            remote_modified: remote.last_modified,
            declared: backend.color_support.declared(remote.color.as_deref()),
        })
        .await?;
    Ok(())
}
