//! Each model's prices, from the models.dev catalog as Cohesion reads it,
//! fetched once per bench run rather than once per case.

use cohesionkit::db::{AddEndpointModelCommand, CreateIntelligenceEndpointCommand};
use cohesionkit::protocol::ModelPrices;

use crate::model_spec::ModelUnderTest;
use crate::scratch::Scratch;

/// Sets each model's prices to the catalog's; a model the catalog doesn't
/// price is left unpriced, and said so on stderr. Large-context prices
/// aren't carried over (a thread is rebuilt long before most models
/// reach them).
pub async fn price(models: &mut [ModelUnderTest]) -> Result<(), String> {
    let scratch = Scratch::open(None).await?;
    let fetched = scratch.refresh_catalog().await;
    if let Err(error) = &fetched {
        eprintln!("The model catalog couldn't be read ({error}); costs will be unpriced.");
    }
    for model in models.iter_mut().filter(|_| fetched.is_ok()) {
        let endpoint = scratch
            .send(CreateIntelligenceEndpointCommand {
                name: Some(model.label.clone()),
                api_key: model.endpoint.api_key.clone(),
                usage_url: None,
                cache_minutes: None,
                long_cache: None,
                preset: model.endpoint.preset.clone(),
            })
            .await?;
        let priced = scratch
            .send(AddEndpointModelCommand {
                endpoint_id: endpoint.id,
                model_identifier: model.model.clone(),
                model_name: None,
                context_size: None,
                prices: None,
            })
            .await?;
        model.prices = match (priced.input_token_cost, priced.output_token_cost) {
            (Some(input_token_cost), Some(output_token_cost)) => Some(ModelPrices {
                input_token_cost,
                output_token_cost,
                cache_read_token_cost: priced.cache_read_token_cost,
                cache_write_token_cost: priced.cache_write_token_cost,
            }),
            _ => {
                eprintln!(
                    "{}: the catalog has no prices for it; its costs will be unpriced.",
                    model.label
                );
                None
            }
        };
    }
    scratch.close().await;
    Ok(())
}
