//! `IntelligenceEndpoint` in scripts.

use cohesion_db::{
    AddEndpointModelCommand, DeleteIntelligenceEndpointCommand, ListEndpointModelsCommand,
};
use cohesion_macros::js_entity;

use cohesion_core::protocol::{RedactedEndpointBackend, RedactedIntelligenceEndpoint};
use rquickjs::Ctx;

use super::endpoint_model::JsEndpointModel;
use crate::globals::EndpointModelOptions;

/// An endpoint to perform inference: a provider's API, reached with an API
/// key or by signing in with one of an extension's accounts (it has an
/// `accountId`).
#[js_entity(RedactedIntelligenceEndpoint, js = "IntelligenceEndpoint")]
impl JsIntelligenceEndpoint {
    /// The models.dev provider, such as `"anthropic"`.
    #[qjs(get, rename = "providerIdentifier")]
    pub fn provider_identifier(&self) -> String {
        match &self.inner.backend {
            RedactedEndpointBackend::Integrated {
                provider_identifier,
                ..
            }
            | RedactedEndpointBackend::Account {
                provider_identifier,
                ..
            } => provider_identifier.clone(),
        }
    }

    /// Which API format the server speaks.
    #[qjs(get, rename = "dialect")]
    #[ts(returns = "Dialect")]
    pub fn dialect(&self) -> String {
        match &self.inner.backend {
            RedactedEndpointBackend::Integrated { dialect, .. }
            | RedactedEndpointBackend::Account { dialect, .. } => dialect.as_str().to_string(),
        }
    }

    /// The base URL requests are sent to.
    #[qjs(get, rename = "baseUrl")]
    pub fn base_url(&self) -> String {
        match &self.inner.backend {
            RedactedEndpointBackend::Integrated { base_url, .. }
            | RedactedEndpointBackend::Account { base_url, .. } => base_url.clone(),
        }
    }

    /// The page where a person sees what the endpoint has used, such as the
    /// provider's usage dashboard; null when it has none.
    #[qjs(get, rename = "usageUrl")]
    pub fn usage_url(&self) -> Option<String> {
        match &self.inner.backend {
            RedactedEndpointBackend::Integrated { usage_url, .. }
            | RedactedEndpointBackend::Account { usage_url, .. } => usage_url.clone(),
        }
    }

    /// How many minutes the endpoint's server keeps a prompt cached after
    /// the request that last used it.
    #[qjs(get, rename = "cacheMinutes")]
    pub fn cache_minutes(&self) -> i64 {
        match &self.inner.backend {
            RedactedEndpointBackend::Integrated { cache_minutes, .. }
            | RedactedEndpointBackend::Account { cache_minutes, .. } => *cache_minutes,
        }
    }

    /// The id of the account the endpoint signs in with, as an `Account`
    /// from `accounts.all()` has it; null unless it signs in with one.
    #[qjs(get, rename = "accountId")]
    pub fn account_id(&self) -> Option<i64> {
        match &self.inner.backend {
            RedactedEndpointBackend::Account { account_id, .. } => Some(*account_id),
            RedactedEndpointBackend::Integrated { .. } => None,
        }
    }

    /// The models this endpoint offers, by name.
    pub fn models<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Vec<JsEndpointModel>> {
        Ok(self
            .read(
                &ctx,
                ListEndpointModelsCommand {
                    endpoint_id: self.inner.id,
                },
            )?
            .into_iter()
            .map(|model| JsEndpointModel::new(model, self.db.clone()))
            .collect())
    }

    /// Adds a model the endpoint offers, or replaces the one with the same
    /// identifier. Needs the `intelligence` permission.
    #[qjs(rename = "addModel")]
    pub fn add_model<'js>(
        &self,
        ctx: Ctx<'js>,
        options: EndpointModelOptions,
    ) -> rquickjs::Result<JsEndpointModel> {
        let model = self.write(
            &ctx,
            AddEndpointModelCommand {
                endpoint_id: self.inner.id,
                model_identifier: options.identifier,
                model_name: options.name,
                context_size: options.context_size,
                prices: options.prices.map(Into::into),
            },
        )?;
        Ok(JsEndpointModel::new(model, self.db.clone()))
    }

    /// Deletes the endpoint and its models; a tier using one of them is left
    /// without a model. Needs the `intelligence` permission.
    pub fn delete<'js>(&mut self, ctx: Ctx<'js>) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            DeleteIntelligenceEndpointCommand { id: self.inner.id },
        )?;
        Ok(())
    }
}
