use cohesion_core::protocol::{
    CalendarSyncSummary, Local, Route, ServerError, SyncCalendarsRequest,
};

use crate::CohesionContext;
use crate::calendars::sync_calendars;

impl Route<Local, SyncCalendarsRequest> for CohesionContext {
    async fn route(&self, _: SyncCalendarsRequest) -> Result<CalendarSyncSummary, ServerError> {
        Ok(sync_calendars(self).await)
    }
}
