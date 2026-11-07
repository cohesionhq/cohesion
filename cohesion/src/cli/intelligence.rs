use clap::{
    Args, Subcommand,
    builder::{PossibleValue, PossibleValuesParser, TypedValueParser},
};
use cohesion_core::{
    model::{
        Dialect, EndpointModel, IntelligenceEndpointPreset, IntelligenceProvider, IntelligenceTier,
        LongCache,
    },
    protocol::{
        AddEndpointModelRequest, CreateIntelligenceEndpointRequest,
        DeleteIntelligenceEndpointRequest, ListCatalogModelsRequest, ListEndpointModelsRequest,
        ListIntelligenceDefaultsRequest, ListIntelligenceEndpointsRequest, ModelPrices,
        RedactedEndpointBackend, RedactedIntelligenceEndpoint, RemoveEndpointModelRequest,
        SetIntelligenceDefaultRequest,
    },
};

use super::{
    noun::Noun,
    table::{self, Row},
};
use crate::{cli_context::Context, cli_error::CliError, cli_macros::row, name_lookup::find};

#[derive(Subcommand)]
pub enum IntelligenceCommand {
    /// Manage intelligence endpoints, or list them when no subcommand is given.
    Endpoint(Noun<EndpointCommand>),
    /// Make one of an endpoint's models the default for a tier.
    Use {
        /// The endpoint's name or id, as shown by `cohesion ai endpoint`.
        endpoint: String,
        /// The model's identifier, as shown by `cohesion ai models`.
        model: String,
        /// The tier to set: agents talk on `agent`; a background run uses `low`, `medium` or
        /// `high`, as its cue's effort says.
        #[arg(long = "for", value_name = "TIER", value_parser = tier_parser(), default_value = "agent")]
        tier: IntelligenceTier,
    },
    /// Manage the models endpoints offer, or list them when no subcommand is given.
    Models(ModelsArgs),
    /// Browse the models.dev catalog for models to add.
    Catalog {
        /// models.dev provider identifier, e.g. `anthropic`. Defaults to the providers of
        /// configured endpoints.
        provider: Option<String>,
        /// Only list models whose identifier or name contains this.
        query: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum EndpointCommand {
    /// Add an endpoint, with its provider's default model.
    Create(CreateEndpointArgs),
    /// Remove an endpoint and its models, unsetting any tier that uses one.
    Delete {
        /// The endpoint's name or id, as shown by `cohesion ai endpoint`.
        endpoint: String,
    },
}

#[derive(Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct ModelsArgs {
    #[command(subcommand)]
    verb: Option<ModelsCommand>,
    /// Only list this endpoint's models (name or id).
    endpoint: Option<String>,
}

#[derive(Subcommand)]
pub enum ModelsCommand {
    /// Add a model to an endpoint: from the catalog when it lists the model for the endpoint's
    /// provider, otherwise described by the options.
    Add {
        /// The endpoint's name or id.
        endpoint: String,
        /// The model's identifier, e.g. `claude-opus-5-5`.
        model: String,
        /// Display name of a model the catalog does not list. Defaults to its identifier.
        #[arg(long)]
        name: Option<String>,
        /// Context window, in tokens, of a model the catalog does not list.
        #[arg(long)]
        context: Option<i64>,
        /// USD per million input tokens, for a model the catalog does not list.
        #[arg(long, requires = "output")]
        input: Option<f64>,
        /// USD per million output tokens, for a model the catalog does not list.
        #[arg(long, requires = "input")]
        output: Option<f64>,
    },
    /// Remove a model from an endpoint, unsetting any tier that uses it.
    Remove {
        /// The endpoint's name or id.
        endpoint: String,
        /// The model's identifier or id.
        model: String,
    },
}

#[derive(Args)]
pub struct CreateEndpointArgs {
    /// Provider to connect to, or `custom` to describe the endpoint by hand.
    #[arg(value_parser = preset_parser())]
    preset: PresetArg,
    /// API key cohesiond sends to the endpoint.
    #[arg(long = "api-key")]
    api_key: Option<String>,
    /// What to call the endpoint. Defaults to the provider's name, or a custom endpoint's
    /// identifier.
    #[arg(short = 'n', long = "name")]
    name: Option<String>,
    /// models.dev provider identifier, e.g. `openai`; used to look up models and pricing.
    #[arg(short = 'i', long = "identifier", required_if_eq("preset", CUSTOM))]
    provider_identifier: Option<String>,
    /// Which API dialect the endpoint speaks.
    #[arg(short = 'd', long = "dialect", value_parser = dialect_parser(), required_if_eq("preset", CUSTOM))]
    dialect: Option<Dialect>,
    /// Base URL requests are sent to.
    #[arg(short = 'u', long = "base-url", required_if_eq("preset", CUSTOM))]
    base_url: Option<String>,
    /// Page showing what the endpoint has used. Defaults to the provider's usage page.
    #[arg(long = "usage-url")]
    usage_url: Option<String>,
    /// Minutes the provider keeps a prompt cached. Defaults to the provider's (5 for a custom endpoint).
    #[arg(long = "cache-minutes", value_parser = clap::value_parser!(i64).range(1..))]
    cache_minutes: Option<i64>,
    /// Minutes the provider's longer prompt cache lasts, which agents ask for. Defaults to the
    /// provider's (none for a custom endpoint); give --long-cache-write-multiple with it.
    #[arg(long = "long-cache-minutes", value_parser = clap::value_parser!(i64).range(1..), requires = "long_cache_write_multiple")]
    long_cache_minutes: Option<i64>,
    /// What a write to the longer cache costs, as a multiple of the short cache's write price.
    #[arg(long = "long-cache-write-multiple", requires = "long_cache_minutes")]
    long_cache_write_multiple: Option<f64>,
}

impl From<CreateEndpointArgs> for CreateIntelligenceEndpointRequest {
    fn from(value: CreateEndpointArgs) -> Self {
        let preset = match value.preset {
            PresetArg::Provider(preset) => IntelligenceEndpointPreset::Provider { preset },
            PresetArg::Custom => IntelligenceEndpointPreset::Custom {
                provider_identifier: value
                    .provider_identifier
                    .expect("clap requires --identifier"),
                dialect: value.dialect.expect("clap requires --dialect"),
                base_url: value.base_url.expect("clap requires --base-url"),
            },
        };
        Self {
            name: value.name,
            api_key: value.api_key,
            usage_url: value.usage_url,
            cache_minutes: value.cache_minutes,
            long_cache: value
                .long_cache_minutes
                .zip(value.long_cache_write_multiple)
                .map(|(minutes, write_multiple)| LongCache {
                    minutes,
                    write_multiple,
                }),
            preset,
        }
    }
}

const CUSTOM: &str = "custom";

#[derive(Clone, Copy)]
enum PresetArg {
    Provider(IntelligenceProvider),
    Custom,
}

fn preset_parser() -> impl TypedValueParser<Value = PresetArg> {
    let providers = IntelligenceProvider::CASES.iter().map(|text| {
        let provider = IntelligenceProvider::from_str_strict(text).expect("CASES parse");
        PossibleValue::new(*text).help(provider.defaults().display_name)
    });
    let custom = PossibleValue::new(CUSTOM)
        .help("Any other endpoint. Give --identifier, --dialect and --base-url.");
    PossibleValuesParser::new(providers.chain([custom])).map(|text| {
        match IntelligenceProvider::from_str_strict(&text) {
            Ok(provider) => PresetArg::Provider(provider),
            Err(_) => PresetArg::Custom,
        }
    })
}

fn dialect_parser() -> impl TypedValueParser<Value = Dialect> {
    PossibleValuesParser::new(Dialect::CASES.iter().copied())
        .map(|text| Dialect::from_str_strict(&text).expect("clap only passes listed cases"))
}

fn tier_parser() -> impl TypedValueParser<Value = IntelligenceTier> {
    PossibleValuesParser::new(IntelligenceTier::CASES.iter().copied()).map(|text| {
        IntelligenceTier::from_str_strict(&text).expect("clap only passes listed cases")
    })
}

pub async fn run(ctx: &Context, verb: Option<IntelligenceCommand>) -> Result<(), CliError> {
    match verb {
        None => overview(ctx).await,
        Some(IntelligenceCommand::Endpoint(Noun { verb })) => match verb {
            None => list_endpoints(ctx).await,
            Some(EndpointCommand::Create(create)) => create_endpoint(ctx, create).await,
            Some(EndpointCommand::Delete { endpoint }) => delete_endpoint(ctx, endpoint).await,
        },
        Some(IntelligenceCommand::Use {
            endpoint,
            model,
            tier,
        }) => set_default(ctx, endpoint, model, tier).await,
        Some(IntelligenceCommand::Models(ModelsArgs { verb, endpoint })) => match verb {
            None => list_models(ctx, endpoint).await,
            Some(ModelsCommand::Add {
                endpoint,
                model,
                name,
                context,
                input,
                output,
            }) => {
                let prices = input.zip(output).map(|(input, output)| ModelPrices {
                    input_token_cost: input,
                    output_token_cost: output,
                    cache_read_token_cost: None,
                    cache_write_token_cost: None,
                });
                add_model(ctx, endpoint, model, name, context, prices).await
            }
            Some(ModelsCommand::Remove { endpoint, model }) => {
                remove_model(ctx, endpoint, model).await
            }
        },
        Some(IntelligenceCommand::Catalog { provider, query }) => {
            list_catalog(ctx, provider, query).await
        }
    }
}

const NO_ENDPOINTS: &str = "No endpoints.\nAdd one with `cohesion ai endpoint create <provider>`.";

/// `name` as one shell word, quoted when it has spaces, for the commands hints suggest.
fn shell_word(name: &str) -> String {
    if name.contains(char::is_whitespace) {
        format!("{name:?}")
    } else {
        name.to_string()
    }
}

/// Where an endpoint is: its provider and base URL.
fn endpoint_place(e: &RedactedIntelligenceEndpoint) -> (&str, &str) {
    match &e.backend {
        RedactedEndpointBackend::Integrated {
            provider_identifier,
            base_url,
            ..
        }
        | RedactedEndpointBackend::Account {
            provider_identifier,
            base_url,
            ..
        } => (provider_identifier, base_url),
    }
}

/// What an endpoint without models shows: how to add one.
fn no_models(e: &RedactedIntelligenceEndpoint) -> String {
    format!(
        "No models.\nAdd one with `cohesion ai models add {} <model>`.",
        shell_word(&e.name)
    )
}

fn endpoint_row(e: &RedactedIntelligenceEndpoint) -> Row {
    let (provider, place) = endpoint_place(e);
    row![e.name, provider, place]
}

/// A model's context size, in thousands of tokens, and its price per million input/output
/// tokens; blank where unknown.
fn context_and_price(
    context_size: Option<i64>,
    input: Option<f64>,
    output: Option<f64>,
) -> (String, String) {
    let context = context_size
        .map(|size| format!("{}k", size / 1000))
        .unwrap_or_default();
    let price = match (input, output) {
        (Some(input), Some(output)) => format!("${input:.2}/${output:.2}"),
        _ => String::new(),
    };
    (context, price)
}

fn model_row(m: &EndpointModel) -> Row {
    let (context, price) =
        context_and_price(m.context_size, m.input_token_cost, m.output_token_cost);
    row![m.model_identifier, >context, >price, m.model_name]
}

/// Every endpoint with the models it offers.
async fn endpoints_with_models(
    ctx: &Context,
) -> Result<Vec<(RedactedIntelligenceEndpoint, Vec<EndpointModel>)>, CliError> {
    let endpoints = ctx.call(ListIntelligenceEndpointsRequest).await?;
    let mut listed = Vec::with_capacity(endpoints.len());
    for endpoint in endpoints {
        let models = ctx
            .call(ListEndpointModelsRequest {
                endpoint_id: endpoint.id,
            })
            .await?;
        listed.push((endpoint, models));
    }
    Ok(listed)
}

/// Endpoints with their models, then what each tier uses.
async fn overview(ctx: &Context) -> Result<(), CliError> {
    let endpoints = endpoints_with_models(ctx).await?;
    let defaults = ctx.call(ListIntelligenceDefaultsRequest).await?;

    if endpoints.is_empty() {
        println!("{NO_ENDPOINTS}");
    }
    for (endpoint, models) in &endpoints {
        let (provider, place) = endpoint_place(endpoint);
        let title = format!("{} ({provider}, {place})", endpoint.name);
        table::section(&title, models, &no_models(endpoint), model_row);
        println!();
    }

    let tiers: Vec<_> = IntelligenceTier::CASES
        .iter()
        .map(|text| {
            let tier = IntelligenceTier::from_str_strict(text).expect("CASES parse");
            let model = defaults.iter().find(|d| d.tier == tier).and_then(|d| {
                endpoints.iter().find_map(|(endpoint, models)| {
                    let model = models.iter().find(|m| m.id == d.endpoint_model_id)?;
                    Some((endpoint.name.as_str(), model.model_identifier.as_str()))
                })
            });
            (*text, model)
        })
        .collect();
    table::section("Tiers", &tiers, "", |(tier, model)| match model {
        Some((endpoint, model)) => row![tier, endpoint, model],
        None => row![tier, "not set"],
    });
    if tiers.iter().any(|(_, model)| model.is_none()) {
        println!("Set a tier with `cohesion ai use <endpoint> <model> --for <tier>`.");
    }
    Ok(())
}

/// The endpoint named `endpoint`, with its models.
async fn endpoint_models(
    ctx: &Context,
    endpoint: &str,
) -> Result<(RedactedIntelligenceEndpoint, Vec<EndpointModel>), CliError> {
    let endpoints = ctx.call(ListIntelligenceEndpointsRequest).await?;
    let endpoint = find(&endpoints, endpoint)?.clone();
    let models = ctx
        .call(ListEndpointModelsRequest {
            endpoint_id: endpoint.id,
        })
        .await?;
    Ok((endpoint, models))
}

async fn set_default(
    ctx: &Context,
    endpoint: String,
    model: String,
    tier: IntelligenceTier,
) -> Result<(), CliError> {
    let (endpoint, models) = endpoint_models(ctx, &endpoint).await?;
    let model = find(&models, &model)?;
    ctx.call(SetIntelligenceDefaultRequest {
        tier,
        endpoint_model_id: model.id,
    })
    .await?;
    println!(
        "The {tier} tier now uses {} on {}.",
        model.model_identifier, endpoint.name
    );
    Ok(())
}

async fn list_endpoints(ctx: &Context) -> Result<(), CliError> {
    let endpoints = ctx.call(ListIntelligenceEndpointsRequest).await?;
    table::list(&endpoints, NO_ENDPOINTS, endpoint_row);
    Ok(())
}

async fn create_endpoint(ctx: &Context, create: CreateEndpointArgs) -> Result<(), CliError> {
    let result = ctx
        .call(CreateIntelligenceEndpointRequest::from(create))
        .await?;
    println!(
        "Created endpoint {}; `cohesion ai models {}` lists its models.",
        result.name,
        shell_word(&result.name)
    );
    Ok(())
}

async fn delete_endpoint(ctx: &Context, endpoint: String) -> Result<(), CliError> {
    let endpoints = ctx.call(ListIntelligenceEndpointsRequest).await?;
    let found = find(&endpoints, &endpoint)?;
    let result = ctx
        .call(DeleteIntelligenceEndpointRequest { id: found.id })
        .await?;
    println!("Deleted endpoint {}.", result.name);
    Ok(())
}

/// The models endpoints offer, with their context size, price per million input/output
/// tokens and name.
async fn list_models(ctx: &Context, endpoint: Option<String>) -> Result<(), CliError> {
    match endpoint {
        Some(endpoint) => {
            let (endpoint, models) = endpoint_models(ctx, &endpoint).await?;
            table::list(&models, &no_models(&endpoint), model_row);
        }
        None => {
            let rows: Vec<_> = endpoints_with_models(ctx)
                .await?
                .into_iter()
                .flat_map(|(endpoint, models)| {
                    models.into_iter().map(move |m| (endpoint.name.clone(), m))
                })
                .collect();
            let empty = "No models.\nAdd one with `cohesion ai models add <endpoint> <model>`.";
            table::list(&rows, empty, |(endpoint, m)| {
                let (context, price) =
                    context_and_price(m.context_size, m.input_token_cost, m.output_token_cost);
                row![endpoint, m.model_identifier, >context, >price, m.model_name]
            });
        }
    }
    Ok(())
}

async fn add_model(
    ctx: &Context,
    endpoint: String,
    model: String,
    name: Option<String>,
    context: Option<i64>,
    prices: Option<ModelPrices>,
) -> Result<(), CliError> {
    let endpoints = ctx.call(ListIntelligenceEndpointsRequest).await?;
    let endpoint = find(&endpoints, &endpoint)?;
    let added = ctx
        .call(AddEndpointModelRequest {
            endpoint_id: endpoint.id,
            model_identifier: model,
            model_name: name,
            context_size: context,
            prices,
        })
        .await?;
    println!(
        "Added {} ({}) to {}; `cohesion ai use {} {} --for <tier>` picks it.",
        added.model_identifier,
        added.model_name,
        endpoint.name,
        shell_word(&endpoint.name),
        added.model_identifier
    );
    Ok(())
}

async fn remove_model(ctx: &Context, endpoint: String, model: String) -> Result<(), CliError> {
    let (endpoint, models) = endpoint_models(ctx, &endpoint).await?;
    let model = find(&models, &model)?;
    let removed = ctx
        .call(RemoveEndpointModelRequest { id: model.id })
        .await?;
    println!(
        "Removed {} from {}; any tier that used it is now unset.",
        removed.model_identifier, endpoint.name
    );
    Ok(())
}

/// The catalog's models, with their context size, price per million input/output tokens
/// and name.
async fn list_catalog(
    ctx: &Context,
    provider: Option<String>,
    query: Option<String>,
) -> Result<(), CliError> {
    let models = ctx
        .call(ListCatalogModelsRequest {
            provider_identifier: provider,
            query,
        })
        .await?;
    let empty = "No catalog models match; `cohesion ai catalog <provider>` browses a provider.";
    table::list(&models, empty, |m| {
        let (context, price) =
            context_and_price(m.context_size, m.input_token_cost, m.output_token_cost);
        row![m.provider_identifier, m.model_identifier, >context, >price, m.model_name]
    });
    Ok(())
}
