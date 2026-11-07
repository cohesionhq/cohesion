//! `EndpointModel` in scripts.

use cohesion_core::model::EndpointModel;
use cohesion_db::RemoveEndpointModelCommand;
use cohesion_macros::js_entity;
use rquickjs::Ctx;

/// A model an endpoint offers, with its context size and prices in USD per
/// million tokens where they are known.
#[js_entity(EndpointModel)]
impl JsEndpointModel {
    /// Removes the model from its endpoint; a tier using it is left without
    /// a model. Needs the `intelligence` permission.
    pub fn remove<'js>(&mut self, ctx: Ctx<'js>) -> rquickjs::Result<()> {
        self.inner = self.write(&ctx, RemoveEndpointModelCommand { id: self.inner.id })?;
        Ok(())
    }
}
