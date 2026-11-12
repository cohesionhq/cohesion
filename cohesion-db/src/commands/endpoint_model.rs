use cohesion_core::{
    FromRequest,
    invalidation::Invalidation,
    model::{CatalogModel, EndpointBackend, EndpointModel, IntelligenceEndpoint},
    protocol::{
        AddEndpointModelRequest, AddEndpointModelsRequest, ListEndpointModelsRequest, ModelPrices,
        RemoveEndpointModelRequest,
    },
};

use crate::changed::{Changed, Invalidates};
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, Filtered, Found,
    GetIntelligenceEndpointCommand, IntelligenceSettings, Read, ReadableDatabase, Select, Write,
};

/// Endpoint models as one flat row each: a field the row sets itself wins,
/// and one it leaves empty is read through its catalog link. Filter on
/// `endpoint_models.` columns, which the catalog join shares names with.
fn endpoint_models<'c>(db: impl ReadableDatabase<'c>) -> Select<'c, EndpointModel, EndpointModel> {
    db.select::<EndpointModel>()
        .order("model_name, endpoint_models.model_identifier")
}

/// The models.dev provider of `endpoint`.
fn provider_of(endpoint: IntelligenceEndpoint) -> String {
    match endpoint.backend {
        EndpointBackend::Integrated {
            provider_identifier,
            ..
        }
        | EndpointBackend::Account {
            provider_identifier,
            ..
        } => provider_identifier,
    }
}

/// Makes an endpoint offer a model, linked to the catalog row of the
/// endpoint's provider with this identifier when there is one. The request
/// is the whole model: its fields are stored on the row and override the
/// catalog's, and one left out clears its override. Adding a model the
/// endpoint already offers replaces it.
#[derive(FromRequest)]
#[request(AddEndpointModelRequest)]
pub struct AddEndpointModelCommand {
    pub endpoint_id: i64,
    pub model_identifier: String,
    pub model_name: Option<String>,
    pub context_size: Option<i64>,
    pub prices: Option<ModelPrices>,
}
impl DatabaseCommand for AddEndpointModelCommand {
    type Response = EndpointModel;
    type Access = Write<IntelligenceSettings>;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Self::Access>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let endpoint = GetIntelligenceEndpointCommand {
            id: self.endpoint_id,
        }
        .handle(&mut ctx.as_read())?;
        let provider_identifier = provider_of(endpoint);
        let catalog: Option<CatalogModel> = ctx
            .db
            .select()
            .filter_eq(CatalogModel::PROVIDER_IDENTIFIER, provider_identifier)
            .filter_eq(CatalogModel::MODEL_IDENTIFIER, &self.model_identifier)
            .one()?;

        let prices = self.prices.as_ref();
        ctx.db
            .insert::<EndpointModel>()
            .set(EndpointModel::ENDPOINT_ID, self.endpoint_id)
            .set(EndpointModel::MODEL_IDENTIFIER, &self.model_identifier)
            .set(
                EndpointModel::CATALOG_MODEL_ID,
                catalog.map(|catalog| catalog.id),
            )
            .set(EndpointModel::MODEL_NAME, self.model_name)
            .set(EndpointModel::CONTEXT_SIZE, self.context_size)
            .set(
                EndpointModel::INPUT_TOKEN_COST,
                prices.map(|p| p.input_token_cost),
            )
            .set(
                EndpointModel::OUTPUT_TOKEN_COST,
                prices.map(|p| p.output_token_cost),
            )
            .set(
                EndpointModel::CACHE_READ_TOKEN_COST,
                prices.and_then(|p| p.cache_read_token_cost),
            )
            .set(
                EndpointModel::CACHE_WRITE_TOKEN_COST,
                prices.and_then(|p| p.cache_write_token_cost),
            )
            .on_conflict_update(&["endpoint_id", "model_identifier"])
            .execute()?;
        Ok(endpoint_models(ctx.db)
            .filter_eq(EndpointModel::ENDPOINT_ID, self.endpoint_id)
            .filter_eq(EndpointModel::MODEL_IDENTIFIER, self.model_identifier)
            .one()
            .found()?
            .invalidating([Invalidation::Intelligence]))
    }
}

/// Imports each of `model_identifiers` into an endpoint, in order, as
/// [`AddEndpointModelCommand`] does with nothing overridden, in one
/// transaction: all of them or, if one fails, none.
#[derive(FromRequest)]
#[request(AddEndpointModelsRequest)]
pub struct AddEndpointModelsCommand {
    pub endpoint_id: i64,
    pub model_identifiers: Vec<String>,
}
impl DatabaseCommand for AddEndpointModelsCommand {
    type Response = Vec<EndpointModel>;
    type Access = Write<IntelligenceSettings>;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Self::Access>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let added = self
            .model_identifiers
            .into_iter()
            .map(|model_identifier| {
                ctx.run(AddEndpointModelCommand {
                    endpoint_id: self.endpoint_id,
                    model_identifier,
                    model_name: None,
                    context_size: None,
                    prices: None,
                })
            })
            .collect::<DatabaseCommandResult<Vec<_>>>()?;
        Ok(added.invalidating([Invalidation::Intelligence]))
    }
}

/// Removes an endpoint model; any tier using it is unset by the cascade.
#[derive(FromRequest)]
#[request(RemoveEndpointModelRequest)]
pub struct RemoveEndpointModelCommand {
    pub id: i64,
}
impl DatabaseCommand for RemoveEndpointModelCommand {
    type Response = EndpointModel;
    type Access = Write<IntelligenceSettings>;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Self::Access>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let model = GetEndpointModelCommand { id: self.id }.handle(&mut ctx.as_read())?;
        ctx.db
            .delete::<EndpointModel>()
            .filter_eq(EndpointModel::ID, self.id)
            .execute()?;
        Ok(model.invalidating([Invalidation::Intelligence]))
    }
}

/// The models endpoint `endpoint_id` offers, by name; every endpoint's
/// when `None`.
#[derive(FromRequest)]
#[request(ListEndpointModelsRequest)]
pub struct ListEndpointModelsCommand {
    pub endpoint_id: Option<i64>,
}
impl DatabaseCommand for ListEndpointModelsCommand {
    type Response = Vec<EndpointModel>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        let query = endpoint_models(ctx.db);
        Ok(match self.endpoint_id {
            Some(endpoint_id) => query.filter_eq(EndpointModel::ENDPOINT_ID, endpoint_id),
            None => query,
        }
        .all()?)
    }
}

pub struct GetEndpointModelCommand {
    pub id: i64,
}
impl DatabaseCommand for GetEndpointModelCommand {
    type Response = EndpointModel;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        endpoint_models(ctx.db)
            .filter_eq(EndpointModel::ID, self.id)
            .one()
            .found()
    }
}

#[cfg(test)]
mod tests;
