//! The models a run compares, as `--model endpoint/model` names them: an
//! endpoint is a provider preset (`anthropic`, `openai`, …), its key read
//! from `<PRESET>_API_KEY`, or one described with `--endpoint`.

use cohesionkit::model::{Dialect, IntelligenceEndpointPreset, IntelligenceProvider};
use cohesionkit::protocol::ModelPrices;

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
    /// How reports name it: `endpoint/model`.
    pub label: String,
    pub endpoint: Endpoint,
    pub model: String,
    pub prices: Option<ModelPrices>,
}

impl Endpoint {
    /// A provider preset, by its CLI name, with its key from
    /// `<NAME>_API_KEY`.
    pub fn preset(name: &str) -> Result<Self, String> {
        let provider = IntelligenceProvider::from_str_strict(name).map_err(|_| {
            format!(
                "{name}: not a provider; use one of {}, or describe it with --endpoint",
                IntelligenceProvider::CASES.join(", ")
            )
        })?;
        let variable = format!("{}_API_KEY", name.to_uppercase());
        let api_key = std::env::var(&variable)
            .map_err(|_| format!("{name}: set {variable} to its API key"))?;
        Ok(Self {
            name: name.to_string(),
            preset: IntelligenceEndpointPreset::Provider { preset: provider },
            api_key: Some(api_key),
        })
    }

    /// An endpoint described as
    /// `name=dialect,base_url[,provider_identifier[,KEY_VARIABLE]]`, such
    /// as `local=chatCompletions,http://localhost:11434/v1`.
    pub fn described(text: &str) -> Result<Self, String> {
        let usage = || {
            format!(
                "{text}: describe an endpoint as name=dialect,base_url[,provider[,KEY_VARIABLE]]"
            )
        };
        let (name, rest) = text.split_once('=').ok_or_else(usage)?;
        let parts: Vec<&str> = rest.split(',').collect();
        let (dialect, base_url) = match parts.as_slice() {
            [dialect, base_url, ..] => (*dialect, *base_url),
            _ => return Err(usage()),
        };
        let dialect: Dialect = serde_json::from_value(serde_json::Value::from(dialect))
            .map_err(|_| {
                format!(
                    "{dialect}: not a dialect; use chatCompletions, messages, responses or statelessResponses"
                )
            })?;
        let provider_identifier = parts.get(2).copied().unwrap_or(name);
        let api_key = match parts.get(3) {
            Some(variable) => Some(
                std::env::var(variable)
                    .map_err(|_| format!("{name}: set {variable} to its API key"))?,
            ),
            None => None,
        };
        Ok(Self {
            name: name.to_string(),
            preset: IntelligenceEndpointPreset::Custom {
                provider_identifier: provider_identifier.to_string(),
                dialect,
                base_url: base_url.to_string(),
            },
            api_key,
        })
    }
}

impl ModelUnderTest {
    /// `endpoint/model`, the endpoint one of `described` or else a preset.
    /// The model is everything after the first `/`, so an OpenRouter model
    /// keeps its own slash.
    pub fn parse(text: &str, described: &[Endpoint]) -> Result<Self, String> {
        let (endpoint, model) = text
            .split_once('/')
            .filter(|(endpoint, model)| !endpoint.is_empty() && !model.is_empty())
            .ok_or_else(|| format!("{text}: name a model as endpoint/model"))?;
        let endpoint = match described.iter().find(|known| known.name == endpoint) {
            Some(known) => known.clone(),
            None => Endpoint::preset(endpoint)?,
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
