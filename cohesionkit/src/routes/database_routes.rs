//! Every database-backed request, run through its `DbRequest` command.

use cohesion_core::protocol::{Database, Route, ServerError};
use cohesion_db::DbRequest;

use crate::CohesionContext;

/// Every database-backed request: convert to its command and run it on the worker.
impl<R: DbRequest> Route<Database, R> for CohesionContext {
    async fn route(&self, request: R) -> Result<R::Reply, ServerError> {
        self.database
            .send(R::Command::from(request))
            .await
            .map_err(ServerError::from)
    }
}
