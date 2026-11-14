//! Calendars: their stores, the calendars and their events, as clients and
//! scripts change them, the writes a sync with a calendar backend makes, and
//! which calendars each workspace shows (its perspective).
//!
//! A calendar's or event's colors are palette colors. The declared one is
//! its backend's; whenever a sync finds it changed, the selected one follows
//! it. A color chosen in Cohesion changes only the selected one, and on a
//! backend that keeps any color (`"exact"`) waits to be pushed.
//!
//! An event on a backend's calendar that changes in Cohesion is marked
//! `pending_push`, so the next sync pushes it; a deleted one stays, deleted
//! and pending, until its deletion is pushed. The sync's own writes take the
//! `modified` they read, and do nothing when the event changed since, so an
//! edit made during a sync is never lost: the next sync takes it up.

use cohesion_core::FromRequest;
use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{
    BackendCalendar, BackendEvent, Calendar, CalendarColorChoice, CalendarColorSupport,
    CalendarEvent, CalendarEventFields, CalendarPerspective, CalendarPerspectiveCalendar,
    CalendarPerspectiveRow, CalendarStore, CalendarStoreBackend, ConnectorAccount, EventInvitee,
    EventTiming, IntelligenceAgentColor, LOCAL_STORE_TITLE,
};
use cohesion_core::protocol::{
    CreateCalendarEventRequest, CreateCalendarRequest, CreateCalendarStoreRequest,
    DeleteCalendarEventRequest, DeleteCalendarRequest, DeleteCalendarStoreRequest,
    GetCalendarEventRequest, GetCalendarPerspectiveRequest, ListCalendarEventsRequest,
    ListCalendarStoresRequest, ListCalendarsRequest, RenameCalendarRequest,
    RenameCalendarStoreRequest, SetCalendarColorRequest, SetCalendarPerspectiveRequest,
    UpdateCalendarEventRequest,
};
use cohesion_core::value::Json;

use crate::changed::{Changed, Invalidates};
use crate::commands::title::ensure_title;
use crate::commands::workspace_scope::ensure_workspace;
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, Filtered, Found, Read,
    ReadableDatabase, ValidationError, Write,
};

/// The title an event or calendar from a backend takes when it has none.
const UNTITLED: &str = "Untitled";

/// The palette color `color` chooses, refused when an exact color isn't
/// one.
fn chosen_color(color: &CalendarColorChoice) -> Result<IntelligenceAgentColor, ValidationError> {
    color.palette_color().ok_or_else(|| {
        ValidationError::in_field("color", "A color must be written as #rrggbb.".to_string())
    })
}

/// Rejects an invitee without an email address.
fn ensure_invitees(invitees: &[EventInvitee]) -> Result<(), ValidationError> {
    if invitees.iter().any(|invitee| invitee.email.is_empty()) {
        return Err(ValidationError::in_field(
            "invitees",
            "An invitee needs an email address.".to_string(),
        ));
    }
    Ok(())
}

/// A calendar's selected color once its declared color went from `before`
/// to `after`: the declared one when it changed to a color, else
/// `selected`, kept.
fn calendar_selected(
    selected: IntelligenceAgentColor,
    before: Option<IntelligenceAgentColor>,
    after: Option<IntelligenceAgentColor>,
) -> IntelligenceAgentColor {
    match after {
        Some(color) if before != after => color,
        _ => selected,
    }
}

/// An event's selected color once its declared color went from `before` to
/// `after`: the declared one when it changed, so an event its backend gives
/// no color of its own takes its calendar's again, else `selected`, kept.
fn event_selected(
    selected: Option<IntelligenceAgentColor>,
    before: Option<IntelligenceAgentColor>,
    after: Option<IntelligenceAgentColor>,
) -> Option<IntelligenceAgentColor> {
    if before != after { after } else { selected }
}

/// What a backend sends as a title, or [`UNTITLED`] when it is empty.
fn backend_title(title: String) -> String {
    if title.is_empty() {
        UNTITLED.to_string()
    } else {
        title
    }
}

/// Store `store_id`.
fn store<'c>(db: impl ReadableDatabase<'c>, store_id: i64) -> DatabaseCommandResult<CalendarStore> {
    db.select()
        .filter_eq(CalendarStore::ID, store_id)
        .one()
        .found()
}

/// Live calendar `calendar_id` and the store that holds it.
fn calendar_in_store<'c>(
    db: impl ReadableDatabase<'c>,
    calendar_id: i64,
) -> DatabaseCommandResult<(Calendar, CalendarStore)> {
    let calendar: Calendar = db
        .select_live()
        .filter_eq(Calendar::ID, calendar_id)
        .one()
        .found()?;
    let store = store(db, calendar.store_id)?;
    Ok((calendar, store))
}

/// Refuses a change to a backend's calendar itself: its backend owns it.
fn ensure_local(store: &CalendarStore) -> Result<(), ValidationError> {
    match store.backend {
        CalendarStoreBackend::Local => Ok(()),
        CalendarStoreBackend::Account { .. } => Err(ValidationError::new(
            "This calendar comes from a connected service; change it there.",
        )),
    }
}

/// Live calendar `calendar_id`, whose events may change, and whether a
/// change to them is pushed to a backend.
fn writable_calendar<'c>(
    db: impl ReadableDatabase<'c>,
    calendar_id: i64,
) -> DatabaseCommandResult<(Calendar, bool)> {
    let (calendar, store) = calendar_in_store(db, calendar_id)?;
    if calendar.read_only {
        return Err(ValidationError::new("This calendar can’t be changed.").into());
    }
    let backed = store.backend != CalendarStoreBackend::Local;
    Ok((calendar, backed))
}

/// Invalidates the calendars, where `value` was just written.
fn calendars_changed<T>(value: T) -> Changed<T> {
    value.invalidating([Invalidation::Calendars])
}

/// Invalidates the calendar events, where `value` was just written.
fn events_changed<T>(value: T) -> Changed<T> {
    value.invalidating([Invalidation::CalendarEvents])
}

/// Every calendar store: local ones, then backends', in the order they were
/// added.
#[derive(FromRequest)]
#[request(ListCalendarStoresRequest)]
pub struct ListCalendarStoresCommand {}
impl DatabaseCommand for ListCalendarStoresCommand {
    type Response = Vec<CalendarStore>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx.db.select().order("account_id IS NOT NULL, id").all()?)
    }
}

/// Creates a local calendar store.
#[derive(FromRequest)]
#[request(CreateCalendarStoreRequest)]
pub struct CreateCalendarStoreCommand {
    pub title: String,
}
impl DatabaseCommand for CreateCalendarStoreCommand {
    type Response = CalendarStore;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<CalendarStore>> {
        ensure_title(&self.title)?;
        let store = ctx
            .db
            .insert::<CalendarStore>()
            .set(CalendarStore::TITLE, self.title)
            .set_true(CalendarStore::NAMED_BY_USER)
            .set_group(CalendarStoreBackend::Local)
            .returning_one()
            .found()?;
        Ok(calendars_changed(store))
    }
}

/// Deletes a local calendar store with its calendars and their events.
#[derive(FromRequest)]
#[request(DeleteCalendarStoreRequest)]
pub struct DeleteCalendarStoreCommand {
    pub store_id: i64,
}
impl DatabaseCommand for DeleteCalendarStoreCommand {
    type Response = CalendarStore;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<CalendarStore>> {
        let store = store(ctx.db.read(), self.store_id)?;
        if store.backend != CalendarStoreBackend::Local {
            return Err(ValidationError::new(
                "These calendars come from a connected service; remove the account to remove them.",
            )
            .into());
        }
        ctx.db
            .delete::<CalendarStore>()
            .filter_eq(CalendarStore::ID, store.id)
            .execute()?;
        Ok(store.invalidating([Invalidation::Calendars, Invalidation::CalendarEvents]))
    }
}

/// Account `account_id`'s calendar store, made if it has none, with its
/// backend's name `title` (unless a person named it) and what colors its
/// backend keeps, `color_support` (when given), brought up to date. Only the sync calls
/// it, for an account whose extension holds `calendar:account`.
pub struct EnsureCalendarStoreCommand {
    pub account_id: i64,
    pub title: String,
    /// `None` keeps what it was, or `"declared"` for a new store.
    pub color_support: Option<CalendarColorSupport>,
}
impl DatabaseCommand for EnsureCalendarStoreCommand {
    type Response = CalendarStore;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<CalendarStore>> {
        let title = backend_title(self.title);
        let existing = ctx
            .db
            .select::<CalendarStore>()
            .filter("account_id = ?")
            .bind(self.account_id)
            .one()?;
        if let Some(store) = existing {
            let retitled = !store.named_by_user && store.title != title;
            let support = self.color_support.unwrap_or(store.color_support);
            if !retitled && store.color_support == support {
                return Ok(store.invalidating_nothing());
            }
            let store = ctx
                .db
                .update::<CalendarStore>()
                .set(
                    CalendarStore::TITLE,
                    if retitled { title } else { store.title },
                )
                .set(CalendarStore::COLOR_SUPPORT, support)
                .filter_eq(CalendarStore::ID, store.id)
                .returning_one()
                .found()?;
            return Ok(calendars_changed(store));
        }
        let store = ctx
            .db
            .insert::<CalendarStore>()
            .set(CalendarStore::TITLE, title)
            .set(
                CalendarStore::COLOR_SUPPORT,
                self.color_support.unwrap_or_default(),
            )
            .set_group(CalendarStoreBackend::Account {
                account_id: self.account_id,
            })
            .returning_one()
            .found()?;
        Ok(calendars_changed(store))
    }
}

/// Names a store, or gives it its backend's name back.
#[derive(FromRequest)]
#[request(RenameCalendarStoreRequest)]
pub struct RenameCalendarStoreCommand {
    pub store_id: i64,
    pub title: Option<String>,
}
impl DatabaseCommand for RenameCalendarStoreCommand {
    type Response = CalendarStore;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<CalendarStore>> {
        let store = store(ctx.db.read(), self.store_id)?;
        let (title, named) = match self.title {
            Some(title) => {
                ensure_title(&title)?;
                (title, true)
            }
            None => (backend_name(ctx.db.read(), &store)?, false),
        };
        let store = ctx
            .db
            .update::<CalendarStore>()
            .set(CalendarStore::TITLE, title)
            .set(CalendarStore::NAMED_BY_USER, named)
            .filter_eq(CalendarStore::ID, store.id)
            .returning_one()
            .found()?;
        Ok(calendars_changed(store))
    }
}

/// The name `store`'s backend gives it: `"Cohesion"` for a store only in
/// Cohesion, its connector account's description for a backend's.
fn backend_name<'c>(
    db: impl ReadableDatabase<'c>,
    store: &CalendarStore,
) -> DatabaseCommandResult<String> {
    match store.backend {
        CalendarStoreBackend::Local => Ok(LOCAL_STORE_TITLE.to_string()),
        CalendarStoreBackend::Account { account_id } => {
            let account: ConnectorAccount = db
                .select()
                .filter_eq(ConnectorAccount::ID, account_id)
                .one()
                .found()?;
            Ok(backend_title(account.description))
        }
    }
}

/// Records how store `store_id`'s sync ended: `failure` says why it failed,
/// or is `None` when it synced in full.
pub struct RecordCalendarSyncCommand {
    pub store_id: i64,
    pub failure: Option<String>,
}
impl DatabaseCommand for RecordCalendarSyncCommand {
    type Response = CalendarStore;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<CalendarStore>> {
        let update = ctx
            .db
            .update::<CalendarStore>()
            .filter_eq(CalendarStore::ID, self.store_id);
        let update = match self.failure {
            Some(failure) => update.set(CalendarStore::SYNC_FAILURE, Some(failure)),
            None => update
                .set_null(CalendarStore::SYNC_FAILURE)
                .set_now(CalendarStore::LAST_SYNCED_AT),
        };
        Ok(calendars_changed(update.returning_one().found()?))
    }
}

/// Every live calendar, by store, then title ignoring case.
#[derive(FromRequest)]
#[request(ListCalendarsRequest)]
pub struct ListCalendarsCommand {}
impl DatabaseCommand for ListCalendarsCommand {
    type Response = Vec<Calendar>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx
            .db
            .select_live()
            .order("store_id, title COLLATE NOCASE, id")
            .all()?)
    }
}

/// Creates a calendar in a local store.
#[derive(FromRequest)]
#[request(CreateCalendarRequest)]
pub struct CreateCalendarCommand {
    pub store_id: i64,
    pub title: String,
    pub color: CalendarColorChoice,
    pub workspace_id: Option<i64>,
}
impl DatabaseCommand for CreateCalendarCommand {
    type Response = Calendar;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Calendar>> {
        ensure_title(&self.title)?;
        let color = chosen_color(&self.color)?;
        ensure_local(&store(ctx.db.read(), self.store_id)?)?;
        if let Some(workspace_id) = self.workspace_id {
            ensure_workspace(ctx.db.read(), workspace_id)?;
        }
        let calendar = ctx
            .db
            .insert::<Calendar>()
            .set(Calendar::STORE_ID, self.store_id)
            .set(Calendar::TITLE, self.title)
            .set(Calendar::SELECTED_COLOR, color)
            .returning_one()
            .found()?;
        let perspective = match self.workspace_id {
            Some(workspace_id) => perspective_row(ctx.db.read(), workspace_id)?,
            None => None,
        };
        let Some(perspective) = perspective else {
            return Ok(calendars_changed(calendar));
        };
        ctx.db
            .insert::<CalendarPerspectiveCalendar>()
            .set(CalendarPerspectiveCalendar::PERSPECTIVE_ID, perspective.id)
            .set(CalendarPerspectiveCalendar::CALENDAR_ID, calendar.id)
            .execute()?;
        Ok(calendar.invalidating([
            Invalidation::Calendars,
            Invalidation::CalendarPerspective(perspective.workspace_id),
        ]))
    }
}

/// Retitles a live calendar of a local store.
#[derive(FromRequest)]
#[request(RenameCalendarRequest)]
pub struct RenameCalendarCommand {
    pub calendar_id: i64,
    pub title: String,
}
impl DatabaseCommand for RenameCalendarCommand {
    type Response = Calendar;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Calendar>> {
        ensure_title(&self.title)?;
        let (calendar, store) = calendar_in_store(ctx.db.read(), self.calendar_id)?;
        ensure_local(&store)?;
        let calendar = ctx
            .db
            .update::<Calendar>()
            .set(Calendar::TITLE, self.title)
            .set_now(Calendar::MODIFIED)
            .filter_eq(Calendar::ID, calendar.id)
            .returning_one()
            .found()?;
        Ok(calendars_changed(calendar))
    }
}

/// Sets a live calendar's selected color, in any store; on a backend that
/// keeps any color, it waits to be pushed.
#[derive(FromRequest)]
#[request(SetCalendarColorRequest)]
pub struct SetCalendarColorCommand {
    pub calendar_id: i64,
    pub color: CalendarColorChoice,
}
impl DatabaseCommand for SetCalendarColorCommand {
    type Response = Calendar;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Calendar>> {
        let color = chosen_color(&self.color)?;
        let (calendar, store) = calendar_in_store(ctx.db.read(), self.calendar_id)?;
        let pushed = store.color_support == CalendarColorSupport::Exact
            && calendar.remote_identifier.is_some()
            && calendar.declared_color != Some(color);
        let calendar = ctx
            .db
            .update::<Calendar>()
            .set(Calendar::SELECTED_COLOR, color)
            .set(Calendar::COLOR_PENDING_PUSH, pushed)
            .set_now(Calendar::MODIFIED)
            .filter_eq(Calendar::ID, calendar.id)
            .returning_one()
            .found()?;
        Ok(calendars_changed(calendar))
    }
}

/// Deletes a live calendar of a local store, and its events with it
/// (trigger `calendars_deleted_delete_events`).
#[derive(FromRequest)]
#[request(DeleteCalendarRequest)]
pub struct DeleteCalendarCommand {
    pub calendar_id: i64,
}
impl DatabaseCommand for DeleteCalendarCommand {
    type Response = Calendar;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Calendar>> {
        let (calendar, store) = calendar_in_store(ctx.db.read(), self.calendar_id)?;
        ensure_local(&store)?;
        let calendar = ctx
            .db
            .update::<Calendar>()
            .set_true(Calendar::DELETED)
            .filter_eq(Calendar::ID, calendar.id)
            .returning_one()
            .found()?;
        Ok(calendar.invalidating([Invalidation::Calendars, Invalidation::CalendarEvents]))
    }
}

/// Brings store `store_id`'s calendars in line with what its backend's
/// `listCalendars` returned: adds the new ones in their declared color (or
/// blue), retitles the rest and takes their declared color, which their
/// selected one follows when it changed, and deletes those it no longer
/// lists. A declared color that changed wins over a chosen one waiting to be
/// pushed. Only the sync calls it. Returns the store's live calendars.
pub struct SyncBackendCalendarsCommand {
    pub store_id: i64,
    pub calendars: Vec<BackendCalendar>,
}
impl DatabaseCommand for SyncBackendCalendarsCommand {
    type Response = Vec<Calendar>;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Vec<Calendar>>> {
        let support = store(ctx.db.read(), self.store_id)?.color_support;
        let known: Vec<Calendar> = ctx
            .db
            .select_live()
            .filter_eq(Calendar::STORE_ID, self.store_id)
            .all()?;
        let mut changed = false;
        for remote in &self.calendars {
            let title = backend_title(remote.title.clone());
            let declared = support.declared(remote.color.as_deref());
            match known
                .iter()
                .find(|c| c.remote_identifier.as_deref() == Some(&remote.identifier))
            {
                Some(calendar)
                    if calendar.title == title
                        && calendar.read_only == remote.read_only
                        && calendar.declared_color == declared => {}
                Some(calendar) => {
                    let recolored = declared.is_some() && calendar.declared_color != declared;
                    ctx.db
                        .update::<Calendar>()
                        .set(Calendar::TITLE, title)
                        .set(Calendar::READ_ONLY, remote.read_only)
                        .set(Calendar::DECLARED_COLOR, declared)
                        .set(
                            Calendar::SELECTED_COLOR,
                            calendar_selected(
                                calendar.selected_color,
                                calendar.declared_color,
                                declared,
                            ),
                        )
                        .set(
                            Calendar::COLOR_PENDING_PUSH,
                            calendar.color_pending_push && !recolored,
                        )
                        .set_now(Calendar::MODIFIED)
                        .filter_eq(Calendar::ID, calendar.id)
                        .execute()?;
                    changed = true;
                }
                None => {
                    // A calendar the backend dropped and listed again comes
                    // back as it was deleted: its row is replaced.
                    ctx.db
                        .delete::<Calendar>()
                        .filter_eq(Calendar::STORE_ID, self.store_id)
                        .filter_eq(Calendar::REMOTE_IDENTIFIER, Some(remote.identifier.clone()))
                        .execute()?;
                    ctx.db
                        .insert::<Calendar>()
                        .set(Calendar::STORE_ID, self.store_id)
                        .set(Calendar::TITLE, title)
                        .set(
                            Calendar::SELECTED_COLOR,
                            declared.unwrap_or(IntelligenceAgentColor::Blue),
                        )
                        .set(Calendar::DECLARED_COLOR, declared)
                        .set(Calendar::REMOTE_IDENTIFIER, Some(remote.identifier.clone()))
                        .set(Calendar::READ_ONLY, remote.read_only)
                        .execute()?;
                    changed = true;
                }
            }
        }
        for calendar in &known {
            let listed = self
                .calendars
                .iter()
                .any(|remote| calendar.remote_identifier.as_deref() == Some(&remote.identifier));
            if !listed {
                ctx.db
                    .update::<Calendar>()
                    .set_true(Calendar::DELETED)
                    .filter_eq(Calendar::ID, calendar.id)
                    .execute()?;
                changed = true;
            }
        }
        let calendars = ctx
            .db
            .select_live()
            .filter_eq(Calendar::STORE_ID, self.store_id)
            .order("title COLLATE NOCASE, id")
            .all()?;
        Ok(if changed {
            calendars.invalidating([Invalidation::Calendars, Invalidation::CalendarEvents])
        } else {
            calendars.invalidating_nothing()
        })
    }
}

/// Records that calendar `calendar_id`'s selected color `pushed` reached its
/// backend, which now declares `declared`: it takes that declared color,
/// which its selected one follows when it changed, and no longer waits.
/// Nothing is written when its selected color is no longer the one pushed:
/// it was chosen again meanwhile, and waits for the next sync. Only the sync
/// calls it.
pub struct RecordCalendarColorPushedCommand {
    pub calendar_id: i64,
    pub pushed: IntelligenceAgentColor,
    pub declared: Option<IntelligenceAgentColor>,
}
impl DatabaseCommand for RecordCalendarColorPushedCommand {
    type Response = Option<Calendar>;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Option<Calendar>>> {
        let calendar: Option<Calendar> = ctx
            .db
            .select_live()
            .filter_eq(Calendar::ID, self.calendar_id)
            .filter_eq(Calendar::SELECTED_COLOR, self.pushed)
            .one()?;
        let Some(calendar) = calendar else {
            return Ok(None.invalidating_nothing());
        };
        let calendar = ctx
            .db
            .update::<Calendar>()
            .set(Calendar::DECLARED_COLOR, self.declared)
            .set(
                Calendar::SELECTED_COLOR,
                calendar_selected(self.pushed, calendar.declared_color, self.declared),
            )
            .set_false(Calendar::COLOR_PENDING_PUSH)
            .filter_eq(Calendar::ID, calendar.id)
            .returning_one()?;
        Ok(calendars_changed(calendar))
    }
}

/// Which calendars workspace `workspace_id` shows, as it chose them.
fn perspective_row<'c>(
    db: impl ReadableDatabase<'c>,
    workspace_id: i64,
) -> DatabaseCommandResult<Option<CalendarPerspectiveRow>> {
    Ok(db
        .select()
        .filter_eq(CalendarPerspectiveRow::WORKSPACE_ID, workspace_id)
        .one()?)
}

/// Workspace `workspace_id`'s perspective: the live calendars it chose, by
/// id, or `None` when it hasn't.
fn perspective<'c>(
    db: impl ReadableDatabase<'c>,
    workspace_id: i64,
) -> DatabaseCommandResult<CalendarPerspective> {
    let calendar_ids = match perspective_row(db, workspace_id)? {
        None => None,
        Some(row) => Some(
            db.select_column::<CalendarPerspectiveCalendar, i64>("calendar_id")
                .filter("perspective_id = ?")
                .bind(row.id)
                .filter("calendar_id IN (SELECT id FROM calendars WHERE NOT deleted)")
                .order("calendar_id")
                .all()?,
        ),
    };
    Ok(CalendarPerspective {
        workspace_id,
        calendar_ids,
    })
}

/// Which calendars a workspace shows.
#[derive(FromRequest)]
#[request(GetCalendarPerspectiveRequest)]
pub struct GetCalendarPerspectiveCommand {
    pub workspace_id: i64,
}
impl DatabaseCommand for GetCalendarPerspectiveCommand {
    type Response = CalendarPerspective;
    type Access = Read;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Read>,
    ) -> DatabaseCommandResult<CalendarPerspective> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        perspective(ctx.db, self.workspace_id)
    }
}

/// Sets which calendars a workspace shows, or forgets its choice.
#[derive(FromRequest)]
#[request(SetCalendarPerspectiveRequest)]
pub struct SetCalendarPerspectiveCommand {
    pub workspace_id: i64,
    pub calendar_ids: Option<Vec<i64>>,
}
impl DatabaseCommand for SetCalendarPerspectiveCommand {
    type Response = CalendarPerspective;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<CalendarPerspective>> {
        ensure_workspace(ctx.db.read(), self.workspace_id)?;
        let mut calendar_ids = self.calendar_ids;
        if let Some(ids) = calendar_ids.as_mut() {
            ids.sort_unstable();
            ids.dedup();
            let live = ctx
                .db
                .select_live::<Calendar>()
                .filter_in_values("id", ids.clone())
                .count()?;
            if usize::try_from(live).ok() != Some(ids.len()) {
                return Err(ValidationError::in_field(
                    "calendar_ids",
                    "A calendar in the perspective doesn't exist.".to_string(),
                )
                .into());
            }
        }
        ctx.db
            .delete::<CalendarPerspectiveRow>()
            .filter_eq(CalendarPerspectiveRow::WORKSPACE_ID, self.workspace_id)
            .execute()?;
        if let Some(ids) = calendar_ids {
            let row = ctx
                .db
                .insert::<CalendarPerspectiveRow>()
                .set(CalendarPerspectiveRow::WORKSPACE_ID, self.workspace_id)
                .returning_one()
                .found()?;
            for calendar_id in ids {
                ctx.db
                    .insert::<CalendarPerspectiveCalendar>()
                    .set(CalendarPerspectiveCalendar::PERSPECTIVE_ID, row.id)
                    .set(CalendarPerspectiveCalendar::CALENDAR_ID, calendar_id)
                    .execute()?;
            }
        }
        let perspective = perspective(ctx.db.read(), self.workspace_id)?;
        Ok(perspective.invalidating([Invalidation::CalendarPerspective(self.workspace_id)]))
    }
}

/// The SQL an event overlaps a range by: a timed one by its instants, an
/// all-day one by the UTC days the range touches. Bound by [`bind_overlap`].
const OVERLAPS_SQL: &str = "((calendar_events.starts_at < ? AND calendar_events.ends_at > ?)
    OR (calendar_events.start_date <= date(?, 'unixepoch')
        AND calendar_events.end_date >= date(?, 'unixepoch')))";

/// Live events overlapping a range, on some calendars or all, by when they
/// start.
#[derive(FromRequest)]
#[request(ListCalendarEventsRequest)]
pub struct ListCalendarEventsCommand {
    pub calendar_ids: Option<Vec<i64>>,
    pub from: i64,
    pub to: i64,
}
impl DatabaseCommand for ListCalendarEventsCommand {
    type Response = Vec<CalendarEvent>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        let query = ctx.db.select_live::<CalendarEvent>().filter(OVERLAPS_SQL);
        let mut query = bind_overlap(query, self.from, self.to);
        if let Some(ids) = self.calendar_ids {
            query = query.filter_in_values("calendar_id", ids);
        }
        Ok(query
            .order("COALESCE(starts_at, unixepoch(start_date)), id")
            .all()?)
    }
}

/// Binds [`OVERLAPS_SQL`]'s placeholders to the range `from` to `to`, in
/// Unix seconds.
fn bind_overlap<Q: Filtered>(query: Q, from: i64, to: i64) -> Q {
    query.bind(to).bind(from).bind(to).bind(from)
}

/// A live event by id.
#[derive(FromRequest)]
#[request(GetCalendarEventRequest)]
pub struct GetCalendarEventCommand {
    pub event_id: i64,
}
impl DatabaseCommand for GetCalendarEventCommand {
    type Response = CalendarEvent;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<CalendarEvent> {
        ctx.db
            .select_live()
            .filter_eq(CalendarEvent::ID, self.event_id)
            .one()
            .found()
    }
}

/// Creates an event on a live calendar; on a backend's, it waits to be
/// pushed.
#[derive(FromRequest)]
#[request(CreateCalendarEventRequest)]
pub struct CreateCalendarEventCommand {
    pub calendar_id: i64,
    pub event: CalendarEventFields,
}
impl DatabaseCommand for CreateCalendarEventCommand {
    type Response = CalendarEvent;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<CalendarEvent>> {
        let fields = checked_fields(self.event)?;
        let (calendar, backed) = writable_calendar(ctx.db.read(), self.calendar_id)?;
        let event = ctx
            .db
            .insert::<CalendarEvent>()
            .set(CalendarEvent::CALENDAR_ID, calendar.id)
            .set(CalendarEvent::TITLE, fields.title)
            .set(CalendarEvent::LOCATION, fields.location)
            .set(CalendarEvent::DESCRIPTION, fields.description)
            .set_group(fields.timing)
            .set(CalendarEvent::INVITEES, Json(fields.invitees))
            .set(CalendarEvent::SELECTED_COLOR, fields.selected_color)
            .set(CalendarEvent::PENDING_PUSH, backed)
            .returning_one()
            .found()?;
        Ok(events_changed(event))
    }
}

/// An event's fields, checked, with its color made a palette color.
struct CheckedFields {
    title: String,
    location: String,
    description: String,
    timing: EventTiming,
    invitees: Vec<EventInvitee>,
    selected_color: Option<IntelligenceAgentColor>,
}

/// Checks `fields` as a client or script gave them; the schema checks the
/// timing.
fn checked_fields(fields: CalendarEventFields) -> Result<CheckedFields, ValidationError> {
    ensure_title(&fields.title)?;
    ensure_invitees(&fields.invitees)?;
    let selected_color = fields
        .selected_color
        .as_ref()
        .map(chosen_color)
        .transpose()?;
    Ok(CheckedFields {
        title: fields.title,
        location: fields.location,
        description: fields.description,
        timing: fields.timing,
        invitees: fields.invitees,
        selected_color,
    })
}

/// Live event `event_id`, on a calendar whose events may change, and
/// whether a change to it is pushed to a backend.
fn writable_event<'c>(
    db: impl ReadableDatabase<'c>,
    event_id: i64,
) -> DatabaseCommandResult<(CalendarEvent, bool)> {
    let event: CalendarEvent = db
        .select_live()
        .filter_eq(CalendarEvent::ID, event_id)
        .one()
        .found()?;
    let (_, backed) = writable_calendar(db, event.calendar_id)?;
    Ok((event, backed))
}

/// Replaces what a live event says; on a backend's calendar, it waits to be
/// pushed.
#[derive(FromRequest)]
#[request(UpdateCalendarEventRequest)]
pub struct UpdateCalendarEventCommand {
    pub event_id: i64,
    pub event: CalendarEventFields,
}
impl DatabaseCommand for UpdateCalendarEventCommand {
    type Response = CalendarEvent;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<CalendarEvent>> {
        let fields = checked_fields(self.event)?;
        let (event, backed) = writable_event(ctx.db.read(), self.event_id)?;
        let event = ctx
            .db
            .update::<CalendarEvent>()
            .set(CalendarEvent::TITLE, fields.title)
            .set(CalendarEvent::LOCATION, fields.location)
            .set(CalendarEvent::DESCRIPTION, fields.description)
            .set_group(fields.timing)
            .set(CalendarEvent::INVITEES, Json(fields.invitees))
            .set(CalendarEvent::SELECTED_COLOR, fields.selected_color)
            .set(CalendarEvent::PENDING_PUSH, backed)
            .increment(CalendarEvent::REVISION)
            .set_now(CalendarEvent::MODIFIED)
            .filter_eq(CalendarEvent::ID, event.id)
            .returning_one()
            .found()?;
        Ok(events_changed(event))
    }
}

/// Deletes a live event. One a backend already has waits, deleted, for its
/// deletion to be pushed.
#[derive(FromRequest)]
#[request(DeleteCalendarEventRequest)]
pub struct DeleteCalendarEventCommand {
    pub event_id: i64,
}
impl DatabaseCommand for DeleteCalendarEventCommand {
    type Response = CalendarEvent;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<CalendarEvent>> {
        let (event, backed) = writable_event(ctx.db.read(), self.event_id)?;
        let event = ctx
            .db
            .update::<CalendarEvent>()
            .set_true(CalendarEvent::DELETED)
            .set(
                CalendarEvent::PENDING_PUSH,
                backed && event.remote_identifier.is_some(),
            )
            .increment(CalendarEvent::REVISION)
            .set_now(CalendarEvent::MODIFIED)
            .filter_eq(CalendarEvent::ID, event.id)
            .returning_one()
            .found()?;
        Ok(events_changed(event))
    }
}

/// Calendar `calendar_id`'s events as a sync sees them: the live ones, and
/// the deleted ones waiting to push their deletion.
pub struct ListEventsToSyncCommand {
    pub calendar_id: i64,
}
impl DatabaseCommand for ListEventsToSyncCommand {
    type Response = Vec<CalendarEvent>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx
            .db
            .select()
            .filter_eq(CalendarEvent::CALENDAR_ID, self.calendar_id)
            .filter("(NOT deleted OR pending_push)")
            .order("id")
            .all()?)
    }
}

/// Writes a backend's copy of an event into calendar `calendar_id`: over
/// event `event_id`, provided it is still as it was when the sync read it
/// (its `revision` is the one read), or as a new event. Its `modified`
/// becomes the backend's `lastModified`, and it no longer waits to be
/// pushed. Only the sync calls it. `None` when the event changed meanwhile.
pub struct ApplyBackendEventCommand {
    pub calendar_id: i64,
    pub existing: Option<SyncedEvent>,
    pub remote: BackendEvent,
    /// What colors the backend keeps, which says what color it declares.
    pub color_support: CalendarColorSupport,
}

/// An event as a sync read it: its id, `revision` and colors then.
#[derive(Clone, Copy, Debug)]
pub struct SyncedEvent {
    pub event_id: i64,
    pub revision: i64,
    pub selected_color: Option<IntelligenceAgentColor>,
    pub declared_color: Option<IntelligenceAgentColor>,
}

impl From<&CalendarEvent> for SyncedEvent {
    fn from(event: &CalendarEvent) -> Self {
        Self {
            event_id: event.id,
            revision: event.revision,
            selected_color: event.selected_color,
            declared_color: event.declared_color,
        }
    }
}

impl DatabaseCommand for ApplyBackendEventCommand {
    type Response = Option<CalendarEvent>;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Option<CalendarEvent>>> {
        let remote = self.remote;
        let declared = self.color_support.declared(remote.color.as_deref());
        let invitees: Vec<EventInvitee> = remote.invitees.into_iter().map(Into::into).collect();
        let timing: EventTiming = remote.timing.into();
        let event = match self.existing {
            Some(existing) => ctx
                .db
                .update::<CalendarEvent>()
                .set(CalendarEvent::TITLE, backend_title(remote.title))
                .set(CalendarEvent::LOCATION, remote.location)
                .set(CalendarEvent::DESCRIPTION, remote.description)
                .set_group(timing)
                .set(CalendarEvent::INVITEES, Json(invitees))
                .set(CalendarEvent::DECLARED_COLOR, declared)
                .set(
                    CalendarEvent::SELECTED_COLOR,
                    event_selected(existing.selected_color, existing.declared_color, declared),
                )
                .set(CalendarEvent::REMOTE_IDENTIFIER, Some(remote.identifier))
                .set(CalendarEvent::REMOTE_MODIFIED, Some(remote.last_modified))
                .set_false(CalendarEvent::DELETED)
                .set_false(CalendarEvent::PENDING_PUSH)
                .set(CalendarEvent::MODIFIED, remote.last_modified)
                .increment(CalendarEvent::REVISION)
                .filter_eq(CalendarEvent::ID, existing.event_id)
                .filter_eq(CalendarEvent::REVISION, existing.revision)
                .returning_one()?,
            None => ctx
                .db
                .insert::<CalendarEvent>()
                .set(CalendarEvent::CALENDAR_ID, self.calendar_id)
                .set(CalendarEvent::TITLE, backend_title(remote.title))
                .set(CalendarEvent::LOCATION, remote.location)
                .set(CalendarEvent::DESCRIPTION, remote.description)
                .set_group(timing)
                .set(CalendarEvent::INVITEES, Json(invitees))
                .set(CalendarEvent::DECLARED_COLOR, declared)
                .set(CalendarEvent::SELECTED_COLOR, declared)
                .set(CalendarEvent::REMOTE_IDENTIFIER, Some(remote.identifier))
                .set(CalendarEvent::REMOTE_MODIFIED, Some(remote.last_modified))
                .set(CalendarEvent::MODIFIED, remote.last_modified)
                .returning_one()?,
        };
        Ok(events_changed(event))
    }
}

/// Records that event `event_id`'s change reached its backend as `remote`,
/// provided it hasn't changed since the sync read it: it takes the
/// backend's identifier, `lastModified` and declared color `declared`, which
/// its selected one follows when it changed, and no longer waits. Only the
/// sync calls it. `None` when the event changed meanwhile.
pub struct RecordEventPushedCommand {
    pub event: SyncedEvent,
    pub remote_identifier: String,
    pub remote_modified: i64,
    pub declared: Option<IntelligenceAgentColor>,
}
impl DatabaseCommand for RecordEventPushedCommand {
    type Response = Option<CalendarEvent>;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Option<CalendarEvent>>> {
        let event = ctx
            .db
            .update::<CalendarEvent>()
            .set(
                CalendarEvent::REMOTE_IDENTIFIER,
                Some(self.remote_identifier),
            )
            .set(CalendarEvent::REMOTE_MODIFIED, Some(self.remote_modified))
            .set(CalendarEvent::DECLARED_COLOR, self.declared)
            .set(
                CalendarEvent::SELECTED_COLOR,
                event_selected(
                    self.event.selected_color,
                    self.event.declared_color,
                    self.declared,
                ),
            )
            .set_false(CalendarEvent::PENDING_PUSH)
            .filter_eq(CalendarEvent::ID, self.event.event_id)
            .filter_eq(CalendarEvent::REVISION, self.event.revision)
            .returning_one()?;
        // Only its colors are something a client shows.
        Ok(if self.event.declared_color != self.declared {
            events_changed(event)
        } else {
            event.invalidating_nothing()
        })
    }
}

/// Removes event `event_id` for good, provided it hasn't changed since the
/// sync read it: its deletion reached the backend, or the backend deleted
/// it. Only the sync calls it. Whether it was removed.
pub struct ForgetCalendarEventCommand {
    pub event: SyncedEvent,
}
impl DatabaseCommand for ForgetCalendarEventCommand {
    type Response = bool;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<bool>> {
        let removed = ctx
            .db
            .delete::<CalendarEvent>()
            .filter_eq(CalendarEvent::ID, self.event.event_id)
            .filter_eq(CalendarEvent::REVISION, self.event.revision)
            .execute()?
            > 0;
        Ok(if removed {
            events_changed(removed)
        } else {
            removed.invalidating_nothing()
        })
    }
}

#[cfg(test)]
mod tests;
