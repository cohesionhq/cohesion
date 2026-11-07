use cohesion_macros::{Fields, Model, TsWire};
use serde::{Deserialize, Serialize};

use super::CatalogModel;

/// A model an intelligence endpoint offers, one of the models every picker
/// lists. A row the models.dev catalog lists links its catalog row and reads
/// through that link whatever it does not set itself, so catalog prices
/// never drift; a value set on the row overrides the catalog's. Every read
/// joins the catalog, so the fields here are the model's either way. Prices
/// are USD per million tokens; a model without input and output prices
/// records no cost.
#[derive(Clone, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "endpoint_models", backup = Included)]
#[model(left_join = CatalogModel, on = "catalog_models.id = endpoint_models.catalog_model_id")]
pub struct EndpointModel {
    /// Unique, stable identifier.
    pub id: i64,
    /// The endpoint that serves the model.
    pub endpoint_id: i64,
    /// The model's identifier on that endpoint, such as `claude-opus-5-5`.
    pub model_identifier: String,
    /// The model's display name, such as `Claude Opus 5.5`.
    #[model(coalesce(Self::model_name, CatalogModel::model_name, Self::model_identifier))]
    pub model_name: String,
    /// The catalog row the model was imported from, if any.
    #[fields(skip)]
    pub catalog_model_id: Option<i64>,
    /// How many tokens the model's context window holds.
    #[model(or_from = CatalogModel)]
    pub context_size: Option<i64>,
    #[model(or_from = CatalogModel)]
    pub input_token_cost: Option<f64>,
    #[model(or_from = CatalogModel)]
    pub output_token_cost: Option<f64>,
    #[model(or_from = CatalogModel)]
    pub cache_read_token_cost: Option<f64>,
    #[model(or_from = CatalogModel)]
    pub cache_write_token_cost: Option<f64>,
    /// Past this many prompt tokens a request is billed at the large-context
    /// prices.
    #[model(or_from = CatalogModel)]
    pub large_context_threshold: Option<i64>,
    #[model(or_from = CatalogModel)]
    pub large_context_input_token_cost: Option<f64>,
    #[model(or_from = CatalogModel)]
    pub large_context_output_token_cost: Option<f64>,
    #[model(or_from = CatalogModel)]
    pub large_context_cache_read_token_cost: Option<f64>,
    #[model(or_from = CatalogModel)]
    pub large_context_cache_write_token_cost: Option<f64>,
}
