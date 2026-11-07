//! A fake actor client for trying actors end to end: connects to
//! cohesiond's Unix socket, identifies itself as `name` (with a new local
//! key each run, so each run is a new client and revoking one doesn't stop
//! the name connecting again), and offers
//! two actors, "Fake Code" in two directories, the first reporting its
//! usage (two plan limits) once offered. Each session it is handed reports a
//! fifth of its context window used and, unless it was started with a
//! title, titles it by the prompt's first few words, then steps through
//! every state a session can be in, reporting as it goes:
//!
//! 1. Working for 10 seconds (reading, taking a screenshot, which it shows,
//!    then waiting on a lock; each tool it runs it reports ended), then a question, needing attention until it
//!    is answered.
//! 2. A command to allow, needing attention until decided: `abort` makes
//!    it idle.
//! 3. A plan to approve, needing attention until decided: not approved
//!    makes it idle.
//! 4. A summary, then idle.
//!
//! A message to an idle or failed session works on it again for 10 seconds
//! and echoes it, with any images it carried; `fail` makes it fail instead. Stopping a session from
//! Cohesion drops its work, and a message after picks it up again. Like any client, it doesn't reconnect once
//! cohesiond evicts it.
//!
//! `cargo run -p cohesiond --example fake_actor -- <socket> [name]`

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cohesionkit::model::{
    ActorDefinition, ActorEffort, ActorMessageContent, ActorMode, ActorModel,
    ActorPermissionDecision, ActorPermissionRequest, ActorPermissionSubject, ActorQuestion,
    ActorQuestionOption, ActorToolKind, ActorUserContent, ContextUsage, ReportedActorSessionStatus,
};
use cohesionkit::protocol::{
    ActorSessionReport, ActorSessionUpdate, ActorUsage, ActorUsageUpdate, CallResult, ClientCall,
    ClientCapabilities, ClientFrame, ClientHello, ClientImage, ClientOutcome, ClientResult, Frame,
    Outcome, PlanUsage, Reply, ReplyData, ServerMessage,
};
use cohesionkit::{LocalHandshakeReply, LocalIdentity};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

/// The `Hello` frame's id, which its reply carries.
const HELLO: u64 = 1;

/// How long the first stretch of a session, and of each message after it, runs.
const WORK: Duration = Duration::from_secs(10);

/// The pause between the steps after the first stretch.
const STEP: Duration = Duration::from_secs(3);

/// The actors offered: identifier and scope.
const ACTORS: [(&str, &str); 2] = [
    ("cohesion", "/home/fake/Developer/cohesion"),
    ("notes", "/home/fake/Documents/notes"),
];

/// The screenshot the first stretch shows: a 24-pixel checkerboard PNG.
const SCREENSHOT: &str = "iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAIAAABvFaqvAAAALklEQVR42mP4sMUGDWlU3EFDxKhhGDWIjgaRpw1TzahB9DRoNGWP5rVRg+hiEABy2ceuF9EnwQAAAABJRU5ErkJggg==";

/// What a session hears from the user: the message's id, what it says and the
/// images sent with it.
type Inbox = mpsc::UnboundedReceiver<(i64, ActorUserContent, Vec<ClientImage>)>;

/// A session being worked on: where what the user says goes, and its work.
struct Running {
    inbox: mpsc::UnboundedSender<(i64, ActorUserContent, Vec<ClientImage>)>,
    work: JoinHandle<()>,
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let Some(socket) = args.next() else {
        eprintln!("usage: fake_actor <socket> [name]");
        std::process::exit(2);
    };
    let name = args.next().unwrap_or_else(|| "Fake laptop".to_string());
    let stream = UnixStream::connect(&socket)
        .await
        .unwrap_or_else(|e| panic!("could not connect to {socket}: {e}"));
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    // The socket's handshake, before the session.
    let identity = LocalIdentity::generate(name.clone());
    writer.write_all(&line(&identity)).await.unwrap();
    let reply = lines.next_line().await.expect("could not read the socket");
    match reply.map(|line| serde_json::from_str::<LocalHandshakeReply>(&line)) {
        Some(Ok(LocalHandshakeReply::Accepted)) => {}
        other => panic!("cohesiond refused the handshake: {other:?}"),
    }

    // Every frame goes out through one queue, so sessions can report at once.
    let (out, mut queue) = mpsc::unbounded_channel::<Vec<u8>>();
    tokio::spawn(async move {
        while let Some(frame) = queue.recv().await {
            writer
                .write_all(&frame)
                .await
                .expect("could not write to the socket");
        }
    });
    let session = Session {
        out,
        replies: Arc::default(),
        next_id: Arc::new(AtomicU64::new(HELLO + 1)),
    };

    let hello = ClientFrame::Hello(ClientHello {
        id: HELLO,
        offers: ClientCapabilities {
            actors: ACTORS
                .iter()
                .map(|(identifier, scope)| actor(identifier, scope))
                .collect(),
        },
    });
    session.send(&hello);

    let mut sessions: HashMap<i64, Running> = HashMap::new();
    while let Some(text) = lines.next_line().await.expect("could not read the socket") {
        let frame: Frame = serde_json::from_str(&text).expect("cohesiond sent a bad frame");
        match frame.message {
            ServerMessage::Call { call_id, call } => {
                let outcome = match call {
                    ClientCall::StartActorSession {
                        actor,
                        session_id,
                        prompt,
                        title,
                        images,
                        mode,
                        model,
                        effort,
                    } => {
                        eprintln!(
                            "session {session_id} on {actor}: started as {title:?} ({mode:?}, {model:?}, {effort:?}, {} images): {prompt}",
                            images.len()
                        );
                        let scope = ACTORS
                            .iter()
                            .find(|(identifier, _)| *identifier == actor)
                            .map_or("/", |(_, scope)| scope);
                        let (inbox, heard) = mpsc::unbounded_channel();
                        let reporter = session.reporter(session_id);
                        let first = First {
                            prompt,
                            titled: title.is_some(),
                        };
                        let work = tokio::spawn(work(reporter, scope, Some(first), heard));
                        sessions.insert(session_id, Running { inbox, work });
                        ClientOutcome::Ok(ClientResult::Accepted)
                    }
                    ClientCall::SendActorSessionMessage {
                        actor,
                        session_id,
                        message_id,
                        content,
                        images,
                    } => {
                        eprintln!(
                            "session {session_id} on {actor}: heard message {message_id}, {} images",
                            images.len()
                        );
                        let running = sessions.entry(session_id).or_insert_with(|| {
                            // A session this run of the fake never started:
                            // pick it up from here.
                            let (inbox, heard) = mpsc::unbounded_channel();
                            let reporter = session.reporter(session_id);
                            let work = tokio::spawn(work(reporter, "/", None, heard));
                            Running { inbox, work }
                        });
                        let _ = running.inbox.send((message_id, content, images));
                        ClientOutcome::Ok(ClientResult::Accepted)
                    }
                    ClientCall::StopActorSession { actor, session_id } => {
                        eprintln!("session {session_id} on {actor}: stopped");
                        if let Some(running) = sessions.remove(&session_id) {
                            running.work.abort();
                        }
                        ClientOutcome::Ok(ClientResult::Accepted)
                    }
                    ClientCall::SetActorSessionMode {
                        actor,
                        session_id,
                        mode,
                    } => {
                        eprintln!("session {session_id} on {actor}: now in mode {mode}");
                        ClientOutcome::Ok(ClientResult::Accepted)
                    }
                };
                session.send(&ClientFrame::CallResult(CallResult { call_id, outcome }));
            }
            ServerMessage::Reply(Reply {
                id: Some(HELLO),
                outcome,
            }) => match outcome {
                Outcome::Ok(_) => {
                    eprintln!("serving as {name:?}");
                    session.send(&ClientFrame::ActorUsage(usage(&session)));
                }
                Outcome::Error(error) => {
                    eprintln!("hello refused: {}", error.message);
                    std::process::exit(1);
                }
            },
            ServerMessage::Reply(Reply {
                id: Some(id),
                outcome,
            }) => {
                let waiting = session.replies.lock().unwrap().remove(&id);
                if let Some(waiting) = waiting {
                    let _ = waiting.send(outcome);
                }
            }
            ServerMessage::Evicted { reason } => {
                eprintln!("evicted ({reason:?}); not reconnecting");
                return;
            }
            _ => {}
        }
    }
    eprintln!("cohesiond closed the session");
}

/// What the first actor has used: most of its five-hour limit.
fn usage(session: &Session) -> ActorUsageUpdate {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs() as i64);
    ActorUsageUpdate {
        id: session.next_id.fetch_add(1, Ordering::Relaxed),
        actor: ACTORS[0].0.to_string(),
        usage: ActorUsage {
            plan: vec![
                PlanUsage {
                    label: "5-hour limit".to_string(),
                    used: 0.72,
                    resets_at: Some(now + 2 * 3600),
                },
                PlanUsage {
                    label: "Weekly limit".to_string(),
                    used: 0.18,
                    resets_at: Some(now + 4 * 86_400),
                },
            ],
            pool: None,
        },
    }
}

fn actor(identifier: &str, scope: &str) -> ActorDefinition {
    let effort = |identifier: &str, title: &str| ActorEffort {
        identifier: identifier.to_string(),
        title: title.to_string(),
    };
    ActorDefinition {
        identifier: identifier.to_string(),
        product: "Fake Code".to_string(),
        scope: scope.to_string(),
        modes: vec![
            ActorMode {
                identifier: "default".to_string(),
                title: "Ask first".to_string(),
                description: Some("Asks before running commands or changing files.".to_string()),
            },
            ActorMode {
                identifier: "auto".to_string(),
                title: "Auto".to_string(),
                description: Some("Runs and edits on its own.".to_string()),
            },
            ActorMode {
                identifier: "plan".to_string(),
                title: "Plan".to_string(),
                description: Some("Plans, and changes nothing.".to_string()),
            },
        ],
        models: vec![
            ActorModel {
                identifier: "big".to_string(),
                title: "Big".to_string(),
                efforts: vec![
                    effort("low", "Low"),
                    effort("medium", "Medium"),
                    effort("high", "High"),
                ],
            },
            ActorModel {
                identifier: "small".to_string(),
                title: "Small".to_string(),
                efforts: Vec::new(),
            },
        ],
    }
}

/// The session's way out, and the updates waiting for their replies.
#[derive(Clone)]
struct Session {
    out: mpsc::UnboundedSender<Vec<u8>>,
    replies: Arc<Mutex<HashMap<u64, oneshot::Sender<Outcome>>>>,
    next_id: Arc<AtomicU64>,
}

impl Session {
    fn send(&self, frame: &impl serde::Serialize) {
        let _ = self.out.send(line(frame));
    }

    fn reporter(&self, session_id: i64) -> Reporter {
        Reporter {
            session: self.clone(),
            session_id,
        }
    }
}

/// Reports on one session.
struct Reporter {
    session: Session,
    session_id: i64,
}

impl Reporter {
    /// Sends `report` and waits for its reply.
    async fn report(&self, report: ActorSessionReport) -> Outcome {
        let id = self.session.next_id.fetch_add(1, Ordering::Relaxed);
        let (reply, replied) = oneshot::channel();
        self.session.replies.lock().unwrap().insert(id, reply);
        self.session
            .send(&ClientFrame::ActorSessionUpdate(ActorSessionUpdate {
                id,
                session_id: self.session_id,
                report,
            }));
        let outcome = replied.await.expect("the session ended");
        if let Outcome::Error(error) = &outcome {
            eprintln!(
                "session {}: update refused: {}",
                self.session_id, error.message
            );
        }
        outcome
    }

    /// Says `content`, answering the message's id.
    async fn say(&self, content: ActorMessageContent) -> i64 {
        self.show(content, Vec::new()).await
    }

    /// Says `content` with `images`, answering the message's id.
    async fn show(&self, content: ActorMessageContent, images: Vec<ClientImage>) -> i64 {
        match self
            .report(ActorSessionReport::Message { content, images })
            .await
        {
            Outcome::Ok(ReplyData::ActorSessionMessage(message)) => message.id,
            _ => -1,
        }
    }

    async fn text(&self, text: impl Into<String>) {
        self.say(ActorMessageContent::Text { text: text.into() })
            .await;
    }

    /// Starts running `tool`, answering its message's id for `end`.
    async fn tool(&self, tool: &str, category: ActorToolKind, summary: &str) -> i64 {
        let content = ActorMessageContent::ToolUse {
            tool: tool.to_string(),
            category,
            summary: summary.to_string(),
            ended_at: None,
        };
        self.say(content).await
    }

    /// Ends the tool or thinking whose message is `message_id`.
    async fn end(&self, message_id: i64) {
        self.report(ActorSessionReport::End { message_id }).await;
    }

    async fn status(&self, status: ReportedActorSessionStatus) {
        self.report(ActorSessionReport::Status { status }).await;
    }

    /// Says the session's context holds `used_tokens` of 200k.
    async fn context(&self, used_tokens: i64) {
        let context = ContextUsage {
            used_tokens,
            total_tokens: 200_000,
        };
        self.report(ActorSessionReport::Context { context }).await;
    }

    async fn title(&self, title: String) {
        self.report(ActorSessionReport::Title { title }).await;
    }

    async fn progress(&self, progress: Option<&str>) {
        let progress = progress.map(str::to_string);
        self.report(ActorSessionReport::Progress { progress }).await;
    }

    /// Makes request `content` and waits, needing attention, for the
    /// user's response to it; anything else they say meanwhile is put off.
    async fn ask(
        &self,
        content: ActorMessageContent,
        inbox: &mut Inbox,
    ) -> Option<ActorUserContent> {
        let request = self.say(content).await;
        self.status(ReportedActorSessionStatus::NeedsAttention)
            .await;
        while let Some((_, content, _)) = inbox.recv().await {
            match content.responds_to() {
                Some((request_id, _)) if request_id == request => {
                    self.status(ReportedActorSessionStatus::Working).await;
                    return Some(content);
                }
                _ => {
                    self.text("I'll get to that once you've answered.").await;
                    self.status(ReportedActorSessionStatus::NeedsAttention)
                        .await;
                }
            }
        }
        None
    }
}

/// A session's work: the first stretch for `prompt`, when it has one, then a
/// stretch for each message after.
/// What a session was started with.
struct First {
    prompt: String,
    /// Whether it was started with a title, which the fake then keeps.
    titled: bool,
}

async fn work(reporter: Reporter, scope: &str, first: Option<First>, mut inbox: Inbox) {
    if let Some(First { prompt, titled }) = first {
        if !titled {
            let words: Vec<&str> = prompt.split_whitespace().take(4).collect();
            reporter.title(format!("Fake: {}", words.join(" "))).await;
        }
        first_stretch(&reporter, scope, &prompt, &mut inbox).await;
        reporter.status(ReportedActorSessionStatus::Idle).await;
    }
    while let Some((_, content, images)) = inbox.recv().await {
        let ActorUserContent::Text { text } = content else {
            // A response to a request no longer waiting.
            continue;
        };
        match text.trim() {
            "fail" => {
                reporter.status(ReportedActorSessionStatus::Working).await;
                reporter
                    .text("Something went wrong, and I can't go on.")
                    .await;
                reporter.status(ReportedActorSessionStatus::Failed).await;
            }
            _ => {
                reporter.status(ReportedActorSessionStatus::Working).await;
                reporter.progress(Some("Thinking it over")).await;
                tokio::time::sleep(WORK).await;
                let said = ActorMessageContent::Text {
                    text: format!("You said:\n\n> {text}"),
                };
                reporter.show(said, images).await;
                reporter.status(ReportedActorSessionStatus::Idle).await;
            }
        }
    }
}

/// Every state a working session can be in: working, waiting on a lock, then
/// needing attention for a question, a command and a plan.
async fn first_stretch(reporter: &Reporter, scope: &str, prompt: &str, inbox: &mut Inbox) {
    reporter.status(ReportedActorSessionStatus::Working).await;
    reporter.context(41_800).await;
    reporter.progress(Some("Reading the session")).await;
    reporter
        .text(format!("On it: *{prompt}*. I'll look around first."))
        .await;
    tokio::time::sleep(WORK * 3 / 10).await;
    let thinking = reporter
        .say(ActorMessageContent::Thinking {
            text: "The session names no file, so start from `src/lib.rs` and follow its imports."
                .to_string(),
            ended_at: None,
        })
        .await;
    tokio::time::sleep(WORK * 2 / 10).await;
    reporter.end(thinking).await;
    let read = reporter
        .tool("Read", ActorToolKind::Read, "src/lib.rs")
        .await;
    reporter.progress(Some("Reading src/lib.rs")).await;
    tokio::time::sleep(WORK * 3 / 10).await;
    reporter.end(read).await;
    let screenshot = ClientImage {
        media_type: "image/png".to_string(),
        data: SCREENSHOT.to_string(),
    };
    let tool = ActorMessageContent::ToolUse {
        tool: "Screenshot".to_string(),
        category: ActorToolKind::Other,
        summary: "the app's window".to_string(),
        ended_at: None,
    };
    let screenshot = reporter.show(tool, vec![screenshot]).await;
    reporter.end(screenshot).await;
    reporter
        .progress(Some("Waiting for cargo's build lock"))
        .await;
    tokio::time::sleep(WORK * 4 / 10).await;
    reporter.progress(None).await;

    let question = ActorMessageContent::Question {
        questions: vec![
            ActorQuestion {
                header: Some("Approach".to_string()),
                question: "How should I fix it?".to_string(),
                options: vec![
                    option("Patch it", "The smallest change that works."),
                    option("Refactor", "Clean up the module while I'm there."),
                ],
                multi_select: false,
                allows_other: true,
            },
            ActorQuestion {
                header: Some("Checks".to_string()),
                question: "Which checks should I run after?".to_string(),
                options: vec![
                    option("Tests", "cargo test"),
                    option("Clippy", "cargo clippy"),
                    option("Format", "cargo fmt"),
                ],
                multi_select: true,
                allows_other: false,
            },
        ],
    };
    let Some(ActorUserContent::Answers { answers, .. }) = reporter.ask(question, inbox).await
    else {
        return;
    };
    let picked: Vec<String> = answers
        .iter()
        .map(|answer| {
            let mut picked = answer.selected.join(", ");
            if let Some(other) = &answer.other {
                picked = format!("{picked} {other}").trim().to_string();
            }
            picked
        })
        .collect();
    reporter
        .text(format!("Got it: {}.", picked.join("; ")))
        .await;
    reporter.progress(Some("Getting ready to test")).await;
    tokio::time::sleep(STEP).await;

    let permission = ActorMessageContent::Permission(ActorPermissionRequest {
        subject: ActorPermissionSubject::Command {
            command: "cargo test --workspace".to_string(),
            cwd: Some(scope.to_string()),
        },
        reason: Some("To check the fix before I go on.".to_string()),
        decisions: vec![
            ActorPermissionDecision::Allow,
            ActorPermissionDecision::AllowForSession,
            ActorPermissionDecision::Deny,
            ActorPermissionDecision::Abort,
        ],
    });
    let Some(ActorUserContent::Permission {
        decision, message, ..
    }) = reporter.ask(permission, inbox).await
    else {
        return;
    };
    let note = message
        .map(|message| format!(" You said: {message}"))
        .unwrap_or_default();
    match decision {
        ActorPermissionDecision::Abort => {
            reporter.text(format!("Stopping here.{note}")).await;
            return;
        }
        ActorPermissionDecision::Deny => {
            reporter.text(format!("I'll skip the tests.{note}")).await;
        }
        ActorPermissionDecision::Allow | ActorPermissionDecision::AllowForSession => {
            let tests = reporter
                .tool("Bash", ActorToolKind::Command, "cargo test --workspace")
                .await;
            reporter.progress(Some("Running the tests")).await;
            tokio::time::sleep(STEP).await;
            reporter.end(tests).await;
            reporter.text("All 412 tests pass.").await;
        }
    }

    let plan = ActorMessageContent::Plan {
        plan: "## Plan\n\n1. Fix the off-by-one in `src/lib.rs`.\n2. Add a test for the empty case.\n3. Run the checks you picked."
            .to_string(),
    };
    let Some(ActorUserContent::Plan {
        approved, feedback, ..
    }) = reporter.ask(plan, inbox).await
    else {
        return;
    };
    if !approved {
        let feedback = feedback.unwrap_or_else(|| "no notes".to_string());
        reporter
            .text(format!("I'll rethink the plan ({feedback})."))
            .await;
        return;
    }
    let change = reporter
        .tool("Edit", ActorToolKind::Edit, "src/lib.rs")
        .await;
    reporter.progress(Some("Making the change")).await;
    tokio::time::sleep(STEP).await;
    reporter.end(change).await;
    let test = reporter
        .tool("Edit", ActorToolKind::Edit, "src/tests.rs")
        .await;
    reporter.end(test).await;
    reporter
        .text("Done. The off-by-one is fixed and the empty case has a test. Send `fail` to see the session fail, or anything else to keep going.")
        .await;
}

fn option(label: &str, description: &str) -> ActorQuestionOption {
    ActorQuestionOption {
        label: label.to_string(),
        description: Some(description.to_string()),
    }
}

/// `frame` as one line of JSON.
fn line(frame: &impl serde::Serialize) -> Vec<u8> {
    let mut text = serde_json::to_string(frame).unwrap();
    text.push('\n');
    text.into_bytes()
}
