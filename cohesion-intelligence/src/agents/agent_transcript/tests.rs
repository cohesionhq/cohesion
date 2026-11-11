use cohesion_core::model::{
    AgentAsk, AgentAskReply, AgentCodeExecution, AgentCodeOutcome, AgentForwardedItem,
    AgentForwardedItemKind, AgentMessageContent, AgentPeer, HookKind, IntelligenceAgentCue,
    IntelligenceAgentMessageSender, IntelligenceEffort,
};

use super::*;
use crate::agents::tests::{NOW, choice, message, now, run_note, text};

fn rendered() -> String {
    let execution = AgentCodeExecution {
        title: "Looking for overdue tasks".to_string(),
        code: "cohesion.workspaces()".to_string(),
        outcome: AgentCodeOutcome::Succeeded {
            result: format!("[{}]", "\"Write the report\",".repeat(40)),
        },
    };
    let cues = [IntelligenceAgentCue {
        id: 1,
        agent_id: 1,
        name: "Triage new tasks".to_string(),
        hook: HookKind::TaskCreate,
        script: "(task) => true".to_string(),
        instructions: "Check whether the new task is urgent.\nThen file it.".to_string(),
        effort: IntelligenceEffort::Low,
        set_at: NOW,
    }];
    let recent = vec![
        text(
            40,
            NOW - 120,
            IntelligenceAgentMessageSender::User,
            "Can you find overdue tasks?",
        ),
        message(
            41,
            NOW - 110,
            IntelligenceAgentMessageSender::Agent,
            AgentMessageContent::CodeExecution(execution),
        ),
        run_note(42),
        run_note(43),
        text(
            44,
            NOW,
            IntelligenceAgentMessageSender::User,
            "What did you find?",
        ),
    ];
    status_message(&cues, "Your agenda is empty.", &recent, Some(44), &now())
}

#[test]
fn status_message_ends_with_the_message_to_answer_and_holds_no_memories() {
    let rendered = rendered();
    let status = rendered.find("# Status").unwrap();
    let recent = rendered.find("# Recent conversation").unwrap();
    assert!(status < recent);
    assert!(!rendered.contains("# Memories"));
    let lines: Vec<_> = rendered.lines().collect();
    let [.., answered, "", answer] = lines.as_slice() else {
        panic!("the block ends with how to answer, after a blank line");
    };
    assert!(answered.starts_with("[#44]"));
    assert!(answered.contains("What did you find?"));
    assert!(answer.contains("[#44]"));
    assert!(answer.contains("send_message"));
    assert!(rendered.contains("Triage new tasks"));
}

// The agent's own text reads as a message it sent, not a reply to copy
// into its final response.
#[test]
fn the_agents_own_text_reads_as_sent() {
    let recent = vec![
        text(1, NOW - 60, IntelligenceAgentMessageSender::User, "Hi"),
        text(2, NOW - 30, IntelligenceAgentMessageSender::Agent, "Hello"),
    ];
    let rendered = status_message(&[], "Your agenda is empty.", &recent, None, &now());
    let line = rendered
        .lines()
        .find(|line| line.starts_with("[#2]"))
        .unwrap();
    assert!(line.starts_with("[#2] You sent ("));
    assert!(line.ends_with(": Hello"));
}

#[test]
fn catch_up_puts_the_message_under_a_heading_only_after_news() {
    assert_eq!(catch_up(None, &[], "[#50] Hi", None, &now()), "[#50] Hi");
    let news = [text(
        49,
        NOW - 60,
        IntelligenceAgentMessageSender::BackgroundAgent,
        "The build failed.",
    )];
    let prompt = catch_up(None, &news, "[#50] Hi", None, &now());
    assert!(prompt.starts_with("# Since your last reply\n[#49]"));
    assert!(prompt.ends_with("\n\n# User's message\n[#50] Hi"));
    assert!(!prompt.contains("# Memories"));
}

/// A script of a fork's in run `run_id`, ending in `outcome`.
fn fork_script(
    id: i64,
    run_id: i64,
    title: &str,
    outcome: AgentCodeOutcome,
) -> IntelligenceAgentMessage {
    let mut script = message(
        id,
        NOW - 60,
        IntelligenceAgentMessageSender::Agent,
        AgentMessageContent::CodeExecution(AgentCodeExecution {
            title: title.to_string(),
            code: "cohesion.workspaces()".to_string(),
            outcome,
        }),
    );
    script.run_id = Some(run_id);
    script
}

fn succeeded() -> AgentCodeOutcome {
    AgentCodeOutcome::Succeeded {
        result: "the whole output".to_string(),
    }
}

// A fork's finished scripts reach a catch-up by id and title, not their output,
// and forks running at once stay apart: every script still has its own id.
#[test]
fn a_catch_up_lists_forks_scripts_by_id_and_title() {
    let news = [
        fork_script(8214, 1, "Reading tasks", succeeded()),
        fork_script(8215, 1, "Reading sessions", succeeded()),
        fork_script(8216, 2, "Reading notes", succeeded()),
        fork_script(8217, 1, "Closing the task", succeeded()),
    ];
    let prompt = catch_up(None, &news, "[#50] Hi", None, &now());
    let lines: Vec<&str> = prompt
        .lines()
        .filter(|line| line.contains(" ran "))
        .collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].contains("2 scripts"));
    assert!(lines[0].contains("[#8214] \"Reading tasks\", [#8215] \"Reading sessions\""));
    assert!(lines[1].contains("1 script;") && lines[1].contains("[#8216] \"Reading notes\""));
    assert!(lines[2].contains("[#8217] \"Closing the task\""));
    assert!(!prompt.contains("the whole output"));
}

// `read_message` has no text for a failed script, so its line keeps the error.
#[test]
fn a_catch_up_keeps_a_failed_scripts_error() {
    let news = [
        fork_script(1, 1, "Reading tasks", succeeded()),
        fork_script(
            2,
            1,
            "Closing the task",
            AgentCodeOutcome::Failed {
                error: "no such task".to_string(),
            },
        ),
        fork_script(3, 1, "Reading sessions", succeeded()),
    ];
    let prompt = catch_up(None, &news, "[#50] Hi", None, &now());
    assert!(prompt.contains("failed: no such task"));
    assert!(prompt.contains("[#1] \"Reading tasks\""));
    assert!(prompt.contains("[#3] \"Reading sessions\""));
}

// Whatever a rebuild leaves out the agent can read again: a note and a failed script's
// error have text, as a finished script's output does.
#[test]
fn a_note_and_a_failed_scripts_error_can_be_read_back() {
    let note = AgentMessageContent::Note {
        note: "Task 7 is stalled.".to_string(),
    };
    assert_eq!(message_text(&note).unwrap(), "Task 7 is stalled.");
    let failed = fork_script(
        1,
        1,
        "Closing the task",
        AgentCodeOutcome::Failed {
            error: "no such task".to_string(),
        },
    );
    assert_eq!(
        message_text(&failed.message_content).unwrap(),
        "no such task"
    );
}

#[test]
fn a_forward_reads_as_the_note_then_the_items_link() {
    let item = AgentForwardedItem {
        kind: AgentForwardedItemKind::Journal,
        workspace_id: 3,
        id: 57,
        title: "Day [1]".to_string(),
    };
    let forward = |note: &str| {
        message(
            50,
            NOW,
            IntelligenceAgentMessageSender::User,
            AgentMessageContent::Forward {
                note: note.to_string(),
                item: item.clone(),
            },
        )
    };
    let reference = "Forwarded journal entry [Day \\[1\\]](cohesion://document/3/57).";
    assert_eq!(
        user_text(&forward("What did I promise?")),
        format!("[#50] What did I promise?\n\n{reference}")
    );
    assert_eq!(user_text(&forward("")), format!("[#50] {reference}"));
    let rendered = status_message(
        &[],
        "Your agenda is empty.",
        &[forward("Look")],
        None,
        &now(),
    );
    assert!(rendered.ends_with(&format!(": Look\n\n{reference}")));
}

#[test]
fn questions_between_agents_show_who_asked_and_the_answer() {
    let ask = |content| message(5, NOW, IntelligenceAgentMessageSender::Agent, content);
    let question = |reply| AgentAsk {
        agent: AgentPeer {
            id: 2,
            name: "App".to_string(),
        },
        question: "Is it green?".to_string(),
        reply,
    };
    let asked = line(
        &ask(AgentMessageContent::AskedAgent(question(
            AgentAskReply::Answered {
                text: "Green.".to_string(),
            },
        ))),
        &now(),
    );
    assert!(asked.ends_with("asked App (agent 2): Is it green?\n\nReply: Green."));
    let asked_by = line(
        &ask(AgentMessageContent::AskedByAgent(question(
            AgentAskReply::Waiting,
        ))),
        &now(),
    );
    assert!(asked_by.ends_with("App (agent 2) asked you: Is it green?\n\nStill being answered."));
}

// A long question between agents is excerpted as text is, and `read_message`
// reads the rest of it from where the excerpt stops.
#[test]
fn a_long_question_between_agents_is_excerpted_and_read_whole() {
    let content = AgentMessageContent::AskedAgent(AgentAsk {
        agent: AgentPeer {
            id: 2,
            name: "App".to_string(),
        },
        question: "q".repeat(1_000),
        reply: AgentAskReply::Answered {
            text: "Green.".to_string(),
        },
    });
    let rendered = line(
        &message(
            7,
            NOW,
            IntelligenceAgentMessageSender::Agent,
            content.clone(),
        ),
        &now(),
    );
    assert!(rendered.contains(&format!("asked App (agent 2): {}…", "q".repeat(800))));
    assert!(rendered.contains("`read_message` 7 from 800"));
    assert!(!rendered.contains("Green."));
    let text = message_text(&content).unwrap();
    let rest: String = text.chars().skip(800).collect();
    assert_eq!(rest, format!("{}\n\nReply: Green.", "q".repeat(200)));
}

#[test]
fn code_execution_shows_title_and_trimmed_result() {
    let rendered = rendered();
    let line = rendered
        .lines()
        .find(|line| line.starts_with("[#41]"))
        .unwrap();
    assert!(line.contains("Looking for overdue tasks"));
    assert!(line.contains("Write the report"));
    assert!(line.chars().count() < 300);
}

#[test]
fn a_runs_scripts_carry_their_code() {
    let execution = AgentCodeExecution {
        title: "Checking the new task".to_string(),
        code: "cohesion.workspace(1).tasks()".to_string(),
        outcome: AgentCodeOutcome::Succeeded {
            result: "[]".to_string(),
        },
    };
    let recent = vec![
        message(
            1,
            NOW - 30,
            IntelligenceAgentMessageSender::BackgroundAgent,
            AgentMessageContent::CodeExecution(execution),
        ),
        text(3, NOW, IntelligenceAgentMessageSender::User, "Do it"),
    ];
    let rendered = status_message(&[], "Your agenda is empty.", &recent, None, &now());
    let run = rendered.find("[#1]").unwrap();
    let user = rendered.find("[#3]").unwrap();
    let run_code = rendered.find("cohesion.workspace(1).tasks()").unwrap();
    assert!(run < run_code && run_code < user);
}

#[test]
fn background_messages_say_what_woke_their_run() {
    let rendered = rendered();
    let pending = rendered
        .lines()
        .find(|line| line.starts_with("[#43]"))
        .unwrap();
    assert!(pending.contains("after \"Test task\" was created"));
    let user = rendered
        .lines()
        .find(|line| line.starts_with("[#40]"))
        .unwrap();
    assert!(!user.contains("after"));
}

#[test]
fn failed_turns_show_as_failures() {
    let recent = vec![
        text(1, NOW - 60, IntelligenceAgentMessageSender::User, "Hello"),
        message(
            2,
            NOW - 30,
            IntelligenceAgentMessageSender::Agent,
            AgentMessageContent::Error {
                error: "The model's provider took too long to answer.".to_string(),
            },
        ),
    ];
    let rendered = status_message(&[], "Your agenda is empty.", &recent, None, &now());
    let line = rendered
        .lines()
        .find(|line| line.starts_with("[#2]"))
        .unwrap();
    assert!(line.contains("failed"));
    assert!(line.contains("took too long"));
}

// A pick posts no message, so a new thread knows it only from the choice.
#[test]
fn a_new_thread_reads_the_pick_and_context_from_the_choice() {
    let (pending, _) = choice(1, None);
    let (answered, _) = choice(2, Some(1));
    let rendered = status_message(
        &[],
        "Your agenda is empty.",
        &[pending, answered],
        None,
        &now(),
    );
    let pending = &rendered[rendered.find("[#1]").unwrap()..rendered.find("[#2]").unwrap()];
    assert!(pending.contains("unanswered"));
    assert!(pending.contains("Task 7 landed in Inbox"));
    let answered = &rendered[rendered.find("[#2]").unwrap()..];
    assert!(answered.contains("picked B. Errands"));
}

#[test]
fn a_pick_carries_its_question_and_the_runs_context() {
    let (message, choice) = choice(2, Some(0));
    let prompt = pick(&message, &choice);
    assert!(prompt.contains("A. Home"));
    assert!(prompt.contains("[#2]"));
    assert!(prompt.contains("Task 7 landed in Inbox"));
}

#[test]
fn a_long_message_is_excerpted_with_how_to_read_the_rest() {
    let short = text(5, NOW, IntelligenceAgentMessageSender::User, "Short one");
    assert!(line(&short, &now()).ends_with(": Short one"));

    let long = text(
        6,
        NOW,
        IntelligenceAgentMessageSender::Agent,
        &"é".repeat(2_000),
    );
    let rendered = line(&long, &now());
    assert!(rendered.contains(&"é".repeat(800)));
    assert!(!rendered.contains(&"é".repeat(801)));
    assert!(rendered.contains("1200 more characters"));
    assert!(rendered.contains("`read_message` 6 from 800"));
}
