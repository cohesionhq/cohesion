use super::*;

fn run(outcome: Outcome, seconds: f64, tools: &[(&str, u32, u32)]) -> CaseResult {
    CaseResult {
        model: "m".to_string(),
        case: "c".to_string(),
        title: String::new(),
        category: String::new(),
        repeat: 1,
        outcome,
        reason: String::new(),
        seconds,
        priced: true,
        activity: Activity {
            model_calls: 3,
            tools: tools
                .iter()
                .map(|(name, calls, failed)| {
                    (
                        name.to_string(),
                        ToolUse {
                            calls: *calls,
                            failed: *failed,
                        },
                    )
                })
                .collect(),
            input_tokens: 100,
            output_tokens: 50,
            cache_read_tokens: 300,
            cache_write_tokens: 0,
            cost: 0.01,
            ..Activity::default()
        },
    }
}

#[test]
fn a_summary_sums_its_runs() {
    let results = [
        run(
            Outcome::Passed,
            10.0,
            &[(CODE_TOOL, 3, 1), ("send_message", 1, 0)],
        ),
        run(Outcome::Failed, 20.0, &[(CODE_TOOL, 4, 0)]),
        run(Outcome::Passed, 30.0, &[(DOCUMENTATION_TOOL, 2, 0)]),
    ];
    let summary = Summary::of("m", &results.iter().collect::<Vec<_>>());
    assert_eq!((summary.runs, summary.passed, summary.failed), (3, 2, 1));
    assert_eq!(summary.success_rate(), Some(2.0 / 3.0));
    assert_eq!((summary.tool_calls, summary.failed_tool_calls), (10, 1));
    assert_eq!(summary.tool_accuracy(), Some(0.9));
    assert_eq!((summary.code_calls, summary.documentation_calls), (7, 2));
    assert!((summary.cost - 0.03).abs() < 1e-12);
    assert_eq!(summary.cost_per_pass(), Some(summary.cost / 2.0));
    assert_eq!(summary.mean_seconds, 20.0);
    assert_eq!(summary.median_seconds, 20.0);
    assert_eq!(summary.p90_seconds, 30.0);
    assert_eq!(summary.output_tokens_per_second, 150.0 / 60.0);
    assert_eq!(summary.mean_total_tokens, 450.0);
    assert_eq!(summary.tokens_per_pass, Some(1350.0 / 2.0));
    assert_eq!(summary.cache_hit_rate, Some(0.75));
}

#[test]
fn a_model_with_no_runs_has_no_rates() {
    let summary = Summary::of("m", &[]);
    assert_eq!(summary.success_rate(), None);
    assert_eq!(summary.tool_accuracy(), None);
    assert_eq!(summary.tokens_per_pass, None);
    assert_eq!(summary.median_seconds, 0.0);
}
