use cohesion_core::model::{IntelligenceEndpointPreset, IntelligenceTier};
use cohesion_db::{
    CreateIntelligenceEndpointCommand, GetIntelligenceDefaultCommand, GuardedDatabaseContext,
    ListIntelligenceEndpointsCommand, LookupIntelligenceEndpointByNameCommand,
};
use rquickjs::function::Opt;
use rquickjs::{Ctx, JsLifetime, Value, class::Trace};

use cohesion_macros::js_global;

use super::global::Global;
use super::intelligence_endpoint_options::EndpointOptions;
use crate::conversion::FromJsField;
use crate::entities::endpoint_model::JsEndpointModel;
use crate::entities::intelligence_endpoint::JsIntelligenceEndpoint;
use crate::traced_command::{read, write};

#[derive(Trace, JsLifetime)]
#[rquickjs::class]
pub struct IntelligenceGlobal {
    #[qjs(skip_trace)]
    database: GuardedDatabaseContext,
}
#[js_global("intelligence")]
#[rquickjs::methods]
impl IntelligenceGlobal {
    /// Creates and saves a new intelligence endpoint, for a built-in
    /// provider (`{ kind: "provider", provider: IntelligenceProvider.openAi,
    /// apiKey }`) or a server described by hand (`{ kind: "custom", ... }`).
    /// A provider's endpoint starts with the provider's default model.
    /// Throws if the name is taken or if `baseURL` is not an HTTP URL. Needs
    /// the `intelligence` permission.
    #[qjs(rename = "createEndpoint")]
    pub fn create_endpoint<'js>(
        &self,
        ctx: Ctx<'js>,
        options: EndpointOptions,
    ) -> rquickjs::Result<JsIntelligenceEndpoint> {
        let message = match options {
            EndpointOptions::Provider(options) => CreateIntelligenceEndpointCommand {
                preset: IntelligenceEndpointPreset::Provider {
                    preset: options.provider,
                },
                name: options.name,
                api_key: Some(options.api_key),
                usage_url: None,
                cache_minutes: options.cache_minutes,
                long_cache: None,
            },
            EndpointOptions::Custom(options) => CreateIntelligenceEndpointCommand {
                preset: IntelligenceEndpointPreset::Custom {
                    provider_identifier: options.identifier,
                    base_url: options.base_url,
                    dialect: options.dialect,
                },
                name: options.name,
                api_key: options.api_key,
                usage_url: None,
                cache_minutes: options.cache_minutes,
                long_cache: None,
            },
        };
        let endpoint = write(&ctx, &self.database, message)?;
        Ok(JsIntelligenceEndpoint::new(endpoint, self.database.clone()))
    }

    /// Every endpoint, oldest first, clients' included.
    pub fn endpoints<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Vec<JsIntelligenceEndpoint>> {
        Ok(
            read(&ctx, &self.database, ListIntelligenceEndpointsCommand {})?
                .into_iter()
                .map(|endpoint| JsIntelligenceEndpoint::new(endpoint, self.database.clone()))
                .collect(),
        )
    }

    /// The endpoint named `name`, such as `"Anthropic"`, ignoring case.
    /// Throws if there is none.
    pub fn endpoint<'js>(
        &self,
        ctx: Ctx<'js>,
        name: String,
    ) -> rquickjs::Result<JsIntelligenceEndpoint> {
        let endpoint = read(
            &ctx,
            &self.database,
            LookupIntelligenceEndpointByNameCommand { name },
        )?;
        Ok(JsIntelligenceEndpoint::new(
            endpoint.into(),
            self.database.clone(),
        ))
    }

    /// The model `tier` uses, `IntelligenceTier.agent` when it is left out.
    /// Throws if the tier has no default model.
    pub fn model<'js>(
        &self,
        ctx: Ctx<'js>,
        #[ts(type = "IntelligenceTier")] tier: Opt<Value<'js>>,
    ) -> rquickjs::Result<JsEndpointModel> {
        let tier = match tier.0 {
            Some(tier) => IntelligenceTier::from_js_field(&ctx, tier)?,
            None => IntelligenceTier::Agent,
        };
        let tier_model = read(&ctx, &self.database, GetIntelligenceDefaultCommand { tier })?;
        Ok(JsEndpointModel::new(
            tier_model.model,
            self.database.clone(),
        ))
    }
}
impl Global for IntelligenceGlobal {
    fn install<'a>(
        context: &Ctx<'a>,
        database: GuardedDatabaseContext,
    ) -> Result<(), rquickjs::Error> {
        context
            .globals()
            .set("intelligence", IntelligenceGlobal { database })?;
        Ok(())
    }
}
