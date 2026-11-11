use std::sync::Arc;
use std::time::Duration;

use jiff::Timestamp;

use cohesion_core::log::Module;
use cohesion_core::log_info;
use cohesion_core::model::{
    CacheLength, CacheLifetime, EndpointBackend, EndpointModel, Memory, RemoteChain,
};
use cohesion_db::{
    CreateIntelligenceCostEventCommand, CreateIntelligenceThreadCommand, DatabaseContext,
    ForkIntelligenceThreadCommand, SetAgentContextTokensCommand, TierModel,
    UpdateIntelligenceThreadRemoteCommand,
};

use super::tier_model::SessionConstructionError;
use crate::account_credentials::AccountCredentials;
use crate::activity::{Activity, ShowActivity};
use crate::intelligence_backend::IntelligenceBackend;
use crate::intelligence_error::IntelligenceError;
use crate::providers::backend_for;
use crate::spend_limits::SpendCap;
use crate::system_prompt::{HeardMemories, SystemPrompt};
use crate::tools::SessionTool;
use crate::transcript::{RemoteTranscriptState, Transcript};
use crate::transcript_item::{
    Prompt, PromptImage, ToolCallRequest, ToolCallStatus, TranscriptItem,
};
use crate::turn::TurnOutput;
use crate::turn_input::TurnInput;
use crate::usage::{TurnCost, Usage};

/// The most model calls one answer may take; a model still calling tools
/// after this many is looping, and the answer fails instead.
const MAX_ROUNDS: usize = 50;

pub struct Session {
    backend: Box<dyn IntelligenceBackend>,
    pub(crate) transcript: Transcript,
    /// The memories the thread last heard of: its system prompt's, then
    /// each change it was told of.
    heard: HeardMemories,
    /// How many memories each scope keeps, as the thread's system prompt says,
    /// whatever the setting has become since.
    memory_limit: i64,
    /// The tools the transcript offers (`transcript.tools` holds their
    /// definitions), each by the name the model calls it.
    tools: Vec<NamedTool>,
    database: DatabaseContext,
    /// The model the session runs on, whose prices its turns are recorded at.
    model: EndpointModel,
    /// The models.dev provider of the model's endpoint, recorded with each
    /// cost.
    provider_identifier: String,
    thread_id: i64,
    /// What the endpoint gives a request asking for its short cache.
    short_cache: CacheLifetime,
    /// What the endpoint gives a request asking for its long cache: its
    /// short one when it offers no long one.
    long_cache: CacheLifetime,
    /// When the session sent its first model call, once it has.
    first_sent_at: Option<Timestamp>,
    /// The thread's latest model call, once it has made one.
    last_call: Option<LastCall>,
    /// What holds the session's model calls to the spend limits; `None`
    /// for the user's own turns, which no limit holds back.
    cap: Option<SpendCap>,
    /// The agent whose conversation this thread is, told how large the
    /// thread is after each request; `None` for every other session.
    reports_to: Option<i64>,
}

/// A thread's latest model call: when it was sent, and how many tokens the
/// provider counted in its prompt. Both providers time a cached prompt's
/// life from the start of the request that wrote or read it, and every
/// call of a thread reads its cached prefix back, so the latest call's
/// sending is when its cache was last kept alive.
#[derive(Clone, Copy, Debug)]
pub struct LastCall {
    pub sent_at: Timestamp,
    pub prompt_tokens: u64,
}
impl LastCall {
    /// Whether a prompt cache that lives `ttl` past this call has expired
    /// by `now`.
    pub fn cache_expired(&self, ttl: Duration, now: Timestamp) -> bool {
        let elapsed = now.as_millisecond() - self.sent_at.as_millisecond();
        elapsed > ttl.as_millis() as i64
    }
}

impl Session {
    /// Starts a new thread on `model`, on its endpoint, with
    /// `system_prompt`. `cache_key` groups sessions that start with the same
    /// tools and system prompt, such as one agent's forks, so a
    /// provider that routes by it finds their shared prefix already cached.
    /// An account-backed endpoint's turns ask `credentials` for the
    /// account's credential.
    pub async fn new(
        database: DatabaseContext,
        credentials: Arc<dyn AccountCredentials>,
        tier_model: TierModel,
        system_prompt: SystemPrompt,
        tools: Vec<Box<dyn SessionTool>>,
        cache_key: String,
    ) -> Result<Self, SessionConstructionError> {
        let thread = database
            .send(CreateIntelligenceThreadCommand {
                system_prompt: system_prompt.prompt(&tier_model.model.model_identifier),
            })
            .await
            .map_err(SessionConstructionError::Command)?;
        Ok(Self::on_thread(
            database,
            credentials,
            tier_model,
            thread.id,
            Transcript::new(thread.system_prompt),
            system_prompt.heard(),
            system_prompt.memory_limit,
            tools,
            cache_key,
        ))
    }

    /// Carries on thread `thread_id` as it stood after its first `items`
    /// items, in a new thread of its own: the same system prompt and
    /// history, so a provider that caches prompts finds them cached, with
    /// `tools`, whose definitions should match the thread's for the same
    /// reason, `heard`, the memories the thread last heard of then, and
    /// `memory_limit`, the capacity its system prompt states. The source thread
    /// is left as it was.
    #[allow(clippy::too_many_arguments)]
    pub async fn fork(
        database: DatabaseContext,
        credentials: Arc<dyn AccountCredentials>,
        tier_model: TierModel,
        thread_id: i64,
        items: usize,
        heard: HeardMemories,
        memory_limit: i64,
        tools: Vec<Box<dyn SessionTool>>,
        cache_key: String,
    ) -> Result<Self, SessionConstructionError> {
        let (thread, entries) = database
            .send(ForkIntelligenceThreadCommand { thread_id, items })
            .await
            .map_err(SessionConstructionError::Command)?;
        let mut transcript = Transcript::new(thread.system_prompt);
        for entry in entries {
            transcript.add(entry.into());
        }
        transcript.remote = thread.remote.map(|chain| RemoteTranscriptState {
            conversation_id: chain.remote_id,
            cursor: chain.remote_cursor as usize,
        });
        Ok(Self::on_thread(
            database,
            credentials,
            tier_model,
            thread.id,
            transcript,
            heard,
            memory_limit,
            tools,
            cache_key,
        ))
    }

    /// The session of thread `thread_id`, which holds `transcript`, has heard
    /// of `heard` and states `memory_limit`, on `model`'s endpoint.
    #[allow(clippy::too_many_arguments)]
    fn on_thread(
        database: DatabaseContext,
        credentials: Arc<dyn AccountCredentials>,
        TierModel { endpoint, model }: TierModel,
        thread_id: i64,
        mut transcript: Transcript,
        heard: HeardMemories,
        memory_limit: i64,
        tools: Vec<Box<dyn SessionTool>>,
        cache_key: String,
    ) -> Self {
        transcript.tools = tools.iter().map(|tool| tool.definition()).collect();
        let tools = transcript
            .tools
            .iter()
            .zip(tools)
            .map(|(definition, tool)| NamedTool {
                name: definition.name,
                tool,
            })
            .collect();
        let model_identifier = model.model_identifier.clone();
        let short_cache = endpoint.backend.cache_lifetime(CacheLength::Short);
        let long_cache = endpoint.backend.cache_lifetime(CacheLength::Long);
        let (EndpointBackend::Integrated {
            provider_identifier,
            ..
        }
        | EndpointBackend::Account {
            provider_identifier,
            ..
        }) = &endpoint.backend;
        let provider_identifier = provider_identifier.clone();
        let backend = backend_for(endpoint.backend, credentials, model_identifier, cache_key);

        Self {
            tools,
            database,
            backend,
            transcript,
            heard,
            memory_limit,
            model,
            provider_identifier,
            thread_id,
            short_cache,
            long_cache,
            first_sent_at: None,
            last_call: None,
            cap: None,
            reports_to: None,
        }
    }

    /// The session, its every model call first checked against the spend
    /// limits through `cap`: one that finds a limit reached ends the answer
    /// with [`IntelligenceError::SpendLimitReached`].
    pub(crate) fn capped(mut self, cap: SpendCap) -> Self {
        self.cap = Some(cap);
        self
    }

    /// The session, reporting the size of its prompt after each request as
    /// agent `agent_id`'s context tokens.
    pub(crate) fn reporting_to(mut self, agent_id: i64) -> Self {
        self.reports_to = Some(agent_id);
        self
    }

    pub fn thread(&self) -> i64 {
        self.thread_id
    }

    /// How many items the thread holds, which a fork of it as it stands now
    /// copies.
    pub fn items(&self) -> usize {
        self.transcript.items.len()
    }

    /// The memories the thread last heard of, which a fork of it carries
    /// on.
    pub fn heard(&self) -> &HeardMemories {
        &self.heard
    }

    /// How many memories each scope keeps, as the thread's system prompt says.
    pub fn memory_limit(&self) -> i64 {
        self.memory_limit
    }

    /// Hears `memories`, every memory the agent sees as it is now: what
    /// changed since the thread last heard (its system prompt's list, then
    /// each change it was told of), for the next prompt to open with, or
    /// `None` when nothing did. The thread's system prompt never changes,
    /// so the provider's cached prefix holds.
    pub fn hear_memories(&mut self, memories: Vec<Memory>) -> Option<String> {
        self.heard.hear(memories)
    }

    /// The endpoint model the session runs on.
    pub fn model(&self) -> &EndpointModel {
        &self.model
    }

    /// When the session sent its first model call, once it has: the
    /// request that read back what it copied from another thread and wrote
    /// the long cache at its end, which keeps that thread's cache alive
    /// from then on every provider. A later read keeps an Anthropic cache
    /// alive too, but not an OpenAI long cache past its own write.
    pub fn first_sent_at(&self) -> Option<Timestamp> {
        self.first_sent_at
    }

    /// The thread's latest model call, once it has made one.
    pub fn last_call(&self) -> Option<LastCall> {
        self.last_call
    }

    /// How long the endpoint's server keeps the thread's prompt cached after
    /// the request that last used it.
    pub fn cache_ttl(&self) -> Duration {
        let tail = match self.transcript.cache.tail {
            CacheLength::Short => self.short_cache,
            CacheLength::Long => self.long_cache,
        };
        minutes(tail)
    }

    /// How long the endpoint's server keeps the thread's shared items
    /// cached after the request that last used them.
    pub fn shared_cache_ttl(&self) -> Duration {
        minutes(self.long_cache)
    }

    /// Asks for `length` of the endpoint's caches for what the thread adds
    /// past its shared items; the long one unless told otherwise.
    pub fn cache_tail(&mut self, length: CacheLength) {
        self.transcript.cache.tail = length;
    }

    /// Asks for the endpoint's short cache for all of the thread, tools and
    /// system prompt included, for a run nothing reads back for long.
    pub fn cache_only_short(&mut self) {
        self.transcript.cache.prefix = CacheLength::Short;
        self.transcript.cache.tail = CacheLength::Short;
    }

    /// Asks for the endpoint's long cache for the thread's items as they
    /// stand, which other threads copy and read back after this one has
    /// moved on.
    pub fn share_items(&mut self) {
        self.transcript.cache.shared_items = self.items();
    }

    /// How many of the thread's items it shares, for a fork of it to copy.
    pub fn shared_items(&self) -> usize {
        self.transcript.cache.shared_items
    }

    /// Opens the thread with `text`, the part of its first prompt other
    /// threads copy, and shares it.
    pub async fn open_shared(&mut self, text: String) -> Result<(), IntelligenceError> {
        self.record(TranscriptItem::UserMessage(Prompt {
            text,
            images: Vec::new(),
        }))
        .await?;
        self.share_items();
        Ok(())
    }

    /// Sends `prompt` with `images` after its text, showing what the answer
    /// is doing through `activity` as it goes, and hearing from the user
    /// through `input` while the answer works: what they send joins it
    /// before its next model call, and their stop cuts short the model call
    /// it waits on and fails the tool calls it has yet to run, ending the
    /// answer with [`IntelligenceError::Stopped`]. A response whose tool
    /// calls all succeed, one of them [ending the turn](SessionTool::ends_turn),
    /// ends the answer there unless the user said more meanwhile, rather
    /// than asking the model for a final response nothing reads. The thread
    /// stays whole for the next prompt.
    pub async fn send_steerable(
        &mut self,
        prompt: String,
        images: Vec<PromptImage>,
        activity: &dyn ShowActivity,
        input: &dyn TurnInput,
    ) -> Result<String, IntelligenceError> {
        self.record(TranscriptItem::UserMessage(Prompt {
            text: prompt,
            images,
        }))
        .await?;
        let mut tc = 0;
        loop {
            if input.is_stopped() {
                return self.stop().await;
            }
            if let Some(cap) = &self.cap {
                let reached = cap
                    .limits
                    .reached(&self.database, cap.agent_id, &cap.trace)
                    .await
                    .map_err(IntelligenceError::SpendCheck)?;
                if let Some(spend) = reached {
                    return Err(IntelligenceError::SpendLimitReached(spend));
                }
            }
            tc += 1;
            if tc > MAX_ROUNDS {
                return Err(IntelligenceError::TooManyRounds(MAX_ROUNDS));
            }
            log_info!(Module::Intelligence, "Starting turn {}...", tc);
            activity.show(Activity::Thinking(tc - 1)).await;
            let sent_at = Timestamp::now();
            let sent = tokio::select! {
                sent = self.backend.send(&self.transcript) => Some(sent),
                () = input.stopped() => None,
            };
            let Some(sent) = sent else {
                return self.stop().await;
            };
            let turn = sent?;
            self.first_sent_at.get_or_insert(sent_at);
            self.last_call = Some(LastCall {
                sent_at,
                prompt_tokens: turn.usage.prompt_tokens(),
            });
            if let Some(remote) = turn.remote {
                self.database
                    .send(UpdateIntelligenceThreadRemoteCommand {
                        thread_id: self.thread_id,
                        remote: Some(RemoteChain {
                            remote_id: remote.conversation_id.to_string(),
                            remote_cursor: remote.cursor as i64,
                        }),
                    })
                    .await
                    .map_err(IntelligenceError::Save)?;
                self.transcript.remote = Some(remote);
            }
            if let Some(agent_id) = self.reports_to {
                _ = self
                    .database
                    .send(SetAgentContextTokensCommand {
                        agent_id,
                        tokens: Some(turn.usage.prompt_tokens() as i64),
                    })
                    .await;
            }
            self.record_cost(&turn.usage, turn.cost).await;

            // Responses must follow every request from the turn, so hold them
            // until the turn's outputs are in the transcript.
            let mut responses = Vec::new();
            let mut reply = None;
            // Whether a call asked to end the turn, and none failed.
            let mut ends = false;
            let mut failed = false;
            for output in turn.output {
                match &output {
                    TurnOutput::ToolCall(tcr) if input.is_stopped() => {
                        failed = true;
                        responses.push(tcr.respond(stopped_call()));
                    }
                    TurnOutput::ToolCall(tcr) => {
                        let (status, ended) = run_tool(&self.tools, activity, tcr).await;
                        failed |= matches!(status, ToolCallStatus::Failure(_));
                        ends |= ended;
                        responses.push(tcr.respond(status));
                    }
                    TurnOutput::Message(text) => reply = Some(text.clone()),
                    TurnOutput::Thinking(_) => {}
                }
                self.record(output.into()).await?;
            }
            // The answer ends once the model calls no tool, or once every
            // call succeeded and one ended the turn, unless the user said
            // more meanwhile.
            let ending = responses.is_empty() || (ends && !failed);
            for response in responses {
                self.record(response).await?;
            }
            let steered = if ending { input.close() } else { input.take() };
            if ending && steered.is_empty() {
                return Ok(reply.map(|x| x.trim().to_owned()).unwrap_or_default());
            }
            for prompt in steered {
                self.record(TranscriptItem::UserMessage(prompt)).await?;
            }
        }
    }

    /// Ends an answer the user stopped, telling the thread so the next
    /// prompt reads after it.
    async fn stop(&mut self) -> Result<String, IntelligenceError> {
        self.record(TranscriptItem::UserMessage(Prompt {
            text: "(The user stopped your turn here.)".to_string(),
            images: Vec::new(),
        }))
        .await?;
        Err(IntelligenceError::Stopped)
    }

    /// Stores what one turn cost as an `IntelligenceCostEvent`: what the
    /// provider reported, or else its usage priced at the model's rates.
    /// Providers apply the large-context threshold per request, so each turn
    /// is priced on its own prompt rather than on the running total. Recording
    /// before the turn's tool calls run means a later failure can't lose a
    /// turn that has already been billed. A turn with no reported cost on a
    /// model without prices records nothing.
    async fn record_cost(&self, usage: &Usage, cost: TurnCost) {
        let cost = match cost {
            TurnCost::None => usage.cost(&self.model, self.long_cache_write_multiple()),
            reported => reported,
        };
        let (total_cost, [input_cost, output_cost, cache_read_cost, cache_write_cost]) = match cost
        {
            TurnCost::Detailed {
                input_cost,
                output_cost,
                cache_read_cost,
                cache_write_cost,
            } => (
                input_cost + output_cost + cache_read_cost + cache_write_cost,
                [input_cost, output_cost, cache_read_cost, cache_write_cost].map(Some),
            ),
            TurnCost::Simple { total_cost } => (total_cost, [None; 4]),
            TurnCost::None => return,
        };
        _ = self
            .database
            .send(CreateIntelligenceCostEventCommand {
                thread_id: self.thread_id,
                provider_identifier: self.provider_identifier.clone(),
                model_identifier: self.model.model_identifier.clone(),
                input_tokens: usage.input_tokens as i64,
                output_tokens: usage.output_tokens as i64,
                cache_read_tokens: usage.cache_reads as i64,
                cache_write_tokens: usage.cache_writes as i64,
                total_cost,
                input_cost,
                output_cost,
                cache_read_cost,
                cache_write_cost,
            })
            .await;
    }

    /// What a write to the endpoint's long cache costs, as a multiple of a
    /// write to its short one; 1 when it offers no long one.
    fn long_cache_write_multiple(&self) -> f64 {
        match self.long_cache {
            CacheLifetime::Short { .. } => 1.0,
            CacheLifetime::Long(long) => long.write_multiple,
        }
    }

    /// Saves `item` to the thread and adds it to the transcript, so a resumed thread
    /// restores exactly what the model saw.
    async fn record(&mut self, item: TranscriptItem) -> Result<(), IntelligenceError> {
        save(&self.database, self.thread_id, &item).await?;
        self.transcript.add(item);
        Ok(())
    }
}

/// How long `cache` lasts. The schema keeps an endpoint's cache lifetimes
/// at least a minute.
fn minutes(cache: CacheLifetime) -> Duration {
    Duration::from_secs(cache.minutes() as u64 * 60)
}

/// What a tool call the user stopped answers, run or not.
fn stopped_call() -> ToolCallStatus {
    ToolCallStatus::Failure("The user stopped your turn before this finished.".to_string())
}

/// Saves `item` to thread `thread_id`.
async fn save(
    database: &DatabaseContext,
    thread_id: i64,
    item: &TranscriptItem,
) -> Result<(), IntelligenceError> {
    database
        .send(item.as_create_message_command(thread_id))
        .await
        .map(drop)
        .map_err(IntelligenceError::Save)
}

/// A session's tool, by the name its definition gives the model.
struct NamedTool {
    name: &'static str,
    tool: Box<dyn SessionTool>,
}

/// Runs `request` with the tool of `tools` it names, showing what it does
/// through `activity`: what it returned, and whether it succeeded in a call
/// that ends the turn.
async fn run_tool(
    tools: &[NamedTool],
    activity: &dyn ShowActivity,
    request: &ToolCallRequest,
) -> (ToolCallStatus, bool) {
    match tools.iter().find(|tool| tool.name == request.tool_name) {
        Some(NamedTool { tool, .. }) => {
            if let Some(doing) = tool.activity(&request.arguments) {
                activity.show(doing).await;
            }
            let status = tool.call(&request.arguments).await;
            let ends =
                matches!(status, ToolCallStatus::Success(_)) && tool.ends_turn(&request.arguments);
            (status, ends)
        }
        None => (
            ToolCallStatus::Failure(unknown_tool(tools, &request.tool_name)),
            false,
        ),
    }
}

/// What a call to `name`, which none of `tools` carries, answers: the
/// tools there are, and the JavaScript globals a model may take for tools,
/// which a script reaches instead.
fn unknown_tool(tools: &[NamedTool], name: &str) -> String {
    let quoted = |names: Vec<String>| {
        names
            .into_iter()
            .map(|name| format!("`{name}`"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let names = tools.iter().map(|tool| tool.name.to_string()).collect();
    let mut error = format!(
        "Error: There is no tool named `{name}`. Your tools are {}.",
        quoted(names)
    );
    for NamedTool {
        name: tool_name,
        tool,
    } in tools
    {
        let globals = tool.globals();
        if !globals.is_empty() {
            let globals = globals.into_iter().map(str::to_string).collect();
            error += &format!(
                " {} are JavaScript globals, not tools: call them and their methods in a \
                 script you pass to `{}`.",
                quoted(globals),
                tool_name
            );
        }
    }
    error
}

#[cfg(test)]
mod tests;
