//! Requests for the intelligence features: endpoints and their models, tier
//! defaults, the model catalog, threads and their costs, and spend limits.

use cohesion_macros::{Fields, TsWire};
use serde::{Deserialize, Serialize};

use crate::model::{
    Dialect, EndpointBackend, IntelligenceCostEvent, IntelligenceEndpoint,
    IntelligenceEndpointPreset, IntelligenceThread, IntelligenceThreadMessage, IntelligenceTier,
    LongCache, SpendPeriod,
};

#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct UpdateModelCatalogRequest;

/// Creates an intelligence endpoint from a preset, with the preset's default
/// model imported. Refused when the name is taken.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct CreateIntelligenceEndpointRequest {
    /// What the endpoint is called. Defaults to the provider's display name,
    /// or a custom endpoint's provider identifier.
    pub name: Option<String>,
    pub api_key: Option<String>,
    /// The page where a person sees what the endpoint has used. Defaults to
    /// the provider's usage page; a custom endpoint has none.
    pub usage_url: Option<String>,
    /// How many minutes the server keeps a prompt cached, at least 1.
    /// Defaults to the provider's; a custom endpoint's to 5.
    pub cache_minutes: Option<i64>,
    /// The longer cache an agent's requests ask for. Defaults to the
    /// provider's; a custom endpoint has none.
    pub long_cache: Option<LongCache>,
    #[serde(flatten)]
    pub preset: IntelligenceEndpointPreset,
}
/// An intelligence endpoint without its API key, safe to send to clients.
#[derive(Serialize, Deserialize, Debug, Clone, Fields, TsWire)]
pub struct RedactedIntelligenceEndpoint {
    pub id: i64,
    /// What the endpoint is called.
    pub name: String,
    /// What serves the endpoint.
    #[fields(skip)]
    pub backend: RedactedEndpointBackend,
}
/// What serves an endpoint, without its API key.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RedactedEndpointBackend {
    /// Cohesion's built-in Rust backends: a provider's API, or a server
    /// described by hand.
    Integrated {
        /// The models.dev provider, such as `"anthropic"`.
        provider_identifier: String,
        dialect: Dialect,
        base_url: String,
        /// The page where a person sees what the endpoint has used.
        usage_url: Option<String>,
        /// How many minutes the server keeps a prompt cached.
        cache_minutes: i64,
        /// The longer cache an agent's requests ask for, or none when it
        /// offers none.
        long_cache: Option<LongCache>,
    },
    /// A built-in backend signed in through an extension's account, such
    /// as a ChatGPT sign-in, which carries the account's credential.
    /// Removing the account removes the endpoint.
    Account {
        /// The models.dev provider, such as `"openai"`.
        provider_identifier: String,
        dialect: Dialect,
        base_url: String,
        /// The connector account the endpoint signs in with.
        account_id: i64,
        /// The page where a person sees what the endpoint has used.
        usage_url: Option<String>,
        /// How many minutes the server keeps a prompt cached.
        cache_minutes: i64,
        /// The longer cache an agent's requests ask for, or none when it
        /// offers none.
        long_cache: Option<LongCache>,
    },
}
impl From<EndpointBackend> for RedactedEndpointBackend {
    fn from(value: EndpointBackend) -> Self {
        match value {
            EndpointBackend::Integrated {
                provider_identifier,
                dialect,
                base_url,
                api_key: _,
                usage_url,
                cache_minutes,
                long_cache,
            } => Self::Integrated {
                provider_identifier,
                dialect,
                base_url,
                usage_url,
                cache_minutes,
                long_cache,
            },
            EndpointBackend::Account {
                provider_identifier,
                dialect,
                base_url,
                account_id,
                usage_url,
                cache_minutes,
                long_cache,
            } => Self::Account {
                provider_identifier,
                dialect,
                base_url,
                account_id,
                usage_url,
                cache_minutes,
                long_cache,
            },
        }
    }
}
impl From<IntelligenceEndpoint> for RedactedIntelligenceEndpoint {
    fn from(value: IntelligenceEndpoint) -> Self {
        Self {
            id: value.id,
            name: value.name,
            backend: value.backend.into(),
        }
    }
}

/// Lists every intelligence endpoint.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListIntelligenceEndpointsRequest;
/// Deletes intelligence endpoint `id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DeleteIntelligenceEndpointRequest {
    pub id: i64,
}

/// Sets where a person sees what endpoint `id` has used, or with no
/// `usage_url` removes it.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetIntelligenceEndpointUsageUrlRequest {
    pub id: i64,
    pub usage_url: Option<String>,
}

/// Sets how many minutes endpoint `id`'s server keeps a prompt cached, at
/// least 1.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetIntelligenceEndpointCacheMinutesRequest {
    pub id: i64,
    pub cache_minutes: i64,
}

/// Sets the longer prompt cache endpoint `id`'s server offers, which an
/// agent's requests ask for, or with no `long_cache` removes it.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetIntelligenceEndpointLongCacheRequest {
    pub id: i64,
    pub long_cache: Option<LongCache>,
}

/// Makes endpoint `endpoint_id` offer model `model_identifier`, linked to
/// the catalog when it lists the model for the endpoint's provider. The
/// request is the whole model: the fields given override the catalog's, and
/// one left out falls back to the catalog's (`model_name` then to the
/// identifier), clearing any override. Adding a model the endpoint already
/// offers replaces it.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct AddEndpointModelRequest {
    pub endpoint_id: i64,
    pub model_identifier: String,
    pub model_name: Option<String>,
    pub context_size: Option<i64>,
    pub prices: Option<ModelPrices>,
}
/// What a model costs, overriding the catalog, in USD per million tokens.
/// Cache tokens without their own price are billed as input.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct ModelPrices {
    pub input_token_cost: f64,
    pub output_token_cost: f64,
    pub cache_read_token_cost: Option<f64>,
    pub cache_write_token_cost: Option<f64>,
}

/// Removes endpoint model `id`, unsetting any tier that uses it.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct RemoveEndpointModelRequest {
    pub id: i64,
}

/// Lists the models endpoint `endpoint_id` offers, by name.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListEndpointModelsRequest {
    pub endpoint_id: i64,
}

/// Makes endpoint model `endpoint_model_id` the default for `tier`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetIntelligenceDefaultRequest {
    pub tier: IntelligenceTier,
    pub endpoint_model_id: i64,
}
/// Lists the default model of every tier that has one.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListIntelligenceDefaultsRequest;

/// Lists the catalog's models of provider `provider_identifier`, or of the
/// providers of configured endpoints when it is absent, whose identifier or
/// name contains `query`, if given.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct ListCatalogModelsRequest {
    pub provider_identifier: Option<String>,
    pub query: Option<String>,
}

/// Fetches intelligence thread `thread_id` with its messages.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct GetIntelligenceThreadRequest {
    pub thread_id: i64,
}
/// An intelligence thread and its messages, oldest first.
#[derive(Serialize, Deserialize, Clone, TsWire)]
pub struct IntelligenceThreadDetail {
    pub thread: IntelligenceThread,
    pub messages: Vec<IntelligenceThreadMessage>,
    /// What each model call in the thread cost, oldest first.
    pub cost_events: Vec<IntelligenceCostEvent>,
}

/// Lists the `limit` most recent intelligence cost events, newest first.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListIntelligenceCostEventsRequest {
    pub limit: u64,
}

/// Removes the default model of `tier`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ClearIntelligenceDefaultRequest {
    pub tier: IntelligenceTier,
}

/// Lists what every model call cost over each spend period, against its
/// limit, one entry per period, shortest first.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListIntelligenceSpendRequest;

/// Limits what every model call together may cost over `period` to
/// `amount` US dollars, more than zero, replacing its limit if it has one.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetIntelligenceSpendLimitRequest {
    pub period: SpendPeriod,
    pub amount: f64,
}

/// Removes the spend limit of `period`, if it has one.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ClearIntelligenceSpendLimitRequest {
    pub period: SpendPeriod,
}
