use cohesion_core::model::{Dialect, IntelligenceProvider};
use cohesion_core::protocol::ModelPrices;
use rquickjs::{Ctx, FromJs, Value};

use crate::conversion::FromJsField;
use cohesion_macros::JsOptions;

/// Options for `intelligence.createEndpoint()`: `kind` picks the shape.
#[derive(JsOptions)]
#[js_options(tag = "kind")]
pub enum EndpointOptions {
    Provider(ProviderEndpointOptions),
    Custom(CustomEndpointOptions),
}

/// An endpoint for a built-in provider, which supplies its base URL and
/// API format.
#[derive(JsOptions)]
pub struct ProviderEndpointOptions {
    /// Which provider, such as `IntelligenceProvider.anthropic`.
    pub provider: IntelligenceProvider,
    /// The name to look the endpoint up by with `intelligence.endpoint()`.
    /// Defaults to the provider's name, such as `"Anthropic"`.
    pub name: Option<String>,
    /// The provider's API key.
    pub api_key: String,
    /// How many minutes the provider keeps a prompt cached, at least 1.
    /// Defaults to the provider's.
    pub cache_minutes: Option<i64>,
}

/// An endpoint for any server that speaks one of the supported API formats,
/// such as a local model server.
#[derive(JsOptions)]
pub struct CustomEndpointOptions {
    /// The provider's id in the models.dev catalog (`"openai"`,
    /// `"ollama"`), where the models it imports find their details.
    pub identifier: String,
    /// The API's base URL, such as `"http://localhost:11434/v1"`.
    #[js_options(rename = "baseURL")]
    pub base_url: String,
    /// Which API format the server speaks.
    pub dialect: Dialect,
    /// The name to look the endpoint up by with `intelligence.endpoint()`.
    /// Defaults to `identifier`.
    pub name: Option<String>,
    /// The server's API key, if it needs one.
    pub api_key: Option<String>,
    /// How many minutes the server keeps a prompt cached, at least 1.
    /// Defaults to 5.
    pub cache_minutes: Option<i64>,
}

/// Options for `Account.createEndpoint()`: an endpoint for any server that
/// speaks one of the supported API formats, signing in with the account.
#[derive(JsOptions)]
pub struct AccountEndpointOptions {
    /// The provider's id in the models.dev catalog (`"openai"`), where the
    /// models it imports find their details.
    pub identifier: String,
    /// The API's base URL, such as `"https://api.openai.com/v1"`.
    #[js_options(rename = "baseURL")]
    pub base_url: String,
    /// Which API format the server speaks.
    pub dialect: Dialect,
    /// The name to look the endpoint up by with `intelligence.endpoint()`.
    /// Defaults to `identifier`.
    pub name: Option<String>,
    /// The page where a person sees what the endpoint has used, such as the
    /// service's usage settings.
    #[js_options(rename = "usageURL")]
    pub usage_url: Option<String>,
    /// How many minutes the server keeps a prompt cached, at least 1.
    /// Defaults to 5.
    pub cache_minutes: Option<i64>,
}

/// Options for `IntelligenceEndpoint.addModel()`. Details left out are read
/// from the models.dev catalog where it knows the model.
#[derive(JsOptions)]
pub struct EndpointModelOptions {
    /// The model's id at the provider, such as `"claude-sonnet-4-5"`.
    pub identifier: String,
    /// The name to show for the model, in place of the catalog's.
    pub name: Option<String>,
    /// How many tokens fit in the model's context, in place of the
    /// catalog's.
    pub context_size: Option<i64>,
    /// What the model costs, in place of the catalog's: all zero for one a
    /// subscription pays for, such as a ChatGPT plan's.
    pub prices: Option<ModelPriceOptions>,
}

/// What a model costs, in USD per million tokens. Cache tokens without a
/// price of their own are billed as input.
#[derive(JsOptions)]
pub struct ModelPriceOptions {
    pub input: f64,
    pub output: f64,
    pub cache_read: Option<f64>,
    pub cache_write: Option<f64>,
}
from_js_field_via_from_js!(ModelPriceOptions);
impl From<ModelPriceOptions> for ModelPrices {
    fn from(prices: ModelPriceOptions) -> Self {
        Self {
            input_token_cost: prices.input,
            output_token_cost: prices.output,
            cache_read_token_cost: prices.cache_read,
            cache_write_token_cost: prices.cache_write,
        }
    }
}
