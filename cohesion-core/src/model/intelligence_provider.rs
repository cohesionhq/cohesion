//! The providers an intelligence endpoint can be created from, the wire
//! dialects they speak, and what a preset resolves to.

use cohesion_macros::{TextEnum, TsWire};
use serde::{Deserialize, Serialize};

use crate::value::{FromValue, IntoValue, Value, ValueError};

/// The wire dialect an intelligence endpoint speaks, which selects the backend
/// used to talk to it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
pub enum Dialect {
    ChatCompletions,
    Messages,
    Responses,
    /// The Responses API with nothing stored on the server: each turn sends
    /// the whole conversation and streams its reply, as a ChatGPT plan
    /// requires.
    StatelessResponses,
}

/// A provider with a built-in endpoint preset. Adding one takes a variant
/// here and its arm in [`IntelligenceProvider::defaults`]; the wire format
/// and the CLI pick it up from there.
///
/// JSON uses the camelCase name (`openAi`); the CLI uses the lowercase text
/// (`openai`).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "lowercase")]
pub enum IntelligenceProvider {
    OpenAi,
    Anthropic,
    Xai,
    Mistral,
    Fireworks,
    DeepInfra,
    TogetherAI,
    OpenRouter,
}
impl IntelligenceProvider {
    pub const fn defaults(self) -> ProviderDefaults {
        match self {
            Self::OpenAi => ProviderDefaults {
                display_name: "OpenAI",
                provider_identifier: "openai",
                default_model: "gpt-6.1-sol",
                dialect: Dialect::Responses,
                base_url: "https://api.openai.com/v1",
                usage_url: "https://platform.openai.com/usage",
                cache_minutes: 30,
                long_cache: Some(OPENAI_LONG_CACHE),
            },
            Self::Anthropic => ProviderDefaults {
                display_name: "Anthropic",
                provider_identifier: "anthropic",
                default_model: "claude-sonnet-5-5",
                dialect: Dialect::Messages,
                base_url: "https://api.anthropic.com/v1",
                usage_url: "https://platform.claude.com/usage",
                cache_minutes: DEFAULT_CACHE_MINUTES,
                long_cache: Some(ANTHROPIC_LONG_CACHE),
            },
            // xAI's chat completions are deprecated and resend the whole
            // history each turn; Responses chain stored turns and route by
            // `prompt_cache_key`, which keeps the prompt cache warm.
            Self::Xai => ProviderDefaults {
                display_name: "xAI",
                provider_identifier: "xai",
                default_model: "grok-4.7",
                dialect: Dialect::Responses,
                base_url: "https://api.x.ai/v1",
                usage_url: "https://console.x.ai/team/default/usage",
                cache_minutes: DEFAULT_CACHE_MINUTES,
                long_cache: None,
            },
            Self::Mistral => ProviderDefaults {
                display_name: "Mistral",
                provider_identifier: "mistral",
                default_model: "mistral-large-latest",
                dialect: Dialect::ChatCompletions,
                base_url: "https://api.mistral.ai/v1",
                usage_url: "https://admin.mistral.ai/organization/usage",
                cache_minutes: DEFAULT_CACHE_MINUTES,
                long_cache: None,
            },
            Self::Fireworks => ProviderDefaults {
                display_name: "Fireworks",
                provider_identifier: "fireworks-ai",
                default_model: "accounts/fireworks/models/deepseek-v4p1-flash",
                dialect: Dialect::ChatCompletions,
                base_url: "https://api.fireworks.ai/inference/v1",
                usage_url: "https://app.fireworks.ai/account/usage",
                cache_minutes: DEFAULT_CACHE_MINUTES,
                long_cache: None,
            },
            Self::DeepInfra => ProviderDefaults {
                display_name: "DeepInfra",
                provider_identifier: "deepinfra",
                default_model: "deepseek-ai/DeepSeek-V4.1-Flash",
                dialect: Dialect::ChatCompletions,
                base_url: "https://api.deepinfra.com/v1/openai",
                usage_url: "https://deepinfra.com/dash/usage",
                cache_minutes: DEFAULT_CACHE_MINUTES,
                long_cache: None,
            },
            Self::TogetherAI => ProviderDefaults {
                display_name: "Together AI",
                provider_identifier: "togetherai",
                default_model: "deepseek-ai/DeepSeek-V4.1-Flash",
                dialect: Dialect::ChatCompletions,
                base_url: "https://api.together.ai/v1",
                usage_url: "https://api.together.ai/settings/organization/~current/billing",
                cache_minutes: DEFAULT_CACHE_MINUTES,
                long_cache: None,
            },
            // OpenRouter reports what each request cost after routing it,
            // which chat completions records over the catalog's prices.
            Self::OpenRouter => ProviderDefaults {
                display_name: "OpenRouter",
                provider_identifier: "openrouter",
                default_model: "anthropic/claude-sonnet-5.5",
                dialect: Dialect::ChatCompletions,
                base_url: "https://openrouter.ai/api/v1",
                usage_url: "https://openrouter.ai/activity",
                cache_minutes: DEFAULT_CACHE_MINUTES,
                long_cache: None,
            },
        }
    }
}
/// How to reach an [`IntelligenceProvider`]. Every provider requires an API
/// key.
pub struct ProviderDefaults {
    /// Human-readable name, shown in CLI help and given to a new endpoint
    /// that names none.
    pub display_name: &'static str,
    pub provider_identifier: &'static str,
    /// The model a new endpoint imports, so a first run works; the others
    /// are imported by hand.
    pub default_model: &'static str,
    pub dialect: Dialect,
    pub base_url: &'static str,
    /// The provider's page showing what an account has used, given to a
    /// new endpoint that names none.
    pub usage_url: &'static str,
    /// How many minutes the provider keeps a prompt cached after the
    /// request that last used it, given to a new endpoint that names none.
    pub cache_minutes: i64,
    /// Its longest cache, when it offers one longer than `cache_minutes`
    /// that a request can ask for.
    pub long_cache: Option<LongCache>,
}

/// A prompt cache longer than an endpoint's default, which a request asks
/// for with [`CacheLength::Long`](super::CacheLength::Long). Stored as JSON
/// text.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, TsWire)]
pub struct LongCache {
    /// How many minutes it lasts, which a request asks for as its `ttl`
    /// (`60` as `1h`, `30` as `30m`). At least 1.
    pub minutes: i64,
    /// What a write to it costs, as a multiple of the model's write rate
    /// for the short cache, which is the only one the catalog (models.dev)
    /// publishes. More than 0.
    pub write_multiple: f64,
}
impl LongCache {
    /// The lifetime as the Messages and Responses APIs spell it: whole
    /// hours as `12h`, anything else as `45m`.
    pub fn ttl(&self) -> String {
        match self.minutes % 60 {
            0 => format!("{}h", self.minutes / 60),
            _ => format!("{}m", self.minutes),
        }
    }
}
impl IntoValue for LongCache {
    fn into_value(self) -> Value {
        Value::Text(serde_json::to_string(&self).expect("a long cache serializes"))
    }
}
impl FromValue for LongCache {
    fn from_value(value: Value) -> Result<Self, ValueError> {
        let text = String::from_value(value)?;
        serde_json::from_str(&text).map_err(|e| ValueError::Invalid(e.to_string()))
    }
}

/// Anthropic's hour-long cache: written at twice input, where the
/// five-minute cache is written at 1.25 times.
const ANTHROPIC_LONG_CACHE: LongCache = LongCache {
    minutes: 60,
    write_multiple: 2.0 / 1.25,
};
/// OpenAI's twelve-hour cache: written at 2.5 times input, where the
/// thirty-minute cache is written at 1.25 times.
const OPENAI_LONG_CACHE: LongCache = LongCache {
    minutes: 12 * 60,
    write_multiple: 2.5 / 1.25,
};

/// The cache lifetime of an endpoint whose provider doesn't publish one,
/// or that is described by hand: five minutes, the shortest any provider
/// promises.
pub const DEFAULT_CACHE_MINUTES: i64 = 5;

/// A known provider to create an endpoint for, or a custom endpoint
/// described by hand. On the wire this is `{"preset": "openAi"}` or
/// `{"preset": "custom", "provider_identifier": …, "dialect": …,
/// "base_url": …}`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[serde(tag = "preset", rename_all = "camelCase")]
pub enum IntelligenceEndpointPreset {
    Custom {
        provider_identifier: String,
        dialect: Dialect,
        base_url: String,
    },
    // Untagged so the provider's own name is the `preset` value.
    #[serde(untagged)]
    Provider { preset: IntelligenceProvider },
}
impl IntelligenceEndpointPreset {
    /// The settings this preset resolves to.
    pub fn resolve(self) -> IntelligenceEndpointDefaults {
        match self {
            Self::Provider { preset } => {
                let defaults = preset.defaults();
                IntelligenceEndpointDefaults {
                    name: defaults.display_name.to_string(),
                    provider_identifier: defaults.provider_identifier.to_string(),
                    dialect: defaults.dialect,
                    base_url: defaults.base_url.to_string(),
                    requires_api_key: true,
                    default_model: Some(defaults.default_model.to_string()),
                    usage_url: Some(defaults.usage_url.to_string()),
                    cache_minutes: defaults.cache_minutes,
                    long_cache: defaults.long_cache,
                }
            }
            Self::Custom {
                provider_identifier,
                dialect,
                base_url,
            } => IntelligenceEndpointDefaults {
                name: provider_identifier.clone(),
                provider_identifier,
                dialect,
                base_url,
                requires_api_key: false,
                default_model: None,
                usage_url: None,
                cache_minutes: DEFAULT_CACHE_MINUTES,
                long_cache: None,
            },
        }
    }
}
/// The settings an [`IntelligenceEndpointPreset`] resolves to.
pub struct IntelligenceEndpointDefaults {
    /// The endpoint's name when it is created without one: the provider's
    /// display name, or a custom endpoint's provider identifier.
    pub name: String,
    pub provider_identifier: String,
    pub dialect: Dialect,
    pub base_url: String,
    pub requires_api_key: bool,
    /// The model the endpoint starts with; a custom endpoint starts with
    /// none.
    pub default_model: Option<String>,
    /// Where the endpoint's usage is shown when it is created without one:
    /// the provider's usage page; a custom endpoint has none.
    pub usage_url: Option<String>,
    /// How long the endpoint's prompt cache lasts when it is created without
    /// one: the provider's, or [`DEFAULT_CACHE_MINUTES`] for a custom
    /// endpoint.
    pub cache_minutes: i64,
    /// Its long cache: the provider's, or none for a custom endpoint.
    pub long_cache: Option<LongCache>,
}
