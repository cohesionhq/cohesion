use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;

use cohesionkit::model::{Dialect, IntelligenceEndpointPreset};
use cohesionkit::protocol::ModelPrices;
use serde_json::{Value, json};

use crate::case::{default_directory, load_cases};
use crate::metrics::{CODE_TOOL, Outcome, ToolUse};
use crate::model_spec::{Endpoint, ModelUnderTest};
use crate::run::run_case;
use crate::validate::validate_all;

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
}

// Every case that ships fails on its untouched data and passes once its
// reference solution has run, on the script API as it is now.
#[test]
fn every_case_can_fail_and_pass() {
    let cases = load_cases(&default_directory(), &[]).unwrap();
    assert!(!cases.is_empty());
    let outcomes = runtime().block_on(validate_all(&cases, 8));
    let broken: Vec<String> = cases
        .iter()
        .zip(outcomes)
        .filter_map(|(case, outcome)| outcome.err().map(|error| format!("{}: {error}", case.name)))
        .collect();
    assert!(broken.is_empty(), "{broken:#?}");
}

/// A model speaking OpenAI's chat completions on a local port, giving
/// `answers` in turn, each reporting 1,000 prompt tokens (800 of them
/// cached) and 50 out; past the last it says it's done.
fn serve(answers: Vec<Value>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        let mut answers = VecDeque::from(answers);
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            if !read_request(&stream) {
                continue;
            }
            let message = answers
                .pop_front()
                .unwrap_or_else(|| json!({ "role": "assistant", "content": "Done." }));
            let body = json!({
                "choices": [{ "message": message }],
                "usage": {
                    "prompt_tokens": 1000,
                    "completion_tokens": 50,
                    "prompt_tokens_details": { "cached_tokens": 800 },
                },
            })
            .to_string();
            _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    base_url
}

/// Reads a request off `stream`; false when it closed without one.
fn read_request(stream: &TcpStream) -> bool {
    let mut reader = BufReader::new(stream);
    let mut length = 0;
    let mut first = true;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 && first {
            return false;
        }
        first = false;
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(": ")
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.parse().unwrap();
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).is_ok()
}

fn calling(id: &str, name: &str, arguments: Value) -> Value {
    json!({
        "role": "assistant",
        "content": null,
        "tool_calls": [{
            "id": id,
            "type": "function",
            "function": { "name": name, "arguments": arguments.to_string() },
        }],
    })
}

fn scratch_path(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("cohesion-bench-test-{}-{name}", std::process::id()));
    _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

// A run goes through Cohesion's own agent: the model's script makes the
// task the check looks for, and what the run measured is what Cohesion
// recorded of its calls, a failed script counting against tool accuracy.
#[test]
fn a_run_measures_what_the_agent_did() {
    let base_url = serve(vec![
        calling(
            "c1",
            CODE_TOOL,
            json!({
                "title": "Add the task",
                "code": "cohesion.workspace('Personal').createTask({ title: 'Buy stamps', \
                         dueDate: '2026-11-14', priority: TaskPriority.high }).id",
            }),
        ),
        calling(
            "c2",
            CODE_TOOL,
            json!({ "title": "Fail", "code": "throw new Error('boom')" }),
        ),
        calling(
            "c3",
            "send_message",
            json!({ "text": "Added **Buy stamps**.", "done": true }),
        ),
    ]);
    let model = ModelUnderTest {
        label: "fake/base".to_string(),
        endpoint: Endpoint {
            name: "fake".to_string(),
            preset: IntelligenceEndpointPreset::Custom {
                provider_identifier: "fake".to_string(),
                dialect: Dialect::ChatCompletions,
                base_url,
            },
            api_key: None,
        },
        model: "base".to_string(),
        prices: Some(ModelPrices {
            input_token_cost: 1.0,
            output_token_cost: 2.0,
            cache_read_token_cost: Some(0.1),
            cache_write_token_cost: None,
        }),
    };
    let cases = load_cases(
        &default_directory(),
        &["create-task-plain-start".to_string()],
    )
    .unwrap();
    let directory = scratch_path("run");
    let result = runtime().block_on(run_case(
        &cases[0],
        &model,
        1,
        &directory.join("cohesion.db"),
        jiff::Zoned::now().date(),
    ));
    _ = std::fs::remove_dir_all(&directory);

    assert_eq!(result.outcome, Outcome::Passed, "{}", result.reason);
    let activity = &result.activity;
    assert_eq!(activity.model_calls, 3);
    assert_eq!(
        activity.tools.get(CODE_TOOL),
        Some(&ToolUse {
            calls: 2,
            failed: 1
        })
    );
    assert_eq!(
        activity.tools.get("send_message"),
        Some(&ToolUse {
            calls: 1,
            failed: 0
        })
    );
    assert_eq!(activity.tool_accuracy(), Some(2.0 / 3.0));
    assert_eq!(activity.messages_sent, 1);
    assert_eq!(activity.reply, "Added **Buy stamps**.");
    assert_eq!(
        (
            activity.input_tokens,
            activity.cache_read_tokens,
            activity.output_tokens
        ),
        (600, 2400, 150)
    );
    // 200 input at $1, 800 cached at $0.10 and 50 out at $2 per million,
    // three times.
    assert!(
        (activity.cost - 3.0 * 0.000_38).abs() < 1e-9,
        "{}",
        activity.cost
    );
    assert!(result.seconds > 0.0);
}

// An unpriced model's calls and tokens are still on record, at no cost.
#[test]
fn an_unpriced_run_still_counts_its_calls() {
    let base_url = serve(vec![calling(
        "c1",
        "send_message",
        json!({ "text": "Nothing to do.", "done": true }),
    )]);
    let model = ModelUnderTest {
        label: "fake/base".to_string(),
        endpoint: Endpoint {
            name: "fake".to_string(),
            preset: IntelligenceEndpointPreset::Custom {
                provider_identifier: "fake".to_string(),
                dialect: Dialect::ChatCompletions,
                base_url,
            },
            api_key: None,
        },
        model: "base".to_string(),
        prices: None,
    };
    let cases = load_cases(
        &default_directory(),
        &["create-task-plain-start".to_string()],
    )
    .unwrap();
    let directory = scratch_path("unpriced");
    let result = runtime().block_on(run_case(
        &cases[0],
        &model,
        1,
        &directory.join("cohesion.db"),
        jiff::Zoned::now().date(),
    ));
    _ = std::fs::remove_dir_all(&directory);

    assert_eq!(result.outcome, Outcome::Failed, "{}", result.reason);
    assert!(!result.priced);
    assert_eq!(result.activity.model_calls, 1);
    assert_eq!(result.activity.tool_calls(), 1);
    assert_eq!(result.activity.output_tokens, 50);
    assert_eq!(result.activity.cost, 0.0);
}
