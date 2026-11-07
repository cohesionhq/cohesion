use cohesion_core::{
    FromRequest, Projection,
    invalidation::Invalidation,
    model::{CatalogModel, EndpointModel, IntelligenceEndpoint},
    protocol::ListCatalogModelsRequest,
};

use crate::changed::{Changed, Invalidates};
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, Filtered, Read, ReadableDatabase, Write,
};

/// An endpoint model the catalog now lists, with the catalog row to link it
/// to: the one of its endpoint's provider with its identifier.
#[derive(Projection)]
#[projection(of = EndpointModel)]
#[projection(
    join = IntelligenceEndpoint,
    on = "intelligence_endpoints.id = endpoint_models.endpoint_id"
)]
#[projection(
    join = CatalogModel,
    on = "catalog_models.provider_identifier = intelligence_endpoints.provider_identifier \
        AND catalog_models.model_identifier = endpoint_models.model_identifier"
)]
struct CatalogLink {
    id: i64,
    #[projection(from = CatalogModel::id)]
    catalog_id: i64,
}

/// Upserts each model by provider identifier and model identifier: overwrites
/// the row if it exists, inserts it otherwise. Then links each endpoint model
/// added before the catalog listed it to its provider's catalog row, which
/// fills in whatever the model does not set itself. It never writes an
/// endpoint model's own fields.
pub struct UpdateCatalogModelsCommand {
    pub models: Vec<CatalogModel>,
}
impl DatabaseCommand for UpdateCatalogModelsCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        for model in self.models {
            ctx.db
                .insert::<CatalogModel>()
                .set(CatalogModel::PROVIDER_IDENTIFIER, model.provider_identifier)
                .set(CatalogModel::PROVIDER_NAME, model.provider_name)
                .set(CatalogModel::MODEL_IDENTIFIER, model.model_identifier)
                .set(CatalogModel::MODEL_NAME, model.model_name)
                .set(CatalogModel::CONTEXT_SIZE, model.context_size)
                .set(CatalogModel::INPUT_TOKEN_COST, model.input_token_cost)
                .set(CatalogModel::OUTPUT_TOKEN_COST, model.output_token_cost)
                .set(
                    CatalogModel::CACHE_READ_TOKEN_COST,
                    model.cache_read_token_cost,
                )
                .set(
                    CatalogModel::CACHE_WRITE_TOKEN_COST,
                    model.cache_write_token_cost,
                )
                .set(
                    CatalogModel::LARGE_CONTEXT_THRESHOLD,
                    model.large_context_threshold,
                )
                .set(
                    CatalogModel::LARGE_CONTEXT_INPUT_TOKEN_COST,
                    model.large_context_input_token_cost,
                )
                .set(
                    CatalogModel::LARGE_CONTEXT_OUTPUT_TOKEN_COST,
                    model.large_context_output_token_cost,
                )
                .set(
                    CatalogModel::LARGE_CONTEXT_CACHE_READ_TOKEN_COST,
                    model.large_context_cache_read_token_cost,
                )
                .set(
                    CatalogModel::LARGE_CONTEXT_CACHE_WRITE_TOKEN_COST,
                    model.large_context_cache_write_token_cost,
                )
                .on_conflict_update(&["provider_identifier", "model_identifier"])
                .execute_silently()?;
        }
        let links = ctx
            .db
            .select_as::<CatalogLink>()
            .filter("endpoint_models.catalog_model_id IS NULL")
            .all()?;
        for link in &links {
            ctx.db
                .update::<EndpointModel>()
                .set(EndpointModel::CATALOG_MODEL_ID, link.catalog_id)
                .filter_eq(EndpointModel::ID, link.id)
                .execute()?;
        }
        Ok(().invalidating((!links.is_empty()).then_some(Invalidation::Intelligence)))
    }
}

/// Browses the catalog: one provider's models, or those of the providers of
/// configured endpoints, narrowed to identifiers or names containing `query`.
#[derive(FromRequest)]
#[request(ListCatalogModelsRequest)]
pub struct ListCatalogModelsCommand {
    pub provider_identifier: Option<String>,
    pub query: Option<String>,
}
impl DatabaseCommand for ListCatalogModelsCommand {
    type Response = Vec<CatalogModel>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        let select = match self.provider_identifier {
            Some(provider) => ctx
                .db
                .select()
                .filter("provider_identifier = ?")
                .bind(provider),
            None => ctx.db.select().filter_in(
                "provider_identifier",
                ctx.db
                    .select_column::<IntelligenceEndpoint, Option<String>>("provider_identifier"),
            ),
        };
        let select = match self.query {
            Some(query) => {
                let pattern = format!("%{query}%");
                select
                    .filter("(model_identifier LIKE ? OR model_name LIKE ?)")
                    .bind(&pattern)
                    .bind(&pattern)
            }
            None => select,
        };
        Ok(select
            .order("provider_identifier, model_identifier")
            .all()?)
    }
}
