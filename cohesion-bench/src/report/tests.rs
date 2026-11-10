use super::*;
use crate::metrics::Activity;

#[test]
fn a_csv_field_is_quoted_only_when_it_must_be() {
    assert_eq!(csv_field("plain"), "plain");
    assert_eq!(csv_field("a, b"), "\"a, b\"");
    assert_eq!(csv_field("say \"hi\"\nthen"), "\"say \"\"hi\"\"\nthen\"");
}

#[test]
fn the_report_escapes_what_the_agent_said() {
    let result = CaseResult {
        model: "m".to_string(),
        case: "c".to_string(),
        title: "T".to_string(),
        category: "tasks".to_string(),
        difficulty: Difficulty::Easy,
        repeat: 1,
        outcome: Outcome::Failed,
        reason: "missed: <b>bold</b>".to_string(),
        seconds: 1.0,
        priced: false,
        activity: Activity {
            reply: "<script>alert(1)</script>".to_string(),
            ..Activity::default()
        },
    };
    let directory = std::env::temp_dir()
        .join(format!("cohesion-bench-report-{}", std::process::id()))
        .join("c");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("prompt.md"),
        "---\ntitle: T\ndifficulty: easy\nstate: none\n---\nDo it\n",
    )
    .unwrap();
    std::fs::write(
        directory.join("validate.ts"),
        "export default function validate() {}\n",
    )
    .unwrap();
    let case = Case::load(&directory, "tasks".to_string(), "", &Default::default()).unwrap();
    _ = std::fs::remove_dir_all(&directory);
    let models = ["m".to_string()];
    let cases = [case];
    let info = RunInfo {
        started: "now".to_string(),
        models: &models,
        cases: &cases,
        repeats: 1,
    };
    let summaries = crate::metrics::summaries(&models, std::slice::from_ref(&result));
    let page = html(&info, &[result], &summaries);
    assert!(page.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(!page.contains("<script>"));
    assert!(page.contains("missed: &lt;b&gt;bold&lt;/b&gt;"));
    assert!(page.contains("unpriced"));
}

/// A deterministic number in 0..1 from `parts`, standing in for chance.
fn chance(parts: &[&str]) -> f64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in parts.join("/").bytes() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
    }
    (hash >> 11) as f64 / (1u64 << 53) as f64
}

// Writes a report from made-up runs of every shipped case, for working on
// the report's design without paying for a run:
// `cargo test -p cohesion-bench mock_report -- --ignored`, then open
// target/cohesion-bench/mock/report.html.
#[test]
#[ignore]
fn mock_report() {
    use crate::case::{default_directory, load_cases};
    use crate::metrics::ToolUse;
    // (model, skill, seconds per call, tokens per call, dollars per million tokens)
    let profiles = [
        (
            "openrouter/deepseek/deepseek-v4.1-flash",
            0.90,
            1.8,
            16_000.0,
            0.25,
        ),
        ("anthropic/claude-haiku-5-5", 0.86, 1.2, 21_000.0, 0.30),
        ("anthropic/claude-sonnet-5-5", 0.95, 2.4, 22_000.0, 1.20),
        ("openrouter/z-ai/glm-5.3-flash", 0.84, 4.5, 16_000.0, 0.55),
        ("openai/gpt-6.1-terra", 0.78, 2.8, 13_500.0, 1.60),
        ("openai/gpt-6.1-mini", 0.66, 1.0, 12_000.0, 0.15),
    ];
    let cases = load_cases(&default_directory(), &[]).unwrap();
    let repeats = 2;
    let mut results = Vec::new();
    for (model, skill, per_call, per_token, price) in profiles {
        for case in &cases {
            for repeat in 1..=repeats {
                let roll = |what: &str| chance(&[model, &case.name, &repeat.to_string(), what]);
                let difficulty = match case.difficulty {
                    Difficulty::Easy => 0.08,
                    Difficulty::Medium => 0.0,
                    Difficulty::Hard => -0.16,
                };
                let leaning = (chance(&[model, &case.category]) - 0.5) * 0.3;
                let outcome = match roll("outcome") {
                    u if u < 0.008 => Outcome::Error,
                    u if u < skill + difficulty + leaning => Outcome::Passed,
                    _ => Outcome::Failed,
                };
                let calls = 2
                    + (roll("calls") * 4.0) as u32
                    + if case.difficulty == Difficulty::Hard {
                        3
                    } else {
                        0
                    };
                let tokens = per_token * f64::from(calls) * (0.8 + roll("tokens") * 0.4);
                let seconds = per_call * f64::from(calls) * (0.6 + roll("time").powi(3) * 4.0);
                let failed = u32::from(roll("tool") > skill);
                results.push(CaseResult {
                    model: model.to_string(),
                    case: case.name.clone(),
                    title: case.title.clone(),
                    category: case.category.clone(),
                    difficulty: case.difficulty,
                    repeat,
                    outcome,
                    reason: match outcome {
                        Outcome::Passed => String::new(),
                        _ => "missed: what the case checks".to_string(),
                    },
                    seconds,
                    priced: true,
                    activity: Activity {
                        model_calls: calls,
                        tools: [(
                            CODE_TOOL.to_string(),
                            ToolUse {
                                calls: calls - 1,
                                failed,
                            },
                        )]
                        .into_iter()
                        .collect(),
                        input_tokens: (tokens * 0.15) as i64,
                        cache_read_tokens: (tokens * 0.8) as i64,
                        output_tokens: (tokens * 0.05) as i64,
                        cost: tokens * price / 1_000_000.0,
                        reply: "A made-up reply.".to_string(),
                        ..Activity::default()
                    },
                });
            }
        }
    }
    let models: Vec<String> = profiles.iter().map(|p| p.0.to_string()).collect();
    let info = RunInfo {
        started: "2026-11-10 15:00 (mock data)".to_string(),
        models: &models,
        cases: &cases,
        repeats,
    };
    let summaries = crate::metrics::summaries(&models, &results);
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/cohesion-bench/mock");
    std::fs::create_dir_all(&out).unwrap();
    write(&out, &info, &results, &summaries).unwrap();
}
