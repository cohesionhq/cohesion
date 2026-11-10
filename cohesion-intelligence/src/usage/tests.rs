use cohesion_core::model::EndpointModel;

use super::{TurnCost, Usage};

/// A model at $3 in, $15 out, $0.30 cache read and $3.75 cache write per
/// million tokens.
fn model() -> EndpointModel {
    EndpointModel {
        id: 1,
        endpoint_id: 1,
        model_identifier: "model".to_string(),
        model_name: "Model".to_string(),
        catalog_model_id: None,
        context_size: None,
        input_token_cost: Some(3.0),
        output_token_cost: Some(15.0),
        cache_read_token_cost: Some(0.3),
        cache_write_token_cost: Some(3.75),
        large_context_threshold: None,
        large_context_input_token_cost: None,
        large_context_output_token_cost: None,
        large_context_cache_read_token_cost: None,
        large_context_cache_write_token_cost: None,
    }
}

fn cache_write_cost(usage: Usage) -> f64 {
    match usage.cost(&model(), 1.6) {
        TurnCost::Detailed {
            cache_write_cost, ..
        } => cache_write_cost,
        other => panic!("expected a detailed cost, got {other:?}"),
    }
}

#[test]
fn a_long_cache_write_costs_its_multiple_of_the_write_rate_and_a_short_one_the_write_rate() {
    let short = cache_write_cost(Usage {
        cache_writes: 1_000_000,
        ..Usage::default()
    });
    assert!((short - 3.75).abs() < 1e-9);

    let long = cache_write_cost(Usage {
        cache_writes: 1_000_000,
        long_cache_writes: 1_000_000,
        ..Usage::default()
    });
    assert!((long - 6.0).abs() < 1e-9);

    let mixed = cache_write_cost(Usage {
        cache_writes: 1_000_000,
        long_cache_writes: 400_000,
        ..Usage::default()
    });
    assert!((mixed - (0.6 * 3.75 + 0.4 * 6.0)).abs() < 1e-9);
}

#[test]
fn counts_too_large_to_add_are_the_largest_count_not_an_overflow() {
    let usage = Usage {
        input_tokens: 10,
        cache_reads: u64::MAX,
        cache_writes: 5,
        ..Usage::default()
    };
    assert_eq!(usage.prompt_tokens(), u64::MAX);
}
