//! The models a run compares, as `provider/model`, and the providers that
//! serve them, as `bench.json` lists them: a built-in preset (`anthropic`,
//! `openai`, …) or one described by its dialect and base URL, each with its
//! API key or the environment variable holding it.

use cohesionkit::model::{Dialect, IntelligenceEndpointPreset, IntelligenceProvider};
use cohesionkit::protocol::ModelPrices;
use serde::{Deserialize, Serialize};

/// A provider in `bench.json`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Provider {
    /// What models name it by, as in `anthropic/claude-sonnet-5-5`.
    pub name: String,
    /// A built-in provider, by its CLI name (`openai`, `anthropic`, `xai`,
    /// …); left out for one described by `dialect` and `baseUrl`. A provider
    /// with neither is the preset its name names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialect: Option<Dialect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// The models.dev provider its prices are looked up under; its name
    /// when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog: Option<String>,
    /// The API key itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// The environment variable holding the API key, when the file
    /// doesn't; a preset falls back to `<NAME>_API_KEY`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
}

/// Where a model is served: what the scratch daemon's endpoint is created
/// from.
#[derive(Clone, Debug)]
pub struct Endpoint {
    pub name: String,
    pub preset: IntelligenceEndpointPreset,
    pub api_key: Option<String>,
}

/// One model under test, on its endpoint, with the prices its cost
/// events are recorded at; `None` leaves them unpriced.
#[derive(Clone, Debug)]
pub struct ModelUnderTest {
    /// How reports name it: `provider/model`.
    pub label: String,
    pub endpoint: Endpoint,
    pub model: String,
    pub prices: Option<ModelPrices>,
}

impl Provider {
    /// The preset called `name`, its key from `<NAME>_API_KEY`.
    pub fn preset(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Self::default()
        }
    }

    /// A provider written as `name=dialect,base_url[,catalog[,KEY_VARIABLE]]`,
    /// as `--provider` takes it, such as
    /// `local=chatCompletions,http://localhost:11434/v1`.
    pub fn described(text: &str) -> Result<Self, String> {
        let usage = || {
            format!("{text}: write a provider as name=dialect,base_url[,catalog[,KEY_VARIABLE]]")
        };
        let (name, rest) = text.split_once('=').ok_or_else(usage)?;
        let parts: Vec<&str> = rest.split(',').map(str::trim).collect();
        let [dialect, base_url, ..] = parts.as_slice() else {
            return Err(usage());
        };
        Ok(Self {
            name: name.trim().to_string(),
            dialect: Some(dialect_named(dialect)?),
            base_url: Some(base_url.to_string()),
            catalog: parts.get(2).map(|catalog| catalog.to_string()),
            api_key_env: parts.get(3).map(|variable| variable.to_string()),
            ..Self::default()
        })
    }

    /// The endpoint the scratch daemon makes for it, its key read now.
    pub fn endpoint(&self) -> Result<Endpoint, String> {
        let from_env = |variable: &str| {
            std::env::var(variable).map_err(|_| {
                format!(
                    "{}: set {variable} to its API key, or give it an apiKey in bench.json",
                    self.name
                )
            })
        };
        let preset = match (&self.dialect, &self.base_url) {
            (Some(dialect), Some(base_url)) => IntelligenceEndpointPreset::Custom {
                provider_identifier: self.catalog.clone().unwrap_or_else(|| self.name.clone()),
                dialect: dialect.clone(),
                base_url: base_url.clone(),
            },
            (None, None) => {
                let name = self.preset.as_deref().unwrap_or(&self.name);
                let provider = IntelligenceProvider::from_str_strict(name).map_err(|_| {
                    format!(
                        "{}: `{name}` isn't a provider preset ({}); describe it with a dialect and baseUrl",
                        self.name,
                        IntelligenceProvider::CASES.join(", ")
                    )
                })?;
                IntelligenceEndpointPreset::Provider { preset: provider }
            }
            _ => {
                return Err(format!(
                    "{}: give both a dialect and a baseUrl, or neither",
                    self.name
                ));
            }
        };
        let api_key = match (&self.api_key, &self.api_key_env, &preset) {
            (Some(key), _, _) => Some(key.clone()),
            (None, Some(variable), _) => Some(from_env(variable)?),
            (None, None, IntelligenceEndpointPreset::Provider { .. }) => {
                let name = self.preset.as_deref().unwrap_or(&self.name);
                Some(from_env(&format!("{}_API_KEY", name.to_uppercase()))?)
            }
            (None, None, IntelligenceEndpointPreset::Custom { .. }) => None,
        };
        Ok(Endpoint {
            name: self.name.clone(),
            preset,
            api_key,
        })
    }
}

/// A dialect by its name in `bench.json`, such as `chatCompletions`.
pub fn dialect_named(name: &str) -> Result<Dialect, String> {
    serde_json::from_value(serde_json::Value::from(name)).map_err(|_| {
        format!(
            "{name}: not a dialect; use chatCompletions, messages, responses or statelessResponses"
        )
    })
}

impl ModelUnderTest {
    /// `provider/model`, the provider one of `providers` or else the preset
    /// it names. The model is everything after the first `/`, so an
    /// OpenRouter model keeps its own slash.
    pub fn parse(text: &str, providers: &[Provider]) -> Result<Self, String> {
        let (provider, model) = text
            .split_once('/')
            .filter(|(provider, model)| !provider.is_empty() && !model.is_empty())
            .ok_or_else(|| format!("{text}: name a model as provider/model"))?;
        let endpoint = match providers.iter().find(|known| known.name == provider) {
            Some(known) => known.endpoint()?,
            None => Provider::preset(provider).endpoint()?,
        };
        Ok(Self {
            label: text.to_string(),
            endpoint,
            model: model.to_string(),
            prices: None,
        })
    }

    /// The label as a file name.
    pub fn slug(&self) -> String {
        self.label
            .chars()
            .map(|c| match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '.' => c,
                _ => '_',
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
