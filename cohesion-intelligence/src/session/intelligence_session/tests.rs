use std::borrow::Cow;
use std::sync::{Arc, Mutex};

use cohesion_db::{Container, DatabaseContext, Secrets};
use serde_json::{Value, json};

use super::Session;
use crate::activity::{Activity, ShowActivity, ShowFuture};
use crate::intelligence_error::IntelligenceError;
use crate::system_prompt::{AboutUser, Profile, Roster, SystemPrompt};
use crate::test_model::{Answer, TestModel, calling, calling_with, text, texts};
use crate::tool_definition::Tool;
use crate::tools::{SessionTool, ToolFuture, asks_done};
use crate::transcript_item::{Prompt, ToolCallStatus};
use crate::turn_input::{StoppedFuture, TurnInput};

/// What the user does while the answer works: the prompts they have sent,
/// and whether they stopped it.
#[derive(Default)]
struct User {
    waiting: Mutex<Vec<Prompt>>,
    stopped: Mutex<bool>,
}
impl User {
    fn says(&self, text: &str) {
        self.waiting.lock().unwrap().push(Prompt {
            text: text.to_string(),
            images: Vec::new(),
        });
    }
}
impl TurnInput for User {
    fn take(&self) -> Vec<Prompt> {
        std::mem::take(&mut *self.waiting.lock().unwrap())
    }
    fn close(&self) -> Vec<Prompt> {
        self.take()
    }
    fn is_stopped(&self) -> bool {
        *self.stopped.lock().unwrap()
    }
    fn stopped(&self) -> StoppedFuture<'_> {
        let stopped = self.is_stopped();
        Box::pin(async move {
            if !stopped {
                std::future::pending::<()>().await;
            }
        })
    }
}

/// A tool that does what the user does when it runs.
struct Meanwhile {
    user: Arc<User>,
    act: fn(&User),
}
impl SessionTool for Meanwhile {
    fn definition(&self) -> Tool {
        Tool {
            name: "lookup",
            description: Cow::Borrowed("Looks something up."),
            fields: Vec::new(),
        }
    }
    fn ends_turn(&self, arguments: &str) -> bool {
        asks_done(arguments)
    }
    fn call<'a>(&'a self, _arguments: &'a str) -> ToolFuture<'a> {
        (self.act)(&self.user);
        Box::pin(async { ToolCallStatus::Success("Found.".to_string()) })
    }
}

/// Sends `prompts` in turn on one session answered by `answers`, with
/// `user` acting as `act` whenever the tool runs. Returns each answer and
/// what the model was sent.
fn converse(
    answers: Vec<Answer>,
    user: Arc<User>,
    act: fn(&User),
    prompts: &[&str],
) -> (Vec<Result<String, IntelligenceError>>, Vec<Value>) {
    let model = TestModel::serve(answers);
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    let database = DatabaseContext::new(container, Secrets::new(), ());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let replies = runtime.block_on(async {
        let mut session = Session::new(
            database,
            model.clone(),
            model.tier_model(),
            SystemPrompt {
                profile: Profile::default(),
                roster: Roster::default(),
                user: AboutUser::default(),
                memories: Vec::new(),
                memory_limit: 20,
            },
            vec![Box::new(Meanwhile {
                user: user.clone(),
                act,
            })],
            String::new(),
        )
        .await
        .unwrap();
        let mut replies = Vec::new();
        for prompt in prompts {
            let reply = session
                .send_steerable(prompt.to_string(), Vec::new(), &Unshown, &*user)
                .await;
            replies.push(reply);
            // A stop holds for one answer.
            *user.stopped.lock().unwrap() = false;
        }
        replies
    });
    (replies, model.sent())
}

#[test]
fn a_message_sent_while_a_tool_runs_joins_before_the_next_model_call() {
    let (replies, sent) = converse(
        vec![calling("lookup", &["c1"]), text("The blue one, then.")],
        Arc::default(),
        |user| user.says("Actually, the blue one."),
        &["Find my keys."],
    );
    assert_eq!(replies[0].as_ref().unwrap(), "The blue one, then.");
    let [_, second] = sent.as_slice() else {
        panic!("expected two model calls, got {}", sent.len());
    };
    // After the tool's result, as the model reads it.
    let last = second["messages"].as_array().unwrap().last().unwrap();
    assert_eq!(last["role"], "user");
    assert_eq!(last["content"], "Actually, the blue one.");
}

#[test]
fn a_message_waiting_as_the_answer_ends_gets_one_more_model_call() {
    let user = Arc::new(User::default());
    user.says("And my wallet?");
    let (replies, sent) = converse(
        vec![text("In the drawer."), text("On the table.")],
        user,
        |_| {},
        &["Where are my keys?"],
    );
    assert_eq!(replies[0].as_ref().unwrap(), "On the table.");
    assert_eq!(
        texts(&sent[1], "user"),
        ["Where are my keys?", "And my wallet?"]
    );
}

#[test]
fn a_stop_skips_the_calls_left_and_keeps_the_thread_whole() {
    let (replies, sent) = converse(
        vec![calling("lookup", &["c1", "c2"]), text("Okay.")],
        Arc::default(),
        |user| *user.stopped.lock().unwrap() = true,
        &["Look twice.", "Never mind."],
    );
    assert!(matches!(replies[0], Err(IntelligenceError::Stopped)));
    assert_eq!(replies[1].as_ref().unwrap(), "Okay.");
    let [_, next] = sent.as_slice() else {
        panic!("expected two model calls, got {}", sent.len());
    };
    // The call that ran keeps its result; the one the stop skipped is
    // answered too, so the thread reads whole.
    let results = texts(next, "tool");
    assert_eq!(results[0], "c1\nFound.");
    assert!(results[1].starts_with("c2\n"));
    assert_ne!(results[1], "c2\nFound.");
    assert_eq!(
        texts(next, "user").last().map(String::as_str),
        Some("Never mind.")
    );
}

/// Shows a session's activity nowhere.
struct Unshown;
impl ShowActivity for Unshown {
    fn show(&self, _activity: Activity) -> ShowFuture<'_> {
        Box::pin(async {})
    }
}

// A call marked `done` that succeeds ends the answer on its result: the
// model is asked for no final response, and the thread holds the result
// for the next prompt.
#[test]
fn a_call_marked_done_ends_the_answer_without_another_model_call() {
    let (replies, sent) = converse(
        vec![
            calling_with("lookup", json!({ "done": true })),
            text("Next one."),
        ],
        Arc::default(),
        |_| {},
        &["Find my keys.", "And my wallet?"],
    );
    assert_eq!(replies[0].as_ref().unwrap(), "");
    assert_eq!(replies[1].as_ref().unwrap(), "Next one.");
    let [_, second] = sent.as_slice() else {
        panic!("expected two model calls, got {}", sent.len());
    };
    let messages = second["messages"].as_array().unwrap();
    let roles: Vec<_> = messages
        .iter()
        .map(|m| m["role"].as_str().unwrap())
        .collect();
    assert_eq!(roles[roles.len() - 2..], ["tool", "user"]);
}

// What the user said while a call marked `done` ran still gets its model
// call: the answer ends only when nothing waits.
#[test]
fn a_message_sent_during_a_call_marked_done_keeps_the_answer_going() {
    let (replies, sent) = converse(
        vec![
            calling_with("lookup", json!({ "done": true })),
            text("The blue one, then."),
        ],
        Arc::default(),
        |user| user.says("Actually, the blue one."),
        &["Find my keys."],
    );
    assert_eq!(replies[0].as_ref().unwrap(), "The blue one, then.");
    assert_eq!(sent.len(), 2);
}
