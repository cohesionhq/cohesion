use cohesion_macros::{Fields, Model, TsWire};
use serde::{Deserialize, Serialize};

/// An LLM offered by a provider, with its context size and per-token pricing.
#[derive(Clone, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "catalog_models", backup = Included)]
pub struct CatalogModel {
    #[fields(skip)]
    pub id: i64,
    /// The models.dev provider identifier, for example `openai`.
    pub provider_identifier: String,
    /// The provider's display name, for example `OpenAI`. Display only.
    pub provider_name: String,
    /// The model identifier, for example `gpt-5`.
    pub model_identifier: String,
    /// The model's display name, for example `GPT-5`. Display only.
    pub model_name: String,
    pub context_size: Option<i64>,
    pub input_token_cost: Option<f64>,
    pub output_token_cost: Option<f64>,
    pub cache_read_token_cost: Option<f64>,
    pub cache_write_token_cost: Option<f64>,
    pub large_context_threshold: Option<i64>,
    pub large_context_input_token_cost: Option<f64>,
    pub large_context_output_token_cost: Option<f64>,
    pub large_context_cache_read_token_cost: Option<f64>,
    pub large_context_cache_write_token_cost: Option<f64>,
}
