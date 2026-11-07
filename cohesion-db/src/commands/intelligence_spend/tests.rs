use cohesion_core::model::{IntelligenceCostEvent, IntelligenceSpend, SpendPeriod};

use crate::test_support::context;
use crate::{Changed, Filtered, Invalidates};
use crate::{
    ClearIntelligenceSpendLimitCommand, CommandContext, CommandError,
    CreateIntelligenceCostEventCommand, CreateIntelligenceThreadCommand, DatabaseCommand,
    DatabaseCommandResult, DatabaseContext, Found, ListIntelligenceSpendCommand,
    SetIntelligenceSpendLimitCommand, Write,
};

/// Records one model call costing `cost` dollars, `ago` seconds ago.
fn spend(db: &DatabaseContext, cost: f64, ago: i64) {
    let thread = db
        .send_blocking(CreateIntelligenceThreadCommand {
            system_prompt: String::new(),
        })
        .unwrap();
    let event = db
        .send_blocking(CreateIntelligenceCostEventCommand {
            thread_id: thread.id,
            provider_identifier: "anthropic".to_string(),
            model_identifier: "haiku".to_string(),
            input_tokens: 1,
            output_tokens: 1,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            total_cost: cost,
            input_cost: None,
            output_cost: None,
            cache_read_cost: None,
            cache_write_cost: None,
        })
        .unwrap();
    db.send_blocking(Backdate { id: event.id, ago }).unwrap();
}

/// Moves an event's `created_at` back by `ago` seconds.
struct Backdate {
    id: i64,
    ago: i64,
}
impl DatabaseCommand for Backdate {
    type Response = IntelligenceCostEvent;
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let event: IntelligenceCostEvent = ctx
            .db
            .update()
            .set(IntelligenceCostEvent::CREATED_AT, ctx.now - self.ago)
            .filter_eq(IntelligenceCostEvent::ID, self.id)
            .returning_one()
            .found()?;
        Ok(event.invalidating_nothing())
    }
}

fn set(db: &DatabaseContext, period: SpendPeriod, amount: f64) -> Vec<IntelligenceSpend> {
    db.send_blocking(SetIntelligenceSpendLimitCommand { period, amount })
        .unwrap()
}

/// Each period's `(limit, spent, reached)`, shortest first.
fn listed(db: &DatabaseContext) -> Vec<(SpendPeriod, Option<f64>, f64, bool)> {
    db.send_blocking(ListIntelligenceSpendCommand {})
        .unwrap()
        .into_iter()
        .map(|spend| (spend.period, spend.limit, spend.spent, spend.reached))
        .collect()
}

const HOUR: i64 = 60 * 60;
const DAY: i64 = 24 * HOUR;

#[test]
fn each_period_sums_the_calls_inside_it() {
    let db = context();
    spend(&db, 1.0, 60);
    spend(&db, 2.0, 2 * HOUR);
    spend(&db, 4.0, 2 * DAY);
    spend(&db, 8.0, 31 * DAY);

    assert_eq!(
        listed(&db),
        [
            (SpendPeriod::Hour, None, 1.0, false),
            (SpendPeriod::Day, None, 3.0, false),
            (SpendPeriod::Month, None, 7.0, false),
        ]
    );
}

#[test]
fn a_limit_is_reached_once_its_period_spent_as_much() {
    let db = context();
    spend(&db, 3.0, 2 * HOUR);

    let reached = |spend: &[IntelligenceSpend]| -> Vec<bool> {
        spend.iter().map(|spend| spend.reached).collect()
    };
    assert_eq!(
        reached(&set(&db, SpendPeriod::Day, 3.0)),
        [false, true, false]
    );
    // The call is outside the hour, so the hour's limit isn't reached.
    assert_eq!(
        reached(&set(&db, SpendPeriod::Hour, 1.0)),
        [false, true, false]
    );
    assert_eq!(
        reached(&set(&db, SpendPeriod::Day, 5.0)),
        [false, false, false]
    );
}

#[test]
fn a_limit_is_replaced_and_cleared() {
    let db = context();
    set(&db, SpendPeriod::Month, 50.0);
    set(&db, SpendPeriod::Month, 20.0);
    set(&db, SpendPeriod::Hour, 1.0);
    assert_eq!(
        listed(&db),
        [
            (SpendPeriod::Hour, Some(1.0), 0.0, false),
            (SpendPeriod::Day, None, 0.0, false),
            (SpendPeriod::Month, Some(20.0), 0.0, false),
        ]
    );

    let cleared = db
        .send_blocking(ClearIntelligenceSpendLimitCommand {
            period: SpendPeriod::Month,
        })
        .unwrap();
    assert_eq!(cleared[2].limit, None);
    assert_eq!(listed(&db)[0].1, Some(1.0));
}

#[test]
fn a_limit_of_zero_or_less_is_refused() {
    let db = context();
    for amount in [0.0, -1.0] {
        let refused = db.send_blocking(SetIntelligenceSpendLimitCommand {
            period: SpendPeriod::Day,
            amount,
        });
        assert!(matches!(refused, Err(CommandError::Validation(_))));
    }
    assert_eq!(listed(&db)[1].1, None);
}
