use cohesion_core::{log::Module, log_info, model::CatalogModel};
use cohesion_db::{DatabaseContext, UpdateCatalogModelsCommand};

use super::catalog_error::CatalogError;
use super::models_dev_schema::{Catalog, Model, Provider, TierCondition};

#[derive(Clone, Debug)]
pub struct ModelCatalog {
    database: DatabaseContext,
}
impl ModelCatalog {
    pub fn new(database: DatabaseContext) -> ModelCatalog {
        ModelCatalog { database }
    }

    pub async fn update(&self) -> Result<(), CatalogError> {
        let download = reqwest::get("https://models.dev/catalog.json").await;
        let download = match download {
            Ok(download) => download,
            Err(e) => return Err(CatalogError::CannotDownload(e.to_string())),
        };

        let bytes = download
            .bytes()
            .await
            .map_err(|err| CatalogError::CannotRead(err.to_string()))?;

        let catalog = serde_json::from_slice::<Catalog>(&bytes)
            .map_err(|err| CatalogError::ParseError(err.to_string()))?;

        let models = catalog
            .providers
            .into_iter()
            .flat_map(|(provider_identifier, provider)| {
                provider
                    .models
                    .iter()
                    .map(|(model_identifier, model)| {
                        self.build_model(&provider_identifier, &provider, model_identifier, model)
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        log_info!(
            Module::Intelligence,
            "built catalog of {} models",
            models.len()
        );

        self.database
            .send(UpdateCatalogModelsCommand { models })
            .await
            .map_err(CatalogError::CommandError)?;

        Ok(())
    }

    fn build_model(
        &self,
        provider_identifier: &str,
        provider: &Provider,
        model_identifier: &str,
        model: &Model,
    ) -> CatalogModel {
        let (large_context_threshold, large_context_cost) = model
            .cost
            .as_ref()
            .and_then(|cost| cost.tiers.as_ref())
            .and_then(|tiers| {
                tiers
                    .iter()
                    .find(|tier| matches!(tier.tier, TierCondition::Context { size: _ }))
            })
            .map(|tier| match tier.tier {
                TierCondition::Context { size } => (Some(size), Some(tier)),
                _ => unreachable!("We guaranteed it's type context already."),
            })
            .unwrap_or((None, None));

        CatalogModel {
            id: 0,
            provider_identifier: provider_identifier.to_string(),
            provider_name: provider.name.clone(),
            model_identifier: model_identifier.to_string(),
            model_name: model.name.to_string(),
            context_size: Some(model.limit.context),
            input_token_cost: model.cost.as_ref().map(|cost| cost.input),
            output_token_cost: model.cost.as_ref().map(|cost| cost.output),
            cache_read_token_cost: model.cost.as_ref().and_then(|cost| cost.cache_read),
            cache_write_token_cost: model.cost.as_ref().and_then(|cost| cost.cache_write),
            large_context_threshold,
            large_context_input_token_cost: large_context_cost.map(|cost| cost.input),
            large_context_output_token_cost: large_context_cost.map(|cost| cost.output),
            large_context_cache_read_token_cost: large_context_cost
                .and_then(|cost| cost.cache_read),
            large_context_cache_write_token_cost: large_context_cost
                .and_then(|cost| cost.cache_write),
        }
    }
}
