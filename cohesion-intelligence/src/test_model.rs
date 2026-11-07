//! A model for tests: a local server speaking OpenAI's chat completions,
//! which gives its answers in turn and keeps every request it was sent.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use cohesion_core::model::{Dialect, EndpointBackend, EndpointModel, IntelligenceEndpoint};
use cohesion_core::model::{IntelligenceEndpointPreset, IntelligenceTier};
use cohesion_db::{
    AddEndpointModelCommand, CreateIntelligenceEndpointCommand, DatabaseContext,
    SetIntelligenceDefaultCommand, TierModel,
};
use serde_json::{Value, json};
use tokio::sync::watch;

use crate::account_credentials::{AccountCredentials, CredentialFuture};

/// The server's model, by the identifier requests name it by.
const MODEL: &str = "base";

/// One answer: the chat-completions message the model replies with, or
/// `None` for one that never comes.
pub(crate) type Answer = Option<Value>;

/// The model's answer saying `text`.
pub(crate) fn text(text: &str) -> Answer {
    Some(json!({ "role": "assistant", "content": text }))
}

/// The model's answer calling tool `name` once per id in `ids`.
pub(crate) fn calling(name: &str, ids: &[&str]) -> Answer {
    let calls: Vec<Value> = ids
        .iter()
        .map(|id| {
            json!({
                "id": id,
                "type": "function",
                "function": { "name": name, "arguments": "{}" },
            })
        })
        .collect();
    Some(json!({ "role": "assistant", "content": null, "tool_calls": calls }))
}

/// The model's answer calling tool `name` once, with `arguments`.
pub(crate) fn calling_with(name: &str, arguments: Value) -> Answer {
    let call = json!({
        "id": "c1",
        "type": "function",
        "function": { "name": name, "arguments": arguments.to_string() },
    });
    Some(json!({ "role": "assistant", "content": null, "tool_calls": [call] }))
}

/// A running server and what it has been sent.
pub(crate) struct TestModel {
    base_url: String,
    sent: Arc<Mutex<Vec<Value>>>,
    calls: watch::Sender<usize>,
}

impl TestModel {
    /// Serves `answers` in turn on a local port; past the last, the model
    /// never answers.
    pub(crate) fn serve(answers: Vec<Answer>) -> Arc<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let model = Arc::new(Self {
            base_url: format!("http://{}/v1", listener.local_addr().unwrap()),
            sent: Arc::default(),
            calls: watch::Sender::new(0),
        });
        let sent = model.sent.clone();
        let calls = model.calls.clone();
        std::thread::spawn(move || {
            let mut answers = VecDeque::from(answers);
            // Held open, unanswered, for as long as the server runs.
            let mut waiting = Vec::new();
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                // A connection closed before it sent anything isn't a call.
                let Some(body) = request_body(&stream) else {
                    continue;
                };
                sent.lock().unwrap().push(body);
                calls.send_modify(|calls| *calls += 1);
                match answers.pop_front().flatten() {
                    Some(message) => respond(&mut stream, message),
                    None => waiting.push(stream),
                }
            }
        });
        model
    }

    /// Every request the model was sent, oldest first.
    pub(crate) fn sent(&self) -> Vec<Value> {
        self.sent.lock().unwrap().clone()
    }

    /// How many requests the model has been sent, as they come.
    pub(crate) fn calls(&self) -> watch::Receiver<usize> {
        self.calls.subscribe()
    }

    /// The server as an endpoint's model, with no prices.
    pub(crate) fn tier_model(&self) -> TierModel {
        TierModel {
            endpoint: IntelligenceEndpoint {
                id: 1,
                name: "Test".to_string(),
                backend: EndpointBackend::Integrated {
                    provider_identifier: "test".to_string(),
                    dialect: Dialect::ChatCompletions,
                    base_url: self.base_url.clone(),
                    api_key: None,
                    usage_url: None,
                    cache_minutes: 5,
                    long_cache: None,
                },
            },
            model: EndpointModel {
                id: 1,
                endpoint_id: 1,
                model_identifier: MODEL.to_string(),
                model_name: "Base".to_string(),
                catalog_model_id: None,
                context_size: None,
                input_token_cost: None,
                output_token_cost: None,
                cache_read_token_cost: None,
                cache_write_token_cost: None,
                large_context_threshold: None,
                large_context_input_token_cost: None,
                large_context_output_token_cost: None,
                large_context_cache_read_token_cost: None,
                large_context_cache_write_token_cost: None,
            },
        }
    }

    /// Stores the server as an endpoint in `db`, its model the default for
    /// `tier`.
    pub(crate) fn store(&self, db: &DatabaseContext, tier: IntelligenceTier) {
        let endpoint = db
            .send_blocking(CreateIntelligenceEndpointCommand {
                name: Some("Test".to_string()),
                api_key: None,
                usage_url: None,
                cache_minutes: None,
                long_cache: None,
                preset: IntelligenceEndpointPreset::Custom {
                    provider_identifier: "test".to_string(),
                    dialect: Dialect::ChatCompletions,
                    base_url: self.base_url.clone(),
                },
            })
            .unwrap();
        let model = db
            .send_blocking(AddEndpointModelCommand {
                endpoint_id: endpoint.id,
                model_identifier: MODEL.to_string(),
                model_name: None,
                context_size: None,
                prices: None,
            })
            .unwrap();
        db.send_blocking(SetIntelligenceDefaultCommand {
            tier,
            endpoint_model_id: model.id,
        })
        .unwrap();
    }
}

/// The server's endpoint signs in with no account.
impl AccountCredentials for TestModel {
    fn authorization(&self, _: i64) -> CredentialFuture<'_> {
        unreachable!("the test model signs in with no account")
    }
}

/// The JSON body of the request on `stream`, or `None` when the client
/// closed it without sending one.
fn request_body(stream: &TcpStream) -> Option<Value> {
    let mut reader = BufReader::new(stream);
    let mut length = 0;
    let mut first = true;
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line).unwrap_or(0);
        if first && read == 0 {
            return None;
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
    reader.read_exact(&mut body).unwrap();
    Some(serde_json::from_slice(&body).unwrap())
}

/// Answers the request on `stream` with `message`, and closes it.
fn respond(stream: &mut TcpStream, message: Value) {
    let body = json!({ "choices": [{ "message": message }] }).to_string();
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
}

/// The text of each of `request`'s messages with `role`, in order: a user's
/// or the system prompt's content, a tool result's `(call id, content)`
/// joined by a newline.
pub(crate) fn texts(request: &Value, role: &str) -> Vec<String> {
    request["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|message| message["role"] == role)
        .map(|message| match &message["tool_call_id"] {
            Value::String(id) => format!("{id}\n{}", message["content"].as_str().unwrap()),
            _ => message["content"].as_str().unwrap().to_string(),
        })
        .collect()
}
