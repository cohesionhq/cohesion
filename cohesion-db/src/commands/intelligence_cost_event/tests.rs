use cohesion_core::model::{IntelligenceCostEvent, ModelUsage};
use cohesion_core::{DayFilter, DayRange};

use crate::test_support::context;
use crate::{Changed, Filtered, Invalidates};
use crate::{
    CommandContext, CreateIntelligenceCostEventCommand, CreateIntelligenceThreadCommand,
    DatabaseCommand, DatabaseCommandResult, DatabaseContext, Found, ListModelUsageCommand, Write,
};

/// Records one call of `model` at `provider`, with `tokens` of each kind and
/// `cost` dollars for each, answering the event's id.
fn record(db: &DatabaseContext, provider: &str, model: &str, tokens: i64, cost: f64) -> i64 {
    let thread = db
        .send_blocking(CreateIntelligenceThreadCommand {
            system_prompt: String::new(),
        })
        .unwrap();
    db.send_blocking(CreateIntelligenceCostEventCommand {
        thread_id: thread.id,
        provider_identifier: provider.to_string(),
        model_identifier: model.to_string(),
        input_tokens: tokens,
        output_tokens: tokens,
        cache_read_tokens: tokens,
        cache_write_tokens: tokens,
        total_cost: cost * 4.0,
        input_cost: Some(cost),
        output_cost: Some(cost),
        cache_read_cost: Some(cost),
        cache_write_cost: Some(cost),
    })
    .unwrap()
    .id
}

/// Moves an event's `created_at` to `at`, a Unix timestamp.
struct Backdate {
    id: i64,
    at: i64,
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
            .set(IntelligenceCostEvent::CREATED_AT, self.at)
            .filter_eq(IntelligenceCostEvent::ID, self.id)
            .returning_one()
            .found()?;
        Ok(event.invalidating_nothing())
    }
}

fn usage(db: &DatabaseContext, days: Option<DayFilter>) -> Vec<ModelUsage> {
    db.send_blocking(ListModelUsageCommand { days }).unwrap()
}

#[test]
fn usage_is_summed_per_model_costliest_first() {
    let db = context();
    record(&db, "anthropic", "haiku", 10, 0.25);
    record(&db, "anthropic", "haiku", 20, 0.5);
    record(&db, "anthropic", "opus", 1, 2.0);
    record(&db, "openai", "haiku", 3, 0.125);

    let model = |provider: &str, model: &str, calls, tokens, cost| ModelUsage {
        provider: provider.to_string(),
        model: model.to_string(),
        calls,
        input_tokens: tokens,
        output_tokens: tokens,
        cache_read_tokens: tokens,
        cache_write_tokens: tokens,
        cost,
    };
    assert_eq!(
        usage(&db, None),
        vec![
            model("anthropic", "opus", 1, 1, 8.0),
            model("anthropic", "haiku", 2, 30, 3.0),
            model("openai", "haiku", 1, 3, 0.5),
        ]
    );
}

#[test]
fn a_days_filter_counts_only_events_on_those_local_days() {
    let db = context();
    let old = record(&db, "anthropic", "haiku", 1, 1.0);
    record(&db, "anthropic", "haiku", 2, 1.0);
    // Noon UTC on 2020-01-15, that day locally in nearly every time zone.
    db.send_blocking(Backdate {
        id: old,
        at: 1_579_089_600,
    })
    .unwrap();

    let calls = |days| -> Vec<(i64, i64)> {
        usage(&db, Some(days))
            .iter()
            .map(|model| (model.calls, model.input_tokens))
            .collect()
    };
    assert_eq!(calls(DayFilter::Day("2020-01-15".to_string())), [(1, 1)]);
    assert_eq!(
        calls(DayFilter::Range(DayRange {
            before: None,
            after: Some("2020-01-31".to_string()),
        })),
        [(1, 2)]
    );
    assert_eq!(calls(DayFilter::Days(vec!["2020-01-16".to_string()])), []);
}
