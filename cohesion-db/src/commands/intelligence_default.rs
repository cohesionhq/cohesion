use cohesion_core::{
    FromRequest,
    invalidation::Invalidation,
    model::{EndpointModel, IntelligenceDefault, IntelligenceEndpoint, IntelligenceTier},
    protocol::{
        AcceptedResponse, ClearIntelligenceDefaultRequest, ListIntelligenceDefaultsRequest,
        SetIntelligenceDefaultRequest,
    },
};

use crate::changed::{Changed, Invalidates};
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, Filtered, Found,
    GetEndpointModelCommand, GetIntelligenceEndpointCommand, IntelligenceSettings, Read,
    ReadableDatabase, Write,
};

/// A tier's default: the endpoint model and the endpoint that serves it.
#[derive(Clone)]
pub struct TierModel {
    pub endpoint: IntelligenceEndpoint,
    pub model: EndpointModel,
}

/// The model `tier` uses, with its endpoint; `NotFound` when the tier has no
/// default.
pub struct GetIntelligenceDefaultCommand {
    pub tier: IntelligenceTier,
}
impl DatabaseCommand for GetIntelligenceDefaultCommand {
    type Response = TierModel;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        let default: IntelligenceDefault = ctx
            .db
            .select()
            .filter_eq(IntelligenceDefault::TIER, self.tier)
            .one()
            .found()?;
        let model = GetEndpointModelCommand {
            id: default.endpoint_model_id,
        }
        .handle(ctx)?;
        let endpoint = GetIntelligenceEndpointCommand {
            id: model.endpoint_id,
        }
        .handle(ctx)?;
        Ok(TierModel { endpoint, model })
    }
}

#[derive(FromRequest)]
#[request(ListIntelligenceDefaultsRequest)]
pub struct ListIntelligenceDefaultsCommand {}
impl DatabaseCommand for ListIntelligenceDefaultsCommand {
    type Response = Vec<IntelligenceDefault>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx.db.select().order("id").all()?)
    }
}

#[derive(FromRequest)]
#[request(SetIntelligenceDefaultRequest)]
pub struct SetIntelligenceDefaultCommand {
    pub tier: IntelligenceTier,
    pub endpoint_model_id: i64,
}
impl DatabaseCommand for SetIntelligenceDefaultCommand {
    type Response = AcceptedResponse;
    type Access = Write<IntelligenceSettings>;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Self::Access>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ctx.db
            .insert::<IntelligenceDefault>()
            .set(IntelligenceDefault::TIER, self.tier)
            .set(
                IntelligenceDefault::ENDPOINT_MODEL_ID,
                self.endpoint_model_id,
            )
            .on_conflict_update(&["tier"])
            .returning_one()
            .found()?;
        Ok(AcceptedResponse {}.invalidating([Invalidation::Intelligence]))
    }
}

/// Removes the default model of `tier`, if it has one.
#[derive(FromRequest)]
#[request(ClearIntelligenceDefaultRequest)]
pub struct ClearIntelligenceDefaultCommand {
    pub tier: IntelligenceTier,
}
impl DatabaseCommand for ClearIntelligenceDefaultCommand {
    type Response = AcceptedResponse;
    type Access = Write<IntelligenceSettings>;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Self::Access>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ctx.db
            .delete::<IntelligenceDefault>()
            .filter_eq(IntelligenceDefault::TIER, self.tier)
            .execute()?;
        Ok(AcceptedResponse {}.invalidating([Invalidation::Intelligence]))
    }
}
