use cohesion_core::{
    FromRequest,
    invalidation::Invalidation,
    model::{
        DEFAULT_CACHE_MINUTES, Dialect, EndpointBackend, IntelligenceEndpoint,
        IntelligenceEndpointPreset, LongCache,
    },
    protocol::{
        CreateIntelligenceEndpointRequest, DeleteIntelligenceEndpointRequest,
        ListIntelligenceEndpointsRequest, RedactedIntelligenceEndpoint,
        SetIntelligenceEndpointCacheMinutesRequest, SetIntelligenceEndpointLongCacheRequest,
        SetIntelligenceEndpointUsageUrlRequest,
    },
};

use crate::changed::{Changed, Invalidates};
use crate::{
    AddEndpointModelCommand, CommandContext, CommandError, DatabaseCommand, DatabaseCommandResult,
    Filtered, Found, IntelligenceSettings, Read, ReadableDatabase, ValidationError, Write,
};

/// Creates an endpoint named `name`, or after its provider, with the
/// preset's usage page, cache lifetime and long cache unless given others,
/// and imports
/// the preset's default model. The schema keeps names unique, ignoring
/// case.
#[derive(FromRequest)]
#[request(CreateIntelligenceEndpointRequest)]
pub struct CreateIntelligenceEndpointCommand {
    pub name: Option<String>,
    pub api_key: Option<String>,
    pub usage_url: Option<String>,
    pub cache_minutes: Option<i64>,
    pub long_cache: Option<LongCache>,
    pub preset: IntelligenceEndpointPreset,
}
impl DatabaseCommand for CreateIntelligenceEndpointCommand {
    type Response = RedactedIntelligenceEndpoint;
    type Access = Write<IntelligenceSettings>;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Self::Access>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let defaults = self.preset.resolve();

        if defaults.requires_api_key && self.api_key.is_none() {
            return Err(CommandError::Validation(ValidationError::new(
                "This endpoint requires an API key.",
            )));
        }

        let endpoint = insert_endpoint(
            ctx,
            self.name.unwrap_or(defaults.name),
            EndpointBackend::Integrated {
                provider_identifier: defaults.provider_identifier,
                dialect: defaults.dialect,
                base_url: defaults.base_url,
                api_key: self.api_key,
                usage_url: self.usage_url.or(defaults.usage_url),
                cache_minutes: self.cache_minutes.unwrap_or(defaults.cache_minutes),
                long_cache: self.long_cache.or(defaults.long_cache),
            },
        )?;
        if let Some(model_identifier) = defaults.default_model {
            ctx.run(AddEndpointModelCommand {
                endpoint_id: endpoint.id,
                model_identifier,
                model_name: None,
                context_size: None,
                prices: None,
            })?;
        }
        Ok(Self::Response::from(endpoint).invalidating([Invalidation::Intelligence]))
    }
}

/// Creates an endpoint named `name`, or after its provider, that signs in
/// with account `account_id`: each request carries the account's
/// credential. Only scripts create one, from one of their extension's own
/// accounts. It starts with no models; the schema removes it with the
/// account. Its prompt cache lasts `cache_minutes`, or
/// [`DEFAULT_CACHE_MINUTES`], and it offers no longer one.
pub struct CreateAccountEndpointCommand {
    pub account_id: i64,
    pub name: Option<String>,
    pub provider_identifier: String,
    pub dialect: Dialect,
    pub base_url: String,
    pub usage_url: Option<String>,
    pub cache_minutes: Option<i64>,
}
impl DatabaseCommand for CreateAccountEndpointCommand {
    type Response = RedactedIntelligenceEndpoint;
    type Access = Write<IntelligenceSettings>;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Self::Access>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let endpoint = insert_endpoint(
            ctx,
            self.name
                .unwrap_or_else(|| self.provider_identifier.clone()),
            EndpointBackend::Account {
                provider_identifier: self.provider_identifier,
                dialect: self.dialect,
                base_url: self.base_url,
                account_id: self.account_id,
                usage_url: self.usage_url,
                cache_minutes: self.cache_minutes.unwrap_or(DEFAULT_CACHE_MINUTES),
                long_cache: None,
            },
        )?;
        Ok(Self::Response::from(endpoint).invalidating([Invalidation::Intelligence]))
    }
}

/// Inserts an endpoint named `name` served by `backend`, refusing a name
/// another endpoint has, which the schema keeps unique ignoring case.
fn insert_endpoint(
    ctx: &mut CommandContext<'_, Write<IntelligenceSettings>>,
    name: String,
    backend: EndpointBackend,
) -> DatabaseCommandResult<IntelligenceEndpoint> {
    let inserted = ctx
        .db
        .insert::<IntelligenceEndpoint>()
        .set(IntelligenceEndpoint::NAME, name.clone())
        .set_group(backend)
        .returning_one();
    match inserted {
        Err(error) if error.is_unique_violation() => Err(ValidationError::in_field(
            "name",
            format!("An endpoint named {name} already exists; give this one another name."),
        )
        .into()),
        inserted => inserted.found(),
    }
}

/// The endpoint named `name`, ignoring case.
pub struct LookupIntelligenceEndpointByNameCommand {
    pub name: String,
}
impl DatabaseCommand for LookupIntelligenceEndpointByNameCommand {
    type Response = IntelligenceEndpoint;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        ctx.db
            .select::<IntelligenceEndpoint>()
            .filter("name = ?")
            .bind(self.name)
            .one()
            .found()
    }
}

/// Endpoint `id`.
pub struct GetIntelligenceEndpointCommand {
    pub id: i64,
}
impl DatabaseCommand for GetIntelligenceEndpointCommand {
    type Response = IntelligenceEndpoint;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        ctx.db
            .select::<IntelligenceEndpoint>()
            .filter_eq(IntelligenceEndpoint::ID, self.id)
            .one()
            .found()
    }
}

#[derive(FromRequest)]
#[request(ListIntelligenceEndpointsRequest)]
pub struct ListIntelligenceEndpointsCommand {}
impl DatabaseCommand for ListIntelligenceEndpointsCommand {
    type Response = Vec<RedactedIntelligenceEndpoint>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx
            .db
            .select::<IntelligenceEndpoint>()
            .order("intelligence_endpoints.id")
            .all()?
            .into_iter()
            .map(|x| x.into())
            .collect())
    }
}

#[derive(FromRequest)]
#[request(DeleteIntelligenceEndpointRequest)]
pub struct DeleteIntelligenceEndpointCommand {
    pub id: i64,
}
impl DatabaseCommand for DeleteIntelligenceEndpointCommand {
    type Response = RedactedIntelligenceEndpoint;
    type Access = Write<IntelligenceSettings>;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Self::Access>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let endpoint = GetIntelligenceEndpointCommand { id: self.id }.handle(&mut ctx.as_read())?;
        ctx.db
            .delete::<IntelligenceEndpoint>()
            .filter_eq(IntelligenceEndpoint::ID, self.id)
            .execute()?;
        Ok(Self::Response::from(endpoint).invalidating([Invalidation::Intelligence]))
    }
}

#[derive(FromRequest)]
#[request(SetIntelligenceEndpointUsageUrlRequest)]
pub struct SetIntelligenceEndpointUsageUrlCommand {
    pub id: i64,
    pub usage_url: Option<String>,
}
impl DatabaseCommand for SetIntelligenceEndpointUsageUrlCommand {
    type Response = RedactedIntelligenceEndpoint;
    type Access = Write<IntelligenceSettings>;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Self::Access>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let endpoint = GetIntelligenceEndpointCommand { id: self.id }.handle(&mut ctx.as_read())?;
        let backend = match endpoint.backend {
            EndpointBackend::Integrated {
                provider_identifier,
                dialect,
                base_url,
                api_key,
                usage_url: _,
                cache_minutes,
                long_cache,
            } => EndpointBackend::Integrated {
                provider_identifier,
                dialect,
                base_url,
                api_key,
                usage_url: self.usage_url,
                cache_minutes,
                long_cache,
            },
            EndpointBackend::Account {
                provider_identifier,
                dialect,
                base_url,
                account_id,
                usage_url: _,
                cache_minutes,
                long_cache,
            } => EndpointBackend::Account {
                provider_identifier,
                dialect,
                base_url,
                account_id,
                usage_url: self.usage_url,
                cache_minutes,
                long_cache,
            },
        };
        let updated = ctx
            .db
            .update::<IntelligenceEndpoint>()
            .set_group(backend)
            .filter_eq(IntelligenceEndpoint::ID, self.id)
            .returning_one()
            .found()?;
        Ok(Self::Response::from(updated).invalidating([Invalidation::Intelligence]))
    }
}

#[derive(FromRequest)]
#[request(SetIntelligenceEndpointCacheMinutesRequest)]
pub struct SetIntelligenceEndpointCacheMinutesCommand {
    pub id: i64,
    pub cache_minutes: i64,
}
impl DatabaseCommand for SetIntelligenceEndpointCacheMinutesCommand {
    type Response = RedactedIntelligenceEndpoint;
    type Access = Write<IntelligenceSettings>;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Self::Access>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let mut endpoint =
            GetIntelligenceEndpointCommand { id: self.id }.handle(&mut ctx.as_read())?;
        match &mut endpoint.backend {
            EndpointBackend::Integrated { cache_minutes, .. }
            | EndpointBackend::Account { cache_minutes, .. } => *cache_minutes = self.cache_minutes,
        }
        let updated = ctx
            .db
            .update::<IntelligenceEndpoint>()
            .set_group(endpoint.backend)
            .filter_eq(IntelligenceEndpoint::ID, self.id)
            .returning_one()
            .found()?;
        Ok(Self::Response::from(updated).invalidating([Invalidation::Intelligence]))
    }
}

#[derive(FromRequest)]
#[request(SetIntelligenceEndpointLongCacheRequest)]
pub struct SetIntelligenceEndpointLongCacheCommand {
    pub id: i64,
    pub long_cache: Option<LongCache>,
}
impl DatabaseCommand for SetIntelligenceEndpointLongCacheCommand {
    type Response = RedactedIntelligenceEndpoint;
    type Access = Write<IntelligenceSettings>;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Self::Access>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let mut endpoint =
            GetIntelligenceEndpointCommand { id: self.id }.handle(&mut ctx.as_read())?;
        match &mut endpoint.backend {
            EndpointBackend::Integrated { long_cache, .. }
            | EndpointBackend::Account { long_cache, .. } => *long_cache = self.long_cache,
        }
        let updated = ctx
            .db
            .update::<IntelligenceEndpoint>()
            .set_group(endpoint.backend)
            .filter_eq(IntelligenceEndpoint::ID, self.id)
            .returning_one()
            .found()?;
        Ok(Self::Response::from(updated).invalidating([Invalidation::Intelligence]))
    }
}

#[cfg(test)]
mod tests;
