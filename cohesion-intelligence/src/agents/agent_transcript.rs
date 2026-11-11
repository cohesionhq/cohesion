//! The compact text an agent's model reads its conversation from: the
//! status message that opens every foreground thread (which a fork starting
//! a thread of its own splits into the recent conversation, shared, then
//! the time and cues), the catch-up that
//! precedes a user message when forks posted in the meantime, the
//! prompts a pick of a choice and a reaction send, what opens a
//! fork of the conversation (a question, an actor session it follows, or a task
//! assigned to it), and
//! the wake-up message of a fork. The memories an agent sees are
//! in its system prompt, not here; a live thread's catch-up only says which
//! changed since.

use cohesion_core::model::{
    ActorSession, ActorSessionStop, AgentAgendaChange, AgentAsk, AgentAskReply, AgentChoice,
    AgentCodeExecution, AgentCodeOutcome, AgentForwardedItem, AgentMemoryChange,
    AgentMessageContent, AgentPeer, AgentWake, Initiative, IntelligenceAgentCue,
    IntelligenceAgentMessage, IntelligenceAgentMessageSender, Task,
};
use cohesion_core::protocol::actor_session_stop_label;
use cohesion_js::ScriptOutput;
use jiff::{Timestamp, Zoned};

use super::actor_exchange::SessionExchange;
use super::agent_messenger::AskMode;

/// How many characters of a script's result or error a line keeps.
const PREVIEW_CHARS: usize = 200;

/// How many characters of a message's text its line in a transcript keeps;
/// `read_message` returns the rest.
const EXCERPT_CHARS: usize = 800;

/// The first user turn of a foreground thread: the time and the agent's
/// cues, and the recent conversation, oldest first, whose last line is the
/// message to answer, `answering`, when the turn answers one of the user's,
/// followed by how to answer it, since the lines above read as a chat the
/// model might otherwise carry on in its final reply.
pub(super) fn status_message(
    cues: &[IntelligenceAgentCue],
    agenda: &str,
    recent: &[IntelligenceAgentMessage],
    answering: Option<i64>,
    now: &Zoned,
) -> String {
    let answer = match answering {
        Some(id) => format!("\n\nAnswer [#{id}] with `send_message` or `react`."),
        None => String::new(),
    };
    format!(
        "{}\n\n{}{answer}",
        status(cues, agenda, now),
        recent_conversation(recent, now)
    )
}

/// The time, the agent's cues and `agenda`, its agenda as the thread hears
/// it: what a fork's opening leaves out, since the time changes from one
/// fork to the next and the cues and agenda do as they are set.
pub(super) fn status(cues: &[IntelligenceAgentCue], agenda: &str, now: &Zoned) -> String {
    let cues = match cues {
        [] => " none".to_string(),
        cues => cues
            .iter()
            .map(|cue| {
                format!(
                    "\n- \"{}\": {} at {} effort — \"{}\"",
                    cue.name,
                    cue.hook,
                    cue.effort,
                    cue.instructions.lines().next().unwrap_or_default()
                )
            })
            .collect(),
    };
    format!("{}\nYour cues:{cues}\n{agenda}", time(now))
}

/// The time alone, which a cue's background run opens with.
pub(super) fn time(now: &Zoned) -> String {
    format!(
        "# Status\nCurrent time: {} (local)",
        now.strftime("%A %Y-%m-%d %H:%M")
    )
}

/// The recent conversation, oldest first: what a fork that starts a thread
/// of its own opens it with, which the agent's later forks copy.
pub(super) fn recent_conversation(recent: &[IntelligenceAgentMessage], now: &Zoned) -> String {
    format!("# Recent conversation\n{}", lines(recent, now))
}

/// A user message (or, under `heading`, the text of a pick or a reaction)
/// to a live thread, preceded by `memories`, the section saying which
/// memories changed since the thread last heard, if any did, and what
/// forks posted since the
/// model last looked.
pub(super) fn catch_up(
    memories: Option<&str>,
    news: &[IntelligenceAgentMessage],
    text: &str,
    heading: Option<&str>,
    now: &Zoned,
) -> String {
    let mut sections: Vec<String> = memories.into_iter().map(str::to_string).collect();
    if !news.is_empty() {
        sections.push(format!(
            "# Since your last reply\n{}",
            news_lines(news, now)
        ));
    }
    match (sections.is_empty(), heading) {
        (true, None) => text.to_string(),
        (true, Some(heading)) => format!("# {heading}\n{text}"),
        (false, heading) => {
            sections.push(format!("# {}\n{text}", heading.unwrap_or("User's message")));
            sections.join("\n\n")
        }
    }
}

/// The text of the user's message `message` as a live thread reads it,
/// after its id (so the agent can react to it) and noting the images that
/// follow it: `[#42] Thanks!`. A forward's note is followed by the item's
/// link, which the agent reads it from.
pub(super) fn user_text(message: &IntelligenceAgentMessage) -> String {
    let id = message.id;
    let text = match &message.message_content {
        AgentMessageContent::Text { text } => text.clone(),
        AgentMessageContent::Forward { note, item } => forwarded(note, item),
        _ => return format!("[#{id}]"),
    };
    match (text.as_str(), message.attachment_count) {
        (text, 0) => format!("[#{id}] {text}"),
        ("", count) => format!("[#{id}] ({} attached)", images(count)),
        (text, count) => format!("[#{id}] {text}\n({} attached)", images(count)),
    }
}

/// A forward as the agent reads it: the user's note, then the item's
/// reference on a paragraph of its own.
fn forwarded(note: &str, item: &AgentForwardedItem) -> String {
    match note {
        "" => item.reference(),
        note => format!("{note}\n\n{}", item.reference()),
    }
}

/// The user's message `message` as it joins a turn already running:
/// [`user_text`] under a heading saying it came while the agent worked.
pub(super) fn steered(message: &IntelligenceAgentMessage) -> String {
    format!(
        "# User's message, sent while you were working\n{}",
        user_text(message)
    )
}

/// What a turn that answers the user hears once when it ends having sent
/// them nothing, since its final reply never reaches them.
pub(super) const UNANSWERED: &str = "# Nothing sent\nThe user saw nothing: your final response is never shown to them. Call `send_message` now with what they should hear, or `react` to their message when nothing needs saying.";

/// The prompt of a turn the user's reaction to one of the agent's messages
/// starts: `The user reacted 👍 to [#12] "Done."`, quoting its excerpt.
pub(super) fn reaction(message: &IntelligenceAgentMessage) -> String {
    let reaction = message.reaction.as_deref().expect("a reaction is set");
    format!(
        "The user reacted {reaction} to [#{}] \"{}\".",
        message.id, message.excerpt
    )
}

/// `1 image` or `3 images`.
fn images(count: i64) -> String {
    match count {
        1 => "1 image".to_string(),
        count => format!("{count} images"),
    }
}

/// The prompt of a turn the user's pick in the answered `choice` starts,
/// in place of a message of theirs: the question and the pick, with what
/// the fork that asked noted for the agent.
pub(super) fn pick(message: &IntelligenceAgentMessage, choice: &AgentChoice) -> String {
    let selected = choice.selected.expect("a pick is answered");
    let pick = format!(
        "The user picked \"{}\" in answer to [#{}] \"{}\".",
        choice.lettered(selected),
        message.id,
        choice.question
    );
    match &choice.context {
        Some(context) => format!("{pick}\nContext from the fork that asked: {context}"),
        None => pick,
    }
}

/// The heading and text of the fork `cue` wakes, in place of a message of
/// the user's: the cue's instructions and the `payload` its function
/// returned.
pub(super) fn cue_fired(cue: &IntelligenceAgentCue, payload: &ScriptOutput) -> (String, String) {
    (
        format!("Cue \"{}\" fired", cue.name),
        format!(
            "Your cue \"{}\" fired on hook `{}` with:\n```json\n{}\n```\n\n\
             Your instructions for it: {}\n\nYou are carrying them out in a background run, \
             which the user doesn't see and which starts fresh: your conversation and notes \
             aren't in it, so work from these instructions, your memories, `storage` and the \
             user's data. Message the user only what deserves their attention; your final \
             reply goes to no one.",
            cue.name, cue.hook, payload.0, cue.instructions
        ),
    )
}

/// The heading and text of a fork's only turn, `asker`'s `question`, in
/// place of a message of the user's, asked in `mode`.
pub(super) fn question(asker: &AgentPeer, question: &str, mode: AskMode) -> (String, String) {
    let answering = match mode {
        AskMode::Reply => {
            "It waits for your reply: answer the question, or do what it asks, then call \
             `reply_to_agent` with what it asked for. Your final response reaches no one."
        }
        AskMode::OneWay => {
            "It wants no reply: only perform the task, then respond with OK, or set `done` on \
             your last call. If it asks you to tell it when you are done, do so by asking it \
             with `ask_agent`, `reply` false."
        }
    };
    (
        format!("{} asks", asker.name),
        format!(
            "{question}\n\n{} (agent {}) asked you this. You are answering in a fork of your \
             conversation, which the user doesn't see. {answering} The user said \
             nothing: `send_message` them only what deserves their attention, or what they \
             asked you to tell them about.",
            asker.name, asker.id
        ),
    )
}

/// The heading and text of a fork's only turn when `task` was just
/// assigned to the agent by someone else, in place of a message of the
/// user's: the task, linked, and its description, as the go-ahead to work
/// on it.
pub(super) fn assigned_task(task: &Task) -> (String, String) {
    let (id, workspace_id) = (task.id, task.workspace_id);
    let description = match task.description.trim() {
        "" => "It has no description.".to_string(),
        description => format!("Its description:\n\n{description}"),
    };
    (
        format!("Task \"{}\" assigned to you", task.title),
        format!(
            "Task {id} in workspace {workspace_id}, [{}](cohesion://task/{workspace_id}/{id}), \
             was just assigned to you. {description}\n\n\
             Being assigned a task is the go-ahead to work on it: do it, or its next step, in \
             this fork of your conversation, which the user doesn't see. `send_message` the \
             user only what deserves their attention, such as the result or a question you \
             need answered. Your final reply goes to no one.",
            task.title
        ),
    )
}

/// What a fork that owes another agent a reply hears once when it ends
/// without giving it, since its final response reaches no one.
pub(super) const UNREPLIED: &str = "# No reply sent\nThe agent that asked is still waiting: your final response never reaches it. Call `reply_to_agent` now with your answer, or with why you can't give one.";

/// The heading and text of a fork's only turn when `actor_session`, an actor session
/// the agent follows, has moved to its status, in place of a message of the
/// user's: the actor_session, linked, where it stands and, from `exchange`, what the
/// conversation told it at its start and what its actor said since it was
/// last told something, or that its transcript couldn't be read.
pub(super) fn actor_session(
    actor_session: &ActorSession,
    stop: ActorSessionStop,
    exchange: Option<&SessionExchange>,
) -> (String, String) {
    let id = actor_session.id;
    let standing = match stop {
        ActorSessionStop::NeedsYou => "needs the user, such as for an answer to a request",
        ActorSessionStop::Finished => "finished its turn",
        ActorSessionStop::Failed => "failed",
        ActorSessionStop::Interrupted => {
            "was interrupted: its device went away mid-turn, so it is idle with the work unfinished"
        }
    };
    let exchange = match exchange {
        Some(SessionExchange {
            prompt: Some(prompt),
            said,
        }) => format!(
            "You told it: \"{}\"\n\nWhat the actor said since it was last told something: {said}",
            told(prompt, id)
        ),
        Some(SessionExchange { prompt: None, said }) => {
            format!("What the actor said since it was last told something: {said}")
        }
        None => format!("Its transcript could not be read; `actors.messages({id})` reads it."),
    };
    (
        format!("Actor session \"{}\"", actor_session.title),
        format!(
            "Actor session {id}, [{}](cohesion://actor-session/{id}), {standing}. {exchange}\n\n\
             You follow this actor_session: your conversation handed it work, and you are carrying that \
             work on in a fork of your conversation, which the user doesn't see. Do what comes \
             next, such as dispatching the next actor_session or answering this one as the user \
             decided. The user said nothing: `send_message` them only what \
             deserves their attention, or what they asked you to tell them about, replying to \
             the message the work follows up on. Your final reply goes to no one.",
            actor_session.title
        ),
    )
}

/// What a fork may do, said after every fork's opening, by who started
/// its chain: a fork the user's work leads to has the conversation's rights,
/// one an event (a cue) started only reads.
pub(super) fn fork_rights(initiative: Initiative) -> &'static str {
    match initiative {
        Initiative::User => {
            "This fork has everything a turn of your conversation has: read and change the \
             user's data as a turn would, and finish what this calls for yourself."
        }
        Initiative::Event => {
            "This turn was started by your cue, not the user, so in it you can read but not \
             change anything: writes throw, there is no `network`, and you can't set cues or \
             change memories. To change something, ask the user with `ask_choice`, putting in \
             `context` what to do on each answer; their pick starts a turn that can. Note \
             where the work stands with `send_note`, and keep your agenda up to date."
        }
    }
}

/// `prompt`, what the conversation told actor session `actor_session_id` at its start,
/// as a follow-up quotes it: whole within [`EXCERPT_CHARS`], else its start
/// and a note of how much was left out and how to read it.
fn told(prompt: &str, actor_session_id: i64) -> String {
    let Some((end, _)) = prompt.char_indices().nth(EXCERPT_CHARS) else {
        return prompt.to_string();
    };
    let omitted = prompt[end..].chars().count();
    format!(
        "{}… [{omitted} more characters; `actors.messages({actor_session_id})` has the rest]",
        &prompt[..end]
    )
}

fn lines(messages: &[IntelligenceAgentMessage], now: &Zoned) -> String {
    if messages.is_empty() {
        return "(none)".to_string();
    }
    messages
        .iter()
        .map(|message| line(message, now))
        .collect::<Vec<_>>()
        .join("\n")
}

/// What forks posted, as a catch-up tells it: [`line`] for each, except that
/// the scripts a run finished in a row share one, by title, since they would
/// fill a long-lived thread and `read_message` has each one's output.
fn news_lines(messages: &[IntelligenceAgentMessage], now: &Zoned) -> String {
    messages
        .chunk_by(|a, b| a.run_id == b.run_id && is_listed_script(a) && is_listed_script(b))
        .map(|group| match is_listed_script(&group[0]) {
            true => listed_scripts(group, now),
            false => line(&group[0], now),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether a catch-up lists `message` by title: a script of the agent's that
/// finished, whose output `read_message` reads. A failure's text is only in its
/// line, and a cue run's line holds its code for the agent to reuse.
fn is_listed_script(message: &IntelligenceAgentMessage) -> bool {
    message.sender == IntelligenceAgentMessageSender::Agent
        && matches!(
            message.message_content,
            AgentMessageContent::CodeExecution(AgentCodeExecution {
                outcome: AgentCodeOutcome::Succeeded { .. },
                ..
            })
        )
}

/// One line for scripts a run finished in a row, each `[#id] "title"`, under
/// the first's time and what woke the run.
fn listed_scripts(scripts: &[IntelligenceAgentMessage], now: &Zoned) -> String {
    let first = &scripts[0];
    let mut notes = vec![sent_time(first.sent_at, now)];
    notes.extend(
        first
            .run_description
            .as_ref()
            .map(|description| format!("after {description}")),
    );
    let titles = scripts
        .iter()
        .filter_map(|script| match &script.message_content {
            AgentMessageContent::CodeExecution(execution) => {
                Some(format!("[#{}] \"{}\"", script.id, execution.title))
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(", ");
    let count = match scripts.len() {
        1 => "1 script".to_string(),
        count => format!("{count} scripts"),
    };
    format!(
        "[#{}] You ({}) ran {count}; `read_message` has the output: {titles}",
        first.id,
        notes.join(", ")
    )
}

/// One message: `[#id] Sender (time)` and what it said or did, the
/// agent's own text as `[#id] You sent (time): …` so it reads as a message
/// sent with `send_message`, not a reply to copy; a
/// fork's message's time is followed by what woke the fork, e.g.
/// `(10:29, after "Test task" was created)`, then the message it replies
/// to (`replying to [#12]`), then any images attached and
/// the reaction to it, e.g. `(10:31, 2 images attached, you reacted 👍)`. An
/// older cue run's scripts follow their line in full, so the agent can reuse or rework
/// them; its own runs are in its thread already, or summed up by title.
/// A message's, a forward's or a question between agents' text (with where
/// the question stands) keeps only its excerpt, a script's result a preview.
pub(super) fn line(message: &IntelligenceAgentMessage, now: &Zoned) -> String {
    let head = head(message, now);
    match &message.message_content {
        AgentMessageContent::Text { .. } | AgentMessageContent::Forward { .. } => {
            let text = message_text(&message.message_content).expect("it has text");
            format!("{head}: {}", excerpt(&text, message.id, EXCERPT_CHARS))
        }
        AgentMessageContent::Error { error } => {
            format!("{head} failed: {}", preview(error))
        }
        AgentMessageContent::Documentation { module } => {
            format!("{head} loaded the `{module}` module")
        }
        AgentMessageContent::Memory {
            memory,
            change: AgentMemoryChange::Remembered,
        } => format!("{head} remembered \"{memory}\""),
        AgentMessageContent::Memory {
            memory,
            change: AgentMemoryChange::Forgotten,
        } => format!("{head} forgot \"{memory}\""),
        AgentMessageContent::CodeExecution(execution) => {
            let outcome = match &execution.outcome {
                AgentCodeOutcome::Succeeded { result } => preview(result),
                AgentCodeOutcome::Failed { error } => format!("failed: {}", preview(error)),
                AgentCodeOutcome::Cancelled => "stopped by the user".to_string(),
            };
            let line = format!("{head} ran \"{}\" → {outcome}", execution.title);
            match message.sender {
                IntelligenceAgentMessageSender::BackgroundAgent => {
                    format!("{line}{}", code_block(&execution.code))
                }
                _ => line,
            }
        }
        AgentMessageContent::Choice(choice) => {
            let options = (0..choice.options.len())
                .map(|index| choice.lettered(index))
                .collect::<Vec<_>>()
                .join("; ");
            let status = match choice.selected {
                Some(index) => format!("picked {}", choice.lettered(index)),
                None => "unanswered".to_string(),
            };
            let line = format!(
                "{head} asked \"{}\" with options {options} [{status}]",
                choice.question
            );
            match &choice.context {
                Some(context) => format!("{line}\nContext from the run: {context}"),
                None => line,
            }
        }
        AgentMessageContent::AskedAgent(ask) => format!(
            "{head} asked {} (agent {}): {}",
            ask.agent.name,
            ask.agent.id,
            excerpt(&ask_text(ask), message.id, EXCERPT_CHARS)
        ),
        AgentMessageContent::AskedByAgent(ask) => format!(
            "{head} {} (agent {}) asked you: {}",
            ask.agent.name,
            ask.agent.id,
            excerpt(&ask_text(ask), message.id, EXCERPT_CHARS)
        ),
        AgentMessageContent::Note { note } => format!("{head} noted: {note}"),
        AgentMessageContent::Woke {
            by: AgentWake::Cue { cue_name, hook },
        } => {
            format!("{head} woke: your cue \"{cue_name}\" fired on {hook}")
        }
        AgentMessageContent::Woke {
            by:
                AgentWake::ActorSession {
                    actor_session_id,
                    title,
                    stop,
                },
        } => format!(
            "{head} woke: actor session {actor_session_id} (\"{title}\") {}",
            actor_session_stop_label(*stop)
        ),
        AgentMessageContent::Woke {
            by:
                AgentWake::AssignedTask {
                    workspace_id,
                    task_id,
                    title,
                },
        } => format!(
            "{head} woke: task \"{title}\" (cohesion://task/{workspace_id}/{task_id}) was assigned \
             to you"
        ),
        AgentMessageContent::RebuiltContext { .. } => {
            format!("{head} rebuilt context: a fresh thread started from these lines")
        }
        AgentMessageContent::Agenda(update) => {
            let changed = update.changed_text();
            match update.change {
                AgentAgendaChange::Appended => format!("{head} added to your agenda: {changed}"),
                AgentAgendaChange::Finished => {
                    format!("{head} finished an item of your agenda: {changed}")
                }
                AgentAgendaChange::Reset => format!("{head} cleared your agenda"),
            }
        }
    }
}

/// The text of a message that has some, whole: what its line excerpts and
/// `read_message` reads from.
pub(super) fn message_text(content: &AgentMessageContent) -> Option<String> {
    match content {
        AgentMessageContent::Text { text } => Some(text.clone()),
        AgentMessageContent::Forward { note, item } => Some(forwarded(note, item)),
        AgentMessageContent::AskedAgent(ask) | AgentMessageContent::AskedByAgent(ask) => {
            Some(ask_text(ask))
        }
        AgentMessageContent::CodeExecution(AgentCodeExecution {
            outcome:
                AgentCodeOutcome::Succeeded { result: text } | AgentCodeOutcome::Failed { error: text },
            ..
        }) => Some(text.clone()),
        AgentMessageContent::Note { note } => Some(note.clone()),
        _ => None,
    }
}

/// A question between agents as text: the question, then after a blank
/// line where it stands, `Reply: …`, `Unanswered: …` or that it is still
/// being answered.
fn ask_text(ask: &AgentAsk) -> String {
    let standing = match &ask.reply {
        AgentAskReply::Waiting => "Still being answered.".to_string(),
        AgentAskReply::Answered { text } => format!("Reply: {text}"),
        AgentAskReply::Unanswered { reason } => format!("Unanswered: {reason}"),
    };
    format!("{}\n\n{standing}", ask.question)
}

/// Who a line says sent `message`.
fn sender(message: &IntelligenceAgentMessage) -> &'static str {
    match message.sender {
        IntelligenceAgentMessageSender::User => "User",
        IntelligenceAgentMessageSender::Agent => match message.message_content {
            AgentMessageContent::Text { .. } | AgentMessageContent::Forward { .. } => "You sent",
            _ => "You",
        },
        IntelligenceAgentMessageSender::BackgroundAgent => "Job",
    }
}

/// What a [`line`] opens with: `[#id] Sender (time, notes)`.
fn head(message: &IntelligenceAgentMessage, now: &Zoned) -> String {
    let mut notes = vec![sent_time(message.sent_at, now)];
    if let Some(description) = &message.run_description {
        notes.push(format!("after {description}"));
    }
    if let Some(reply_to) = message.reply_to {
        notes.push(format!("replying to [#{reply_to}]"));
    }
    if message.attachment_count > 0 {
        notes.push(format!("{} attached", images(message.attachment_count)));
    }
    if let Some(reaction) = &message.reaction {
        notes.push(match message.sender {
            IntelligenceAgentMessageSender::User => format!("you reacted {reaction}"),
            _ => format!("the user reacted {reaction}"),
        });
    }
    format!(
        "[#{}] {} ({})",
        message.id,
        sender(message),
        notes.join(", ")
    )
}

/// `text` of message `id` cut to its first `keep` characters: whole when it
/// is within them, else its start and a note of how much was left out and
/// how to read it.
pub(super) fn excerpt(text: &str, id: i64, keep: usize) -> String {
    let Some((end, _)) = text.char_indices().nth(keep) else {
        return text.to_string();
    };
    let omitted = text[end..].chars().count();
    format!(
        "{}… [{omitted} more characters; `read_message` {id} from {keep} for the rest]",
        &text[..end]
    )
}

/// `code` fenced on the lines after a message's.
fn code_block(code: &str) -> String {
    format!("\n```js\n{}\n```", code.trim_end())
}

/// `HH:MM` for today, else `Mon D, HH:MM`, in `now`'s time zone.
fn sent_time(sent_at: i64, now: &Zoned) -> String {
    let sent = Timestamp::from_second(sent_at)
        .expect("a stored timestamp is in range")
        .to_zoned(now.time_zone().clone());
    if sent.date() == now.date() {
        sent.strftime("%H:%M").to_string()
    } else {
        sent.strftime("%b %-d, %H:%M").to_string()
    }
}

/// `text` on one line, cut to [`PREVIEW_CHARS`].
fn preview(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match flat.char_indices().nth(PREVIEW_CHARS) {
        Some((end, _)) => format!("{}…", &flat[..end]),
        None => flat,
    }
}

#[cfg(test)]
mod tests;
