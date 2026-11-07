use cohesion_core::{
    FromRequest, Projection,
    invalidation::Invalidation,
    model::{IntelligenceCostEvent, IntelligenceSpend, IntelligenceSpendLimit, SpendPeriod},
    protocol::{
        ClearIntelligenceSpendLimitRequest, ListIntelligenceSpendRequest,
        SetIntelligenceSpendLimitRequest,
    },
};

use crate::changed::{Changed, Invalidates};
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, Filtered, Found, IntelligenceSettings,
    Read, ReadableDatabase, Write,
};

/// What the cost events since a moment add up to; nothing is zero.
#[derive(Projection)]
#[projection(of = IntelligenceCostEvent)]
struct Spent {
    #[projection(coalesce("SUM(total_cost)", "0"))]
    cost: f64,
}

/// What every model call cost over each spend period, against its limit,
/// one entry per period, shortest first. A period has reached its limit
/// once what it spent is at least the limit: background work makes no
/// model call then.
#[derive(FromRequest)]
#[request(ListIntelligenceSpendRequest)]
pub struct ListIntelligenceSpendCommand {}
impl DatabaseCommand for ListIntelligenceSpendCommand {
    type Response = Vec<IntelligenceSpend>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        let limits: Vec<IntelligenceSpendLimit> = ctx.db.select().all()?;
        SpendPeriod::ALL
            .iter()
            .map(|&period| {
                let spent: Spent = ctx
                    .db
                    .select_as()
                    .filter("created_at > ?")
                    .bind(ctx.now - period.seconds())
                    .one()
                    .found()?;
                let limit = limits
                    .iter()
                    .find(|limit| limit.period == period)
                    .map(|limit| limit.amount);
                Ok(IntelligenceSpend {
                    period,
                    limit,
                    spent: spent.cost,
                    reached: limit.is_some_and(|limit| spent.cost >= limit),
                })
            })
            .collect()
    }
}

/// Limits what model calls may cost over `period` to `amount`, replacing
/// its limit if it has one; answers every period's spend.
#[derive(FromRequest)]
#[request(SetIntelligenceSpendLimitRequest)]
pub struct SetIntelligenceSpendLimitCommand {
    pub period: SpendPeriod,
    pub amount: f64,
}
impl DatabaseCommand for SetIntelligenceSpendLimitCommand {
    type Response = Vec<IntelligenceSpend>;
    type Access = Write<IntelligenceSettings>;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Self::Access>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ctx.db
            .insert::<IntelligenceSpendLimit>()
            .set(IntelligenceSpendLimit::PERIOD, self.period)
            .set(IntelligenceSpendLimit::AMOUNT, self.amount)
            .on_conflict_update(&["period"])
            .execute()?;
        let spend = ListIntelligenceSpendCommand {}.handle(&mut ctx.as_read())?;
        Ok(spend.invalidating([Invalidation::Intelligence]))
    }
}

/// Removes the spend limit of `period`, if it has one; answers every
/// period's spend.
#[derive(FromRequest)]
#[request(ClearIntelligenceSpendLimitRequest)]
pub struct ClearIntelligenceSpendLimitCommand {
    pub period: SpendPeriod,
}
impl DatabaseCommand for ClearIntelligenceSpendLimitCommand {
    type Response = Vec<IntelligenceSpend>;
    type Access = Write<IntelligenceSettings>;
    fn handle(
        self,
        ctx: &mut CommandContext<'_, Self::Access>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ctx.db
            .delete::<IntelligenceSpendLimit>()
            .filter_eq(IntelligenceSpendLimit::PERIOD, self.period)
            .execute()?;
        let spend = ListIntelligenceSpendCommand {}.handle(&mut ctx.as_read())?;
        Ok(spend.invalidating([Invalidation::Intelligence]))
    }
}

#[cfg(test)]
mod tests;
