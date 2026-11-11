use std::{
    collections::HashSet,
    io::{self, IsTerminal, Write},
    time::{SystemTime, UNIX_EPOCH},
};

use clap::{
    ArgGroup, Subcommand,
    builder::{PossibleValuesParser, TypedValueParser},
};
use cohesion_core::{
    invalidation::Invalidation,
    model::{
        AgentAgendaChange, AgentAskReply, AgentCodeOutcome, AgentForwardedItemKind,
        AgentMemoryChange, AgentMessageContent, AgentWake, IntelligenceAgent,
        IntelligenceAgentColor, IntelligenceAgentMessage, IntelligenceAgentMessageSender,
        IntelligenceAgentPersonality, IntelligenceAgentRunState, IntelligenceAgentVerbosity,
        MemoryScope,
    },
    protocol::{
        AcceptedResponse, Call, CreateIntelligenceAgentRequest, DeleteIntelligenceAgentRequest,
        ForgetMemoryRequest, GetAgentMessageRequest, GetIntelligenceAgentRequest,
        ListAgentCuesRequest, ListAgentMessagesRequest, ListAgentRunsRequest,
        ListAgentTeamsRequest, ListIntelligenceAgentsRequest, ListMemoriesRequest,
        MarkAgentReadRequest, RemoveAgentCueRequest, RespondToAgentChoiceRequest,
        SendAgentMessageRequest, SetAgentTeamRequest, UpdateIntelligenceAgentRequest,
        actor_session_stop_label,
    },
};

use super::{
    table::{self, ago, clock, duration, truncate},
    team::find_team,
    terminal_io::{confirmed, edit_in_editor, spin, words_and_stdin},
};
use crate::{
    cli_context::Context,
    cli_error::CliError,
    cli_macros::row,
    connection::{Invalidations, Received},
    name_lookup::find,
};

#[derive(Subcommand)]
pub enum AgentCommand {
    /// Create an agent. Agents talk on the `agent` tier's model (`cohesion ai use --for agent`).
    Create {
        /// What to call the agent; left out, it gets an unused first name like `Ada`.
        name: Option<String>,
        /// The color the agent is drawn in.
        #[arg(long, value_parser = color_parser(), default_value = "blue")]
        color: IntelligenceAgentColor,
        /// An emoji shown in place of the name's initial.
        #[arg(long)]
        emoji: Option<String>,
        /// The manner the agent speaks in; `balanced` when left out.
        #[arg(long, value_parser = personality_parser())]
        personality: Option<IntelligenceAgentPersonality>,
        /// How much the agent says; `brief` when left out.
        #[arg(long, value_parser = verbosity_parser())]
        verbosity: Option<IntelligenceAgentVerbosity>,
        /// What the agent is for, in a few words.
        #[arg(long)]
        role: Option<String>,
        /// The rules the agent always follows.
        #[arg(long)]
        directive: Option<String>,
        /// The team it starts on (id or title).
        #[arg(long)]
        team: Option<String>,
    },
    /// Rename, recolor or restyle an agent, or move it to a team; a flag left out keeps its
    /// value.
    #[command(group(ArgGroup::new("changes").required(true).multiple(true)))]
    Update {
        /// Agent id or name (names match case-insensitively).
        agent: String,
        /// The new name.
        #[arg(long, group = "changes")]
        name: Option<String>,
        /// The new color.
        #[arg(long, value_parser = color_parser(), group = "changes")]
        color: Option<IntelligenceAgentColor>,
        /// The new emoji; `--emoji ""` clears it.
        #[arg(long, group = "changes")]
        emoji: Option<String>,
        /// The new personality.
        #[arg(long, value_parser = personality_parser(), group = "changes")]
        personality: Option<IntelligenceAgentPersonality>,
        /// The new verbosity.
        #[arg(long, value_parser = verbosity_parser(), group = "changes")]
        verbosity: Option<IntelligenceAgentVerbosity>,
        /// The new role; `--role ""` clears it.
        #[arg(long, group = "changes")]
        role: Option<String>,
        /// The new directive; `--directive ""` clears it.
        #[arg(long, group = "changes")]
        directive: Option<String>,
        /// The team to move the agent to (id or title). The agent's pin won't change.
        #[arg(long, group = "changes", conflicts_with = "no_team")]
        team: Option<String>,
        /// Take it off its team.
        #[arg(long, group = "changes")]
        no_team: bool,
    },
    /// Delete an agent with its cues, runs and messages, after asking for confirmation.
    Delete {
        /// Agent id or name.
        agent: String,
        /// Don't ask for confirmation.
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Send a message to an agent and print what it sends back as its turn runs.
    Chat {
        /// Agent id or name.
        agent: String,
        /// What to say; several words are joined with spaces. Text piped to stdin is
        /// appended. With neither, reads one message per line from the terminal until end
        /// of input.
        message: Vec<String>,
        /// Open what the agent sends back in $VISUAL or $EDITOR instead of printing it.
        #[arg(short = 'e', long = "editor")]
        open_in_editor: bool,
    },
    /// Print an agent's latest messages, oldest first.
    Messages {
        /// Agent id or name.
        agent: String,
        /// How many messages to print.
        #[arg(short = 'n', long = "limit", default_value_t = 30)]
        limit: i64,
    },
    /// Pick an answer to a choice the agent asked, and print what the agent sends back.
    Pick {
        /// The choice's message id, as `agent messages` shows it.
        message_id: i64,
        /// The answer's letter, such as `B`.
        #[arg(value_parser = option_letter)]
        option: usize,
    },
    /// Print the memories an agent sees: its own, its team's and the global ones.
    Memories {
        /// Agent id or name.
        agent: String,
    },
    /// Forget a memory, by the id `agent memories` shows.
    Forget {
        /// The memory's id, such as `m12`.
        #[arg(value_parser = memory_id)]
        memory_id: i64,
    },
    /// Print the cues that wake an agent: each one's hook, effort, instructions and code.
    Cues {
        /// Agent id or name.
        agent: String,
        /// Remove this cue instead (its id or name), so its hook stops waking the agent.
        #[arg(long, value_name = "CUE")]
        remove: Option<String>,
    },
    /// List an agent's recent background runs, newest first.
    Runs {
        /// Agent id or name.
        agent: String,
        /// How many runs to list.
        #[arg(short = 'n', long = "limit", default_value_t = 20)]
        limit: i64,
    },
}

fn color_parser() -> impl TypedValueParser<Value = IntelligenceAgentColor> {
    PossibleValuesParser::new(IntelligenceAgentColor::CASES.iter().copied()).map(|text| {
        IntelligenceAgentColor::from_str_strict(&text).expect("clap only passes listed cases")
    })
}

fn personality_parser() -> impl TypedValueParser<Value = IntelligenceAgentPersonality> {
    PossibleValuesParser::new(IntelligenceAgentPersonality::CASES.iter().copied()).map(|text| {
        IntelligenceAgentPersonality::from_str_strict(&text).expect("clap only passes listed cases")
    })
}

fn verbosity_parser() -> impl TypedValueParser<Value = IntelligenceAgentVerbosity> {
    PossibleValuesParser::new(IntelligenceAgentVerbosity::CASES.iter().copied()).map(|text| {
        IntelligenceAgentVerbosity::from_str_strict(&text).expect("clap only passes listed cases")
    })
}

pub async fn run(ctx: &Context, verb: Option<AgentCommand>) -> Result<(), CliError> {
    match verb {
        None => list(ctx).await,
        Some(AgentCommand::Create {
            name,
            color,
            emoji,
            personality,
            verbosity,
            role,
            directive,
            team,
        }) => {
            let team_id = match team {
                Some(team) => Some(find_team(ctx, &team).await?.id),
                None => None,
            };
            create(
                ctx,
                CreateIntelligenceAgentRequest {
                    name,
                    color,
                    emoji,
                    personality,
                    verbosity,
                    role,
                    directive,
                    team_id,
                },
            )
            .await
        }
        Some(AgentCommand::Update {
            agent,
            name,
            color,
            emoji,
            personality,
            verbosity,
            role,
            directive,
            team,
            no_team,
        }) => {
            let agent_id = find_agent(ctx, &agent).await?.id;
            let team_id = match (team, no_team) {
                (Some(team), _) => Some(Some(find_team(ctx, &team).await?.id)),
                (None, true) => Some(None),
                (None, false) => None,
            };
            update(
                ctx,
                UpdateIntelligenceAgentRequest {
                    agent_id,
                    name,
                    color,
                    emoji,
                    personality,
                    verbosity,
                    role,
                    directive,
                },
                team_id,
            )
            .await
        }
        Some(AgentCommand::Delete { agent, yes }) => delete(ctx, agent, yes).await,
        Some(AgentCommand::Chat {
            agent,
            message,
            open_in_editor,
        }) => chat(ctx, agent, message, open_in_editor).await,
        Some(AgentCommand::Messages { agent, limit }) => messages(ctx, agent, limit).await,
        Some(AgentCommand::Pick { message_id, option }) => pick(ctx, message_id, option).await,
        Some(AgentCommand::Memories { agent }) => memories(ctx, agent).await,
        Some(AgentCommand::Forget { memory_id }) => forget(ctx, memory_id).await,
        Some(AgentCommand::Cues {
            agent,
            remove: None,
        }) => cues(ctx, agent).await,
        Some(AgentCommand::Cues {
            agent,
            remove: Some(cue),
        }) => remove_cue(ctx, agent, cue).await,
        Some(AgentCommand::Runs { agent, limit }) => runs(ctx, agent, limit).await,
    }
}

/// The agent whose id or name is `query`.
async fn find_agent(ctx: &Context, query: &str) -> Result<IntelligenceAgent, CliError> {
    let agents = ctx.call(ListIntelligenceAgentsRequest).await?;
    find(&agents, query).cloned()
}

/// `agent "Name" (id)`, as messages name an agent.
fn describe(agent: &IntelligenceAgent) -> String {
    format!("agent \"{}\" ({})", agent.agent_name, agent.id)
}

/// What the agent is doing while it works, else its role.
fn shown_status(agent: &IntelligenceAgent) -> &str {
    agent.activity.as_deref().unwrap_or(&agent.agent_role)
}

/// Agents with their color, team and what they are doing while they work,
/// else their role; a `•` after the name marks one with unread messages.
async fn list(ctx: &Context) -> Result<(), CliError> {
    let agents = ctx.call(ListIntelligenceAgentsRequest).await?;
    let teams = ctx.call(ListAgentTeamsRequest).await?;
    let team_of = |agent: &IntelligenceAgent| {
        teams
            .iter()
            .find(|team| Some(team.id) == agent.team_id)
            .map_or(String::new(), |team| team.title.clone())
    };
    table::list(
        &agents,
        "No agents.\nCreate one with `cohesion agent create <name>`.",
        |a| {
            row![
                >a.id,
                if a.agent_unread {
                    format!("{} •", a.agent_name)
                } else {
                    a.agent_name.clone()
                },
                a.agent_color,
                team_of(a),
                truncate(shown_status(a), 60)
            ]
        },
    );
    Ok(())
}

async fn create(ctx: &Context, request: CreateIntelligenceAgentRequest) -> Result<(), CliError> {
    let agent = ctx.call(request).await?;
    println!(
        "Created {}; talk to it with `cohesion agent chat {}`.",
        describe(&agent),
        agent.id
    );
    Ok(())
}

/// Applies `request`, when it changes anything, then puts the agent on
/// `team_id` when given (`Some(None)` for no team).
async fn update(
    ctx: &Context,
    request: UpdateIntelligenceAgentRequest,
    team_id: Option<Option<i64>>,
) -> Result<(), CliError> {
    let agent_id = request.agent_id;
    let UpdateIntelligenceAgentRequest {
        agent_id: _,
        name,
        color,
        emoji,
        personality,
        verbosity,
        role,
        directive,
    } = &request;
    let changes_profile = name.is_some()
        || color.is_some()
        || emoji.is_some()
        || personality.is_some()
        || verbosity.is_some()
        || role.is_some()
        || directive.is_some();
    let mut agent = None;
    if changes_profile {
        agent = Some(ctx.call(request).await?);
    }
    if let Some(team_id) = team_id {
        agent = Some(ctx.call(SetAgentTeamRequest { agent_id, team_id }).await?);
    }
    let agent = agent.expect("clap requires at least one change");
    println!("Updated {}: {}.", describe(&agent), agent.agent_color);
    Ok(())
}

async fn delete(ctx: &Context, agent: String, yes: bool) -> Result<(), CliError> {
    let found = find_agent(ctx, &agent).await?;
    let question = format!(
        "Delete {} with all its messages and runs?",
        describe(&found)
    );
    if !confirmed(&question, yes)? {
        return Ok(());
    }
    let deleted = ctx
        .call(DeleteIntelligenceAgentRequest { agent_id: found.id })
        .await?;
    println!("Deleted {}.", describe(&deleted));
    Ok(())
}

/// Sends one message, or with no message and a terminal on stdin, one per line until end of
/// input. A failed message in the loop is reported and the conversation goes on.
async fn chat(
    ctx: &Context,
    agent: String,
    message: Vec<String>,
    open_in_editor: bool,
) -> Result<(), CliError> {
    let agent = find_agent(ctx, &agent).await?;
    let message = words_and_stdin(&message)?;
    if !message.is_empty() || !io::stdin().is_terminal() {
        return send(ctx, &agent, message, open_in_editor).await;
    }

    eprintln!("Talking to {}. Ctrl-D to exit.", agent.agent_name);
    loop {
        eprint!("> ");
        io::stderr().flush().map_err(CliError::CannotReadTerminal)?;
        let mut line = String::new();
        let read = io::stdin()
            .read_line(&mut line)
            .map_err(CliError::CannotReadTerminal)?;
        if read == 0 {
            eprintln!();
            return Ok(());
        }
        let message = line.trim();
        if message.is_empty() {
            continue;
        }
        if let Err(err) = send(ctx, &agent, message.to_string(), open_in_editor).await {
            eprintln!("Error: {err}");
        }
    }
}

/// Sends `message` to `agent`; only what the agent's turn posts goes to stdout, as it
/// posts it (or all at once in the editor).
async fn send(
    ctx: &Context,
    agent: &IntelligenceAgent,
    message: String,
    open_in_editor: bool,
) -> Result<(), CliError> {
    let request = SendAgentMessageRequest {
        agent_id: agent.id,
        message,
        attachments: None,
        forward: None,
        skill: None,
    };
    let posted = follow_turn(ctx, agent.id, request, !open_in_editor).await?;
    if open_in_editor {
        edit_in_editor(&posted.join("\n\n"))?;
    }
    ctx.call(MarkAgentReadRequest { agent_id: agent.id })
        .await?;
    Ok(())
}

/// Fires `request`, which queues a turn of agent `agent_id`'s, then follows the agent's
/// invalidations until the turn has run, and returns what it posted: its messages,
/// choices, why it failed, and any reaction it put on the user's message.
/// With `print`, each is printed as it arrives.
async fn follow_turn<C: Call<Reply = AcceptedResponse>>(
    ctx: &Context,
    agent_id: i64,
    request: C,
    print: bool,
) -> Result<Vec<String>, CliError> {
    let newest = |limit| ListAgentMessagesRequest {
        agent_id,
        before: None,
        limit,
    };
    let after = ctx.call(newest(1)).await?.last().map_or(0, |m| m.id);
    // Listening before the request goes out, so no invalidation is missed.
    let mut invalidations = ctx
        .connection()
        .subscribe()
        .await
        .map_err(|err| CliError::Unreachable(ctx.target(), err))?;
    ctx.call(request).await?;

    let mut posted = Vec::new();
    let mut shown = HashSet::new();
    // The turn is queued, so the agent works until it has run; a turn that fails at
    // once may end before the agent is seen working, but posts its error.
    let mut seen_working = false;
    loop {
        let agent = ctx.call(GetIntelligenceAgentRequest { agent_id }).await?;
        let working = agent.activity.is_some();
        seen_working |= working;
        let mut failed = false;
        for message in ctx.call(newest(100)).await? {
            if message.id <= after {
                continue;
            }
            failed |= matches!(message.message_content, AgentMessageContent::Error { .. });
            let text = match (message.sender, &message.message_content) {
                (IntelligenceAgentMessageSender::User, _) => match &message.reaction {
                    Some(reaction) => format!("({} reacted {reaction})", agent.agent_name),
                    None => continue,
                },
                (_, AgentMessageContent::Text { text }) => text.clone(),
                (_, AgentMessageContent::Choice(_) | AgentMessageContent::Error { .. }) => {
                    summary(&message).join("\n")
                }
                _ => continue,
            };
            if shown.insert((message.id, text.clone())) {
                if print {
                    println!("{text}");
                }
                posted.push(text);
            }
        }
        if failed || (seen_working && !working) {
            return Ok(posted);
        }
        spin("Thinking…", agent_invalidated(&mut invalidations, agent_id))
            .await
            .ok_or(CliError::UnexpectedReply)?;
    }
}

/// Waits for an invalidation of agent `agent_id`, or a `Reset`, which may have dropped one
/// (the caller reads the agent afresh either way); `None` once the session ends.
async fn agent_invalidated(invalidations: &mut Invalidations, agent_id: i64) -> Option<()> {
    loop {
        match invalidations.next().await? {
            Received::Batch(batch) if !batch.keys.contains(&Invalidation::Agent(agent_id)) => {}
            Received::Batch(_) | Received::Reset => return Some(()),
        }
    }
}

/// The feed, one row per message: its id, time, sender and a one-line summary, with what woke a background run, and the message it replies to on indented lines
/// below.
async fn messages(ctx: &Context, agent: String, limit: i64) -> Result<(), CliError> {
    let agent = find_agent(ctx, &agent).await?;
    let messages = ctx
        .call(ListAgentMessagesRequest {
            agent_id: agent.id,
            before: None,
            limit,
        })
        .await?;
    let summaries = messages
        .iter()
        .map(|message| (message, summary(message)))
        .collect::<Vec<_>>();
    let empty = format!(
        "No messages.\nStart with `cohesion agent chat {} <message>`.",
        agent.id
    );
    table::list(&summaries, &empty, |(message, lines)| {
        let sender = match (message.sender, message.run_id) {
            (IntelligenceAgentMessageSender::User, _) => "you".to_string(),
            (IntelligenceAgentMessageSender::Agent, _) => agent.agent_name.clone(),
            (IntelligenceAgentMessageSender::BackgroundAgent, Some(run)) => {
                format!("{} (run {run})", agent.agent_name)
            }
            (IntelligenceAgentMessageSender::BackgroundAgent, None) => {
                format!("{} (job)", agent.agent_name)
            }
        };
        let (headline, details) = lines.split_first().expect("a summary has a headline");
        let woke = message
            .run_description
            .as_ref()
            .map(|description| format!("after {description}"));
        let reply = message.reply_to.map(|id| format!("replying to #{id}"));
        details.iter().fold(
            row![>format!("#{}", message.id), clock(message.sent_at), sender, headline]
                .detail(woke)
                .detail(reply),
            |row, detail| row.detail(Some(detail.clone())),
        )
    });
    ctx.call(MarkAgentReadRequest { agent_id: agent.id })
        .await?;
    Ok(())
}

/// A message as a headline, then indented detail lines: a text's first line (cut short); a
/// code execution as `ran "title"` with its error, or marked cancelled; a documentation lookup
/// as `learned about module`; a choice as `asked "question" [status]` with its lettered
/// options; a memory as `remembered: "text"` or `forgot: "text"`; a forward as
/// `forwarded task "title" #42` with the note; a skill as `used skill "name"` with the
/// user's note; a question between agents as `asked Ada [status]` or `Ada asked [status]`
/// with the question and any answer.
fn summary(message: &IntelligenceAgentMessage) -> Vec<String> {
    match &message.message_content {
        AgentMessageContent::Text { text } => {
            let mut lines = text.trim().lines();
            let first = lines.next().unwrap_or_default();
            let more = if lines.next().is_some() { " …" } else { "" };
            vec![format!("{}{more}", truncate(first, 100))]
        }
        AgentMessageContent::CodeExecution(run) => match &run.outcome {
            AgentCodeOutcome::Succeeded { .. } => vec![format!("ran \"{}\"", run.title)],
            AgentCodeOutcome::Failed { error } => vec![
                format!("ran \"{}\" → error", run.title),
                truncate(error, 100),
            ],
            AgentCodeOutcome::Cancelled => vec![format!("ran \"{}\" → cancelled", run.title)],
        },
        AgentMessageContent::Error { error } => {
            vec![format!("failed: {}", truncate(error, 100))]
        }
        AgentMessageContent::Documentation { module } => {
            vec![format!("learned about {module}")]
        }
        AgentMessageContent::Memory { memory, change } => {
            let verb = match change {
                AgentMemoryChange::Remembered => "remembered",
                AgentMemoryChange::Forgotten => "forgot",
            };
            vec![format!("{verb}: \"{}\"", truncate(memory, 100))]
        }
        AgentMessageContent::Choice(choice) => {
            let status = match choice.selected {
                Some(index) => format!("picked {}", choice.lettered(index)),
                None => "unanswered".to_string(),
            };
            let mut lines = vec![format!(
                "asked \"{}\" [{status}]",
                truncate(&choice.question, 100)
            )];
            lines.extend((0..choice.options.len()).map(|index| choice.lettered(index)));
            lines
        }
        AgentMessageContent::Forward { note, item } => {
            let kind = item.kind.noun();
            let id = match item.kind {
                AgentForwardedItemKind::Task => format!(" #{}", item.id),
                _ => String::new(),
            };
            let mut lines = vec![format!("forwarded {kind} \"{}\"{id}", item.title)];
            lines.extend((!note.is_empty()).then(|| truncate(note, 100)));
            lines
        }
        AgentMessageContent::AskedAgent(ask) => ask_lines(
            format!("asked {}", ask.agent.name),
            &ask.question,
            &ask.reply,
        ),
        AgentMessageContent::AskedByAgent(ask) => ask_lines(
            format!("{} asked", ask.agent.name),
            &ask.question,
            &ask.reply,
        ),
        AgentMessageContent::Note { note } => vec![format!("noted: {}", truncate(note, 100))],
        AgentMessageContent::Woke {
            by: AgentWake::Cue { cue_name, .. },
        } => {
            vec![format!("woke: cue \"{cue_name}\" fired")]
        }
        AgentMessageContent::Woke {
            by: AgentWake::ActorSession { title, stop, .. },
        } => {
            vec![format!(
                "woke: actor session \"{title}\" {}",
                actor_session_stop_label(*stop)
            )]
        }
        AgentMessageContent::Woke {
            by: AgentWake::AssignedTask { title, .. },
        } => {
            vec![format!("woke: task \"{title}\" was assigned to you")]
        }
        AgentMessageContent::RebuiltContext { .. } => vec!["context rebuilt".to_string()],
        AgentMessageContent::Agenda(update) => {
            let changed = truncate(&update.changed_text(), 100);
            vec![match update.change {
                AgentAgendaChange::Appended => format!("added to the agenda: {changed}"),
                AgentAgendaChange::Finished => format!("finished: {changed}"),
                AgentAgendaChange::Reset => "cleared the agenda".to_string(),
            }]
        }
        AgentMessageContent::Skill { name, note, .. } => {
            let mut lines = vec![format!("used skill \"{name}\"")];
            lines.extend((!note.is_empty()).then(|| truncate(note, 100)));
            lines
        }
        AgentMessageContent::UsedSkill { name } => vec![format!("used skill \"{name}\"")],
    }
}

/// A question between agents: `headline [status]`, the question, then the answer or why
/// there is none.
fn ask_lines(headline: String, question: &str, reply: &AgentAskReply) -> Vec<String> {
    let (status, detail) = match reply {
        AgentAskReply::Waiting => ("waiting", None),
        AgentAskReply::Answered { text } => ("answered", Some(truncate(text, 100))),
        AgentAskReply::Unanswered { reason } => ("unanswered", Some(truncate(reason, 100))),
    };
    let mut lines = vec![format!("{headline} [{status}]"), truncate(question, 100)];
    lines.extend(detail);
    lines
}

/// Picks option `option` of the choice in `message_id`; only what the agent's turn on it
/// posts goes to stdout.
async fn pick(ctx: &Context, message_id: i64, option: usize) -> Result<(), CliError> {
    let agent_id = ctx
        .call(GetAgentMessageRequest { message_id })
        .await?
        .agent_id;
    let request = RespondToAgentChoiceRequest { message_id, option };
    follow_turn(ctx, agent_id, request, true).await?;
    ctx.call(MarkAgentReadRequest { agent_id }).await?;
    Ok(())
}

/// An answer's letter, `A` or `a` onwards, as its index.
fn option_letter(letter: &str) -> Result<usize, String> {
    match letter.to_ascii_uppercase().as_bytes() {
        [letter @ b'A'..=b'Z'] => Ok(usize::from(letter - b'A')),
        _ => Err("expected a letter, such as B".to_string()),
    }
}

/// The memories `agent` sees, its own, then its team's, then the global ones: each one's
/// id, `team` or `global` for one it shares, and its text.
async fn memories(ctx: &Context, agent: String) -> Result<(), CliError> {
    let agent = find_agent(ctx, &agent).await?;
    let memories = ctx
        .call(ListMemoriesRequest {
            agent_id: Some(agent.id),
        })
        .await?;
    table::list(&memories, "No memories yet.", |m| {
        let scope = match m.scope {
            MemoryScope::Agent { .. } => "",
            MemoryScope::Team { .. } => "team",
            MemoryScope::Global => "global",
        };
        row![>format!("m{}", m.id), scope, m.memory]
    });
    Ok(())
}

async fn forget(ctx: &Context, memory_id: i64) -> Result<(), CliError> {
    let memory = ctx.call(ForgetMemoryRequest { memory_id }).await?;
    println!("Forgot m{}.", memory.id);
    Ok(())
}

/// A memory's id as `agent memories` shows it, `m12`, or bare, `12`.
fn memory_id(id: &str) -> Result<i64, String> {
    id.strip_prefix(['m', 'M'])
        .unwrap_or(id)
        .parse()
        .map_err(|_| "expected a memory id, such as m12".to_string())
}

/// Each cue under its name and id: the hook and effort, then the agent's
/// instructions to itself and the code that decides whether it wakes, each
/// under a heading.
async fn cues(ctx: &Context, agent: String) -> Result<(), CliError> {
    let agent = find_agent(ctx, &agent).await?;
    let cues = ctx
        .call(ListAgentCuesRequest { agent_id: agent.id })
        .await?;
    if cues.is_empty() {
        println!("No cues set.");
    }
    for (index, cue) in cues.iter().enumerate() {
        if index > 0 {
            println!();
        }
        println!("{} (cue {})", cue.name, cue.id);
        println!("  hook: {}", cue.hook);
        println!("  effort: {}", cue.effort);
        for (title, text) in [("Instructions", &cue.instructions), ("Code", &cue.script)] {
            println!("\n  {title}");
            for line in text.trim_end().lines() {
                println!("    {line}");
            }
        }
    }
    Ok(())
}

async fn remove_cue(ctx: &Context, agent: String, cue: String) -> Result<(), CliError> {
    let agent = find_agent(ctx, &agent).await?;
    let cues = ctx
        .call(ListAgentCuesRequest { agent_id: agent.id })
        .await?;
    let cue = find(&cues, &cue)?;
    ctx.call(RemoveAgentCueRequest { cue_id: cue.id }).await?;
    println!("Removed {}'s cue \"{}\".", agent.agent_name, cue.name);
    Ok(())
}

/// Runs, newest first, with when they started, how long they took, the cue and hook that fired,
/// tokens in/out, cost, the thread they ran in and how they ended, with a failure's error.
async fn runs(ctx: &Context, agent: String, limit: i64) -> Result<(), CliError> {
    let agent = find_agent(ctx, &agent).await?;
    let runs = ctx
        .call(ListAgentRunsRequest {
            agent_id: agent.id,
            limit,
        })
        .await?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    table::list(
        &runs,
        "No runs.\nAn agent runs in the background once it sets a cue.",
        |r| {
            let took = match &r.state {
                IntelligenceAgentRunState::Running => String::new(),
                IntelligenceAgentRunState::Completed { ended_at }
                | IntelligenceAgentRunState::Failed { ended_at, .. } => {
                    duration(ended_at - r.started_at)
                }
            };
            let cost = r.cost.map(|c| format!("${c:.4}")).unwrap_or_default();
            let thread = r
                .thread_id
                .map(|t| format!("thread {t}"))
                .unwrap_or_default();
            let outcome = match &r.state {
                IntelligenceAgentRunState::Running => "running".to_string(),
                IntelligenceAgentRunState::Completed { .. } => "completed".to_string(),
                IntelligenceAgentRunState::Failed { error, .. } => {
                    format!("failed: {}", truncate(error, 60))
                }
            };
            row![
                >r.id,
                ago(now - r.started_at),
                >took,
                r.cue_name.as_deref().unwrap_or_default(),
                r.hook,
                >format!("{}/{}", r.input_tokens + r.cache_write_tokens, r.output_tokens),
                >cost,
                thread,
                outcome
            ]
        },
    );
    Ok(())
}
