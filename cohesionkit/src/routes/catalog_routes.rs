//! Refreshing the model catalog.

use cohesion_core::protocol::{
    AcceptedResponse, Intelligence, Route, ServerError, UpdateModelCatalogRequest,
};

use crate::CohesionContext;

impl Route<Intelligence, UpdateModelCatalogRequest> for CohesionContext {
    async fn route(
        &self,
        _request: UpdateModelCatalogRequest,
    ) -> Result<<UpdateModelCatalogRequest as cohesion_core::protocol::Call>::Reply, ServerError>
    {
        self.refresh_catalog().await?;
        Ok(AcceptedResponse {})
    }
}
