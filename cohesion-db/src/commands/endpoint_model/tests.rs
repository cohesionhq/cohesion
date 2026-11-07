use cohesion_core::model::{
    CatalogModel, Dialect, EndpointModel, IntelligenceEndpointPreset, IntelligenceProvider,
    IntelligenceTier,
};
use cohesion_core::protocol::ModelPrices;

use crate::test_support::context;
use crate::{
    AddEndpointModelCommand, CommandError, CreateIntelligenceEndpointCommand, DatabaseContext,
    GetIntelligenceDefaultCommand, ListEndpointModelsCommand, RemoveEndpointModelCommand,
    SetIntelligenceDefaultCommand, UpdateCatalogModelsCommand,
};

/// Lists `model_identifier` for `provider_identifier` in the catalog, at
/// `input` and `output` dollars per million tokens and a 200k context.
fn catalog(db: &DatabaseContext, provider_identifier: &str, model_identifier: &str, input: f64) {
    db.send_blocking(UpdateCatalogModelsCommand {
        models: vec![CatalogModel {
            id: 0,
            provider_identifier: provider_identifier.to_string(),
            provider_name: provider_identifier.to_string(),
            model_identifier: model_identifier.to_string(),
            model_name: format!("{model_identifier} (catalog)"),
            context_size: Some(200_000),
            input_token_cost: Some(input),
            output_token_cost: Some(input * 5.0),
            cache_read_token_cost: None,
            cache_write_token_cost: None,
            large_context_threshold: Some(100_000),
            large_context_input_token_cost: Some(input * 2.0),
            large_context_output_token_cost: None,
            large_context_cache_read_token_cost: None,
            large_context_cache_write_token_cost: None,
        }],
    })
    .unwrap();
}

/// A custom endpoint named `name` on `provider_identifier`, which starts with
/// no models.
fn endpoint(db: &DatabaseContext, name: &str, provider_identifier: &str) -> i64 {
    db.send_blocking(CreateIntelligenceEndpointCommand {
        name: Some(name.to_string()),
        api_key: None,
        usage_url: None,
        cache_minutes: None,
        long_cache: None,
        preset: IntelligenceEndpointPreset::Custom {
            provider_identifier: provider_identifier.to_string(),
            dialect: Dialect::ChatCompletions,
            base_url: "http://localhost:8080/v1".to_string(),
        },
    })
    .unwrap()
    .id
}

fn add(db: &DatabaseContext, endpoint_id: i64, model_identifier: &str) -> EndpointModel {
    db.send_blocking(AddEndpointModelCommand {
        endpoint_id,
        model_identifier: model_identifier.to_string(),
        model_name: None,
        context_size: None,
        prices: None,
    })
    .unwrap()
}

fn identifiers(db: &DatabaseContext, endpoint_id: i64) -> Vec<String> {
    db.send_blocking(ListEndpointModelsCommand { endpoint_id })
        .unwrap()
        .into_iter()
        .map(|model| model.model_identifier)
        .collect()
}

fn prices(input: f64) -> ModelPrices {
    ModelPrices {
        input_token_cost: input,
        output_token_cost: input * 2.0,
        cache_read_token_cost: None,
        cache_write_token_cost: None,
    }
}

#[test]
fn a_catalog_model_is_linked_and_reports_the_catalogs_prices() {
    let db = context();
    let endpoint_id = endpoint(&db, "Work", "anthropic");
    catalog(&db, "anthropic", "claude-opus-5-5", 4.0);

    let model = add(&db, endpoint_id, "claude-opus-5-5");
    assert!(model.catalog_model_id.is_some());
    assert_eq!(model.model_name, "claude-opus-5-5 (catalog)");
    assert_eq!(model.context_size, Some(200_000));
    assert_eq!(model.input_token_cost, Some(4.0));
    assert_eq!(model.large_context_input_token_cost, Some(8.0));

    // A later sync's prices reach the imported model.
    catalog(&db, "anthropic", "claude-opus-5-5", 3.0);
    let listed = db
        .send_blocking(ListEndpointModelsCommand { endpoint_id })
        .unwrap();
    assert_eq!(listed[0].input_token_cost, Some(3.0));
}

#[test]
fn fields_given_for_a_catalog_model_override_the_catalogs() {
    let db = context();
    let endpoint_id = endpoint(&db, "Work", "anthropic");
    catalog(&db, "anthropic", "claude-opus-5-5", 4.0);

    let model = db
        .send_blocking(AddEndpointModelCommand {
            endpoint_id,
            model_identifier: "claude-opus-5-5".to_string(),
            model_name: Some("Mine".to_string()),
            context_size: None,
            prices: Some(prices(1.0)),
        })
        .unwrap();
    assert!(model.catalog_model_id.is_some());
    assert_eq!(model.model_name, "Mine");
    assert_eq!(model.input_token_cost, Some(1.0));
    // What it doesn't set still comes from the catalog.
    assert_eq!(model.context_size, Some(200_000));
    assert_eq!(model.large_context_input_token_cost, Some(8.0));

    // A later sync moves only what the model doesn't override.
    catalog(&db, "anthropic", "claude-opus-5-5", 3.0);
    let listed = db
        .send_blocking(ListEndpointModelsCommand { endpoint_id })
        .unwrap();
    assert_eq!(listed[0].input_token_cost, Some(1.0));
    assert_eq!(listed[0].large_context_input_token_cost, Some(6.0));
}

#[test]
fn adding_an_offered_model_again_replaces_it() {
    let db = context();
    let endpoint_id = endpoint(&db, "Work", "anthropic");
    catalog(&db, "anthropic", "claude-opus-5-5", 4.0);
    let first = db
        .send_blocking(AddEndpointModelCommand {
            endpoint_id,
            model_identifier: "claude-opus-5-5".to_string(),
            model_name: Some("Mine".to_string()),
            context_size: Some(32_000),
            prices: Some(prices(1.0)),
        })
        .unwrap();

    let repriced = db
        .send_blocking(AddEndpointModelCommand {
            endpoint_id,
            model_identifier: "claude-opus-5-5".to_string(),
            model_name: Some("Mine".to_string()),
            context_size: None,
            prices: Some(prices(2.0)),
        })
        .unwrap();
    assert_eq!(repriced.id, first.id);
    assert_eq!(repriced.input_token_cost, Some(2.0));
    // Left out, so back to the catalog's.
    assert_eq!(repriced.context_size, Some(200_000));

    // Leaving everything out clears every override.
    let cleared = add(&db, endpoint_id, "claude-opus-5-5");
    assert_eq!(cleared.id, first.id);
    assert_eq!(cleared.model_name, "claude-opus-5-5 (catalog)");
    assert_eq!(cleared.input_token_cost, Some(4.0));
    assert_eq!(identifiers(&db, endpoint_id), ["claude-opus-5-5"]);
}

#[test]
fn a_model_the_catalog_lacks_keeps_the_given_fields() {
    let db = context();
    let endpoint_id = endpoint(&db, "Local", "ollama");
    // Listed, but for another provider.
    catalog(&db, "anthropic", "qwen3", 4.0);

    let model = db
        .send_blocking(AddEndpointModelCommand {
            endpoint_id,
            model_identifier: "qwen3".to_string(),
            model_name: Some("Qwen 3".to_string()),
            context_size: Some(32_000),
            prices: Some(prices(0.5)),
        })
        .unwrap();
    assert_eq!(model.catalog_model_id, None);
    assert_eq!(model.model_name, "Qwen 3");
    assert_eq!(model.context_size, Some(32_000));
    assert_eq!(model.input_token_cost, Some(0.5));
    assert_eq!(model.output_token_cost, Some(1.0));

    let unnamed = add(&db, endpoint_id, "llama4");
    assert_eq!(unnamed.model_name, "llama4");
    assert_eq!(unnamed.input_token_cost, None);
}

#[test]
fn removing_a_model_unsets_its_tier() {
    let db = context();
    let endpoint_id = endpoint(&db, "Local", "ollama");
    let model = add(&db, endpoint_id, "qwen3");
    let other = add(&db, endpoint_id, "llama4");
    for (tier, model) in [
        (IntelligenceTier::Low, &model),
        (IntelligenceTier::High, &other),
    ] {
        db.send_blocking(SetIntelligenceDefaultCommand {
            tier,
            endpoint_model_id: model.id,
        })
        .unwrap();
    }

    db.send_blocking(RemoveEndpointModelCommand { id: model.id })
        .unwrap();

    let low = db.send_blocking(GetIntelligenceDefaultCommand {
        tier: IntelligenceTier::Low,
    });
    assert!(matches!(low, Err(CommandError::NotFound(_))));
    let high = db
        .send_blocking(GetIntelligenceDefaultCommand {
            tier: IntelligenceTier::High,
        })
        .unwrap();
    assert_eq!(high.model.model_identifier, "llama4");
    assert_eq!(high.endpoint.name, "Local");
}

#[test]
fn endpoints_on_one_provider_keep_their_own_models() {
    let db = context();
    catalog(&db, "anthropic", "claude-sonnet-5-5", 2.0);
    catalog(&db, "anthropic", "claude-opus-5-5", 4.0);
    let create = |name: &str| {
        db.send_blocking(CreateIntelligenceEndpointCommand {
            name: Some(name.to_string()),
            api_key: Some("key".to_string()),
            usage_url: None,
            cache_minutes: None,
            long_cache: None,
            preset: IntelligenceEndpointPreset::Provider {
                preset: IntelligenceProvider::Anthropic,
            },
        })
        .unwrap()
        .id
    };
    let work = create("Work");
    let personal = create("Personal");

    // Each starts with the preset's default model, linked to the catalog.
    let default_model = IntelligenceProvider::Anthropic.defaults().default_model;
    assert_eq!(identifiers(&db, work), [default_model]);
    add(&db, work, "claude-opus-5-5");

    assert_eq!(identifiers(&db, work), ["claude-opus-5-5", default_model]);
    assert_eq!(identifiers(&db, personal), [default_model]);
}

#[test]
fn a_removed_models_id_is_never_reused() {
    // A live agent notices a tier change by the model's id, so a new model
    // must not take the id of the newest one removed.
    let db = context();
    let endpoint_id = endpoint(&db, "Local", "ollama");
    add(&db, endpoint_id, "qwen3");
    let newest = add(&db, endpoint_id, "llama4");
    db.send_blocking(RemoveEndpointModelCommand { id: newest.id })
        .unwrap();

    let next = add(&db, endpoint_id, "llama4");
    assert_ne!(next.id, newest.id);
}

#[test]
fn a_later_sync_links_a_model_and_keeps_its_own_fields() {
    let db = context();
    let endpoint_id = endpoint(&db, "Work", "anthropic");
    // Added before the catalog listed either.
    let bare = add(&db, endpoint_id, "claude-opus-5-5");
    let priced = db
        .send_blocking(AddEndpointModelCommand {
            endpoint_id,
            model_identifier: "claude-sonnet-5-5".to_string(),
            model_name: None,
            context_size: None,
            prices: Some(prices(1.0)),
        })
        .unwrap();
    assert_eq!(bare.catalog_model_id, None);
    assert_eq!(bare.model_name, "claude-opus-5-5");

    catalog(&db, "anthropic", "claude-opus-5-5", 4.0);
    catalog(&db, "anthropic", "claude-sonnet-5-5", 3.0);

    let get = |id| {
        db.send_blocking(ListEndpointModelsCommand { endpoint_id })
            .unwrap()
            .into_iter()
            .find(|model| model.id == id)
            .unwrap()
    };
    let bare = get(bare.id);
    assert!(bare.catalog_model_id.is_some());
    assert_eq!(bare.model_name, "claude-opus-5-5 (catalog)");
    assert_eq!(bare.input_token_cost, Some(4.0));
    let priced = get(priced.id);
    assert!(priced.catalog_model_id.is_some());
    assert_eq!(priced.input_token_cost, Some(1.0));
    assert_eq!(priced.context_size, Some(200_000));
}
