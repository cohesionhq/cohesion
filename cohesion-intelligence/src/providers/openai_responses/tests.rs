//! A stateless Responses backend against a local server: every turn asks
//! the account for its credential afresh and replays the whole transcript.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use cohesion_core::model::CacheLifetime;

use super::{ResponsesBackend, ResponsesPreset};
use crate::account_credentials::{AccountCredentials, CredentialFuture};
use crate::intelligence_backend::IntelligenceBackend;
use crate::providers::EndpointKey;
use crate::transcript::Transcript;
use crate::transcript_item::{Prompt, TranscriptItem};
use crate::turn::TurnOutput;

/// An account whose credential is renewed every time it is asked for.
#[derive(Default)]
struct RenewingAccount(Mutex<u32>);
impl AccountCredentials for RenewingAccount {
    fn authorization(&self, _: i64) -> CredentialFuture<'_> {
        let mut renewals = self.0.lock().unwrap();
        *renewals += 1;
        let token = format!("token-{renewals}");
        Box::pin(async move { Ok(token) })
    }
}

/// Each request a server was sent: its authorization and its body.
type Requests = Arc<Mutex<Vec<(String, serde_json::Value)>>>;

/// Answers `turns` requests on a local port with a streamed reply saying
/// `Reply n`, keeping each request's authorization and body.
fn serve(turns: usize) -> (String, Requests) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = requests.clone();
    std::thread::spawn(move || {
        for turn in 1..=turns {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let (mut authorization, mut length) = (String::new(), 0);
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let line = line.trim_end();
                if line.is_empty() {
                    break;
                }
                let (name, value) = line.split_once(": ").unwrap_or((line, ""));
                match name.to_ascii_lowercase().as_str() {
                    "authorization" => authorization = value.to_string(),
                    "content-length" => length = value.parse().unwrap(),
                    _ => {}
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            seen.lock()
                .unwrap()
                .push((authorization, serde_json::from_slice(&body).unwrap()));
            let completed = serde_json::json!({
                "type": "response.completed",
                "response": {
                    "id": format!("resp_{turn}"),
                    "output": [{ "type": "message", "content": [
                        { "type": "output_text", "text": format!("Reply {turn}") }
                    ] }],
                    "usage": { "input_tokens": 10, "output_tokens": 2 },
                },
            });
            let events = format!(
                "event: response.created\r\ndata: {{\"type\":\"response.created\"}}\r\n\r\n\
                 event: response.completed\r\ndata: {completed}\r\n\r\n"
            );
            let mut stream = reader.into_inner();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{events}",
                events.len()
            )
            .unwrap();
        }
    });
    (base_url, requests)
}

fn user(text: &str) -> TranscriptItem {
    TranscriptItem::UserMessage(Prompt {
        text: text.to_string(),
        images: Vec::new(),
    })
}

#[test]
fn a_stateless_turn_renews_the_credential_and_replays_the_transcript() {
    let (base_url, requests) = serve(2);
    let mut backend = ResponsesBackend::new(
        base_url,
        EndpointKey::Account {
            account_id: 1,
            credentials: Arc::new(RenewingAccount::default()),
        },
        "gpt".to_string(),
        "cache".to_string(),
        ResponsesPreset::Stateless,
        CacheLifetime::Short { minutes: 5 },
    );
    let mut transcript = Transcript::new("Be brief.".to_string());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async {
        for prompt in ["Hi", "Again"] {
            transcript.add(user(prompt));
            let turn = backend.send(&transcript).await.unwrap();
            assert!(turn.remote.is_none());
            for output in turn.output {
                let TurnOutput::Message(text) = output else {
                    panic!("{output:?}");
                };
                transcript.add(TranscriptItem::AssistantMessage(Prompt {
                    text,
                    images: Vec::new(),
                }));
            }
        }
    });

    let requests = requests.lock().unwrap();
    let authorizations: Vec<_> = requests.iter().map(|(auth, _)| auth.as_str()).collect();
    assert_eq!(authorizations, ["Bearer token-1", "Bearer token-2"]);
    // Nothing is stored to chain from, so the second turn sends it all, with
    // the system prompt as instructions rather than a system message.
    let second = &requests[1].1;
    assert_eq!(second["store"], false);
    assert_eq!(second["stream"], true);
    assert_eq!(second["instructions"], "Be brief.");
    assert!(second.get("previous_response_id").is_none());
    let roles: Vec<_> = second["input"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["role"].as_str().unwrap())
        .collect();
    assert_eq!(roles, ["user", "assistant", "user"]);
}
