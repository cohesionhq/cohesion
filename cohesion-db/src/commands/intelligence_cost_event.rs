use std::collections::BTreeMap;

use cohesion_core::{
    DayFilter, FromRequest,
    invalidation::Invalidation,
    model::{IntelligenceCostEvent, ModelUsage},
    protocol::ListIntelligenceCostEventsRequest,
};

use crate::changed::{Changed, Invalidates};
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, Filtered, Found, Read,
    ReadableDatabase, Write,
};

/// Records what one inference call cost.
pub struct CreateIntelligenceCostEventCommand {
    pub thread_id: i64,
    pub provider_identifier: String,
    pub model_identifier: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub total_cost: f64,
    pub input_cost: Option<f64>,
    pub output_cost: Option<f64>,
    pub cache_read_cost: Option<f64>,
    pub cache_write_cost: Option<f64>,
}
impl DatabaseCommand for CreateIntelligenceCostEventCommand {
    type Response = IntelligenceCostEvent;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let event: IntelligenceCostEvent = ctx
            .db
            .insert()
            .set(IntelligenceCostEvent::THREAD_ID, self.thread_id)
            .set(
                IntelligenceCostEvent::PROVIDER_IDENTIFIER,
                self.provider_identifier,
            )
            .set(
                IntelligenceCostEvent::MODEL_IDENTIFIER,
                self.model_identifier,
            )
            .set(IntelligenceCostEvent::INPUT_TOKENS, self.input_tokens)
            .set(IntelligenceCostEvent::OUTPUT_TOKENS, self.output_tokens)
            .set(
                IntelligenceCostEvent::CACHE_READ_TOKENS,
                self.cache_read_tokens,
            )
            .set(
                IntelligenceCostEvent::CACHE_WRITE_TOKENS,
                self.cache_write_tokens,
            )
            .set(IntelligenceCostEvent::TOTAL_COST, self.total_cost)
            .set(IntelligenceCostEvent::INPUT_COST, self.input_cost)
            .set(IntelligenceCostEvent::OUTPUT_COST, self.output_cost)
            .set(IntelligenceCostEvent::CACHE_READ_COST, self.cache_read_cost)
            .set(
                IntelligenceCostEvent::CACHE_WRITE_COST,
                self.cache_write_cost,
            )
            .returning_one()
            .found()?;
        Ok(event.invalidating([Invalidation::IntelligenceCostEvents]))
    }
}

/// The `limit` most recent cost events, newest first.
#[derive(FromRequest)]
#[request(ListIntelligenceCostEventsRequest)]
pub struct ListIntelligenceCostEventsCommand {
    pub limit: u64,
}
impl DatabaseCommand for ListIntelligenceCostEventsCommand {
    type Response = Vec<IntelligenceCostEvent>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(ctx.db.select().order("id DESC").limit(self.limit).all()?)
    }
}

/// Every model's usage, summed from its cost events: one entry per provider
/// and model, costliest first. `days` counts only events on those local
/// days, as task dates are; every event when `None`.
pub struct ListModelUsageCommand {
    pub days: Option<DayFilter>,
}
impl DatabaseCommand for ListModelUsageCommand {
    type Response = Vec<ModelUsage>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        let mut query = ctx.db.select::<IntelligenceCostEvent>();
        if let Some(days) = self.days {
            query = query.filter_day("date(created_at, 'unixepoch', 'localtime')", days);
        }
        let mut models = BTreeMap::<(String, String), ModelUsage>::new();
        for event in query.all()? {
            let usage = models
                .entry((
                    event.provider_identifier.clone(),
                    event.model_identifier.clone(),
                ))
                .or_insert_with(|| ModelUsage {
                    provider: event.provider_identifier,
                    model: event.model_identifier,
                    calls: 0,
                    input_tokens: 0,
                    output_tokens: 0,
                    cache_read_tokens: 0,
                    cache_write_tokens: 0,
                    cost: 0.0,
                });
            usage.calls += 1;
            usage.input_tokens += event.input_tokens;
            usage.output_tokens += event.output_tokens;
            usage.cache_read_tokens += event.cache_read_tokens;
            usage.cache_write_tokens += event.cache_write_tokens;
            usage.cost += event.total_cost;
        }
        let mut models: Vec<ModelUsage> = models.into_values().collect();
        models.sort_by(|a, b| b.cost.total_cmp(&a.cost));
        Ok(models)
    }
}

#[cfg(test)]
mod tests;
