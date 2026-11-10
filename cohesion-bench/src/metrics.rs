//! What a case's run measured, read back from the scratch database the way
//! Cohesion recorded it (the agent's messages, its threads' tool calls and
//! their results, the cost events), and the sums over a model's runs.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use cohesionkit::db::{
    DatabaseContext, GetIntelligenceThreadMessagesCommand, ListAgentCuesCommand,
    ListAgentMessagesCommand, ListIntelligenceCostEventsCommand, ListMemoriesCommand,
};

use crate::case::{CueLeft, Difficulty, Left};
use crate::score;
use cohesionkit::model::{AgentMessageContent, IntelligenceAgentMessageSender, ThreadItem};

/// The code tool, whose calls run the script API.
pub const CODE_TOOL: &str = "code-tool";
/// The tool that hands out a script module's declarations.
pub const DOCUMENTATION_TOOL: &str = "documentation";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The check passed.
    Passed,
    /// The check failed, the turn failed or it ran out of time.
    Failed,
    /// The bench couldn't run the case: its setup or check broke, or the
    /// scratch daemon refused something. Not the model's doing.
    Error,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Error => "error",
        }
    }
}

/// How often one tool was called in a run, and how many of those calls
/// failed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ToolUse {
    pub calls: u32,
    pub failed: u32,
}

/// What the agent did and what it cost, as recorded.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Activity {
    /// Model calls: one cost event each.
    pub model_calls: u32,
    pub tools: BTreeMap<String, ToolUse>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    /// In USD.
    pub cost: f64,
    /// Messages the agent sent the user.
    pub messages_sent: u32,
    /// The questions it asked the user to pick an answer to.
    pub asked: Vec<String>,
    /// What the agent said to the user, its messages and questions joined
    /// by blank lines.
    pub reply: String,
    /// The errors its turns posted.
    pub errors: Vec<String>,
    /// The cues it holds once its turns have ended.
    pub cues: Vec<CueLeft>,
    /// The memories it holds once its turns have ended, its own and those
    /// every agent shares.
    pub memories: Vec<String>,
}

/// One run of one case on one model.
#[derive(Clone, Debug)]
pub struct CaseResult {
    pub model: String,
    pub case: String,
    pub title: String,
    pub category: String,
    pub difficulty: Difficulty,
    /// Which repetition, from 1.
    pub repeat: u32,
    pub outcome: Outcome,
    /// Why it passed or failed, as the check or the bench put it.
    pub reason: String,
    /// From the user's message to the end of everything it set going.
    pub seconds: f64,
    /// Whether the model's prices were known, so `cost` means something.
    pub priced: bool,
    pub activity: Activity,
}

impl Activity {
    pub fn tool_calls(&self) -> u32 {
        self.tools.values().map(|tool| tool.calls).sum()
    }

    pub fn failed_tool_calls(&self) -> u32 {
        self.tools.values().map(|tool| tool.failed).sum()
    }

    pub fn calls_to(&self, tool: &str) -> u32 {
        self.tools.get(tool).map_or(0, |tool| tool.calls)
    }

    /// Every token the provider counted in prompts, cached or not.
    pub fn prompt_tokens(&self) -> i64 {
        self.input_tokens + self.cache_read_tokens + self.cache_write_tokens
    }

    pub fn total_tokens(&self) -> i64 {
        self.prompt_tokens() + self.output_tokens
    }

    /// What a check reads besides the database.
    pub fn left(&self) -> Left {
        Left {
            reply: self.reply.clone(),
            cues: self.cues.clone(),
            memories: self.memories.clone(),
        }
    }

    /// Of the calls made, the share that succeeded; `None` with no calls.
    pub fn tool_accuracy(&self) -> Option<f64> {
        ratio(
            (self.tool_calls() - self.failed_tool_calls()) as f64,
            self.tool_calls() as f64,
        )
    }

    /// Reads what agent `agent_id` did from `database`, which holds that
    /// agent's runs only.
    pub async fn read(database: &DatabaseContext, agent_id: i64) -> Result<Self, String> {
        let mut activity = Self::default();
        let messages = database
            .send(ListAgentMessagesCommand {
                agent_id,
                before: None,
                limit: i64::from(u32::MAX),
            })
            .await
            .map_err(|e| format!("{e:?}"))?;
        let mut reply = Vec::new();
        for message in messages {
            if message.sender == IntelligenceAgentMessageSender::User {
                continue;
            }
            match message.message_content {
                AgentMessageContent::Text { text } => {
                    activity.messages_sent += 1;
                    reply.push(text);
                }
                AgentMessageContent::Choice(choice) => {
                    let question = format!("{} [{}]", choice.question, choice.options.join(" / "));
                    activity.asked.push(question.clone());
                    reply.push(question);
                }
                AgentMessageContent::Error { error } => activity.errors.push(error),
                _ => {}
            }
        }
        activity.reply = reply.join("\n\n");
        let fail = |e| format!("{e:?}");
        activity.cues = database
            .send(ListAgentCuesCommand { agent_id })
            .await
            .map_err(fail)?
            .into_iter()
            .map(|cue| CueLeft {
                name: cue.name,
                hook: cue.hook.to_string(),
                code: cue.script,
                instructions: cue.instructions,
                effort: cue.effort.to_string(),
            })
            .collect();
        activity.memories = database
            .send(ListMemoriesCommand {
                agent_id: Some(agent_id),
            })
            .await
            .map_err(fail)?
            .into_iter()
            .map(|memory| memory.memory)
            .collect();

        let events = database
            .send(ListIntelligenceCostEventsCommand {
                limit: u64::from(u32::MAX),
            })
            .await
            .map_err(|e| format!("{e:?}"))?;
        let mut threads = BTreeSet::new();
        for event in &events {
            activity.model_calls += 1;
            activity.input_tokens += event.input_tokens;
            activity.output_tokens += event.output_tokens;
            activity.cache_read_tokens += event.cache_read_tokens;
            activity.cache_write_tokens += event.cache_write_tokens;
            activity.cost += event.total_cost;
            threads.extend(event.thread_id);
        }

        // A fork's thread repeats the items it inherits, so each item
        // counts once, but a thread pairs its responses with every request
        // it holds.
        let mut counted = HashSet::new();
        for thread_id in threads {
            let entries = database
                .send(GetIntelligenceThreadMessagesCommand { thread_id })
                .await
                .map_err(|e| format!("{e:?}"))?;
            let mut called = HashMap::new();
            for entry in entries {
                let fresh = counted.insert(entry.message.id);
                match entry.message.item {
                    ThreadItem::ToolCallRequest {
                        call_identifier,
                        tool_name,
                        ..
                    } => {
                        if fresh {
                            activity.tools.entry(tool_name.clone()).or_default().calls += 1;
                        }
                        called.insert(call_identifier, tool_name);
                    }
                    ThreadItem::ToolCallResponse {
                        call_identifier,
                        succeeded: false,
                        ..
                    } if fresh => {
                        let name = called
                            .get(&call_identifier)
                            .cloned()
                            .unwrap_or_else(|| "unknown".to_string());
                        activity.tools.entry(name).or_default().failed += 1;
                    }
                    _ => {}
                }
            }
        }
        Ok(activity)
    }
}

/// One model's runs, summed.
#[derive(Clone, Debug)]
pub struct Summary {
    pub model: String,
    pub runs: u32,
    pub passed: u32,
    pub failed: u32,
    pub errors: u32,
    /// The scores, from 0 to 100 (`score.rs`); `None` with nothing to go on.
    pub accuracy: Option<f64>,
    pub efficiency: Option<f64>,
    pub overall: Option<f64>,
    /// Accuracy over each difficulty's cases alone, easy to hard.
    pub accuracy_by_difficulty: [Option<f64>; 3],
    pub tool_calls: u32,
    pub failed_tool_calls: u32,
    pub code_calls: u32,
    pub documentation_calls: u32,
    pub cost: f64,
    /// Whether every run was priced.
    pub priced: bool,
    pub mean_seconds: f64,
    pub median_seconds: f64,
    pub p90_seconds: f64,
    pub output_tokens_per_second: f64,
    pub mean_model_calls: f64,
    pub max_model_calls: u32,
    pub mean_tool_calls: f64,
    pub mean_total_tokens: f64,
    pub mean_output_tokens: f64,
    /// Every token spent, over the runs that passed.
    pub tokens_per_pass: Option<f64>,
    /// Of the prompt tokens, the share read from the provider's cache.
    pub cache_hit_rate: Option<f64>,
    /// Runs that asked the user to pick.
    pub asked_user: u32,
}

impl CaseResult {
    /// How efficiently it ran, from 0 to 100; `None` unless it passed.
    pub fn efficiency(&self) -> Option<f64> {
        (self.outcome == Outcome::Passed).then(|| {
            score::run_efficiency(
                self.difficulty,
                self.activity.total_tokens() as f64,
                f64::from(self.activity.model_calls),
                self.seconds,
            )
        })
    }
}

/// Accuracy over `results`, from 0 to 100, leaving out the bench's own
/// errors (`score.rs`).
pub fn accuracy(results: &[&CaseResult]) -> Option<f64> {
    let (got, possible) = results.iter().filter(|r| r.outcome != Outcome::Error).fold(
        (0.0, 0.0),
        |(got, possible), r| {
            let weight = score::weight(r.difficulty);
            let got_it = if r.outcome == Outcome::Passed {
                weight
            } else {
                0.0
            };
            (got + got_it, possible + weight)
        },
    );
    ratio(100.0 * got, possible)
}

impl Summary {
    /// Sums `results`, which are all one model's.
    pub fn of(model: &str, results: &[&CaseResult]) -> Self {
        let runs = results.len() as u32;
        let count = |outcome| results.iter().filter(|r| r.outcome == outcome).count() as u32;
        let sum = |value: fn(&CaseResult) -> f64| results.iter().map(|r| value(r)).sum::<f64>();
        let mean = |value: fn(&CaseResult) -> f64| ratio(sum(value), runs as f64).unwrap_or(0.0);
        let mut seconds: Vec<f64> = results.iter().map(|r| r.seconds).collect();
        seconds.sort_by(f64::total_cmp);
        let passed = count(Outcome::Passed);
        let total_tokens = sum(|r| r.activity.total_tokens() as f64);
        let accuracy_score = accuracy(results);
        let efficiencies: Vec<f64> = results.iter().filter_map(|r| r.efficiency()).collect();
        let efficiency = ratio(efficiencies.iter().sum(), efficiencies.len() as f64);
        let overall = accuracy_score.map(|a| score::overall(a, efficiency.unwrap_or(0.0)));
        Self {
            model: model.to_string(),
            runs,
            passed,
            failed: count(Outcome::Failed),
            errors: count(Outcome::Error),
            accuracy: accuracy_score,
            efficiency,
            overall,
            accuracy_by_difficulty: Difficulty::ALL.map(|difficulty| {
                let of: Vec<&CaseResult> = results
                    .iter()
                    .copied()
                    .filter(|r| r.difficulty == difficulty)
                    .collect();
                accuracy(&of)
            }),
            tool_calls: results.iter().map(|r| r.activity.tool_calls()).sum(),
            failed_tool_calls: results.iter().map(|r| r.activity.failed_tool_calls()).sum(),
            code_calls: results.iter().map(|r| r.activity.calls_to(CODE_TOOL)).sum(),
            documentation_calls: results
                .iter()
                .map(|r| r.activity.calls_to(DOCUMENTATION_TOOL))
                .sum(),
            cost: sum(|r| r.activity.cost),
            priced: results.iter().all(|r| r.priced),
            mean_seconds: mean(|r| r.seconds),
            median_seconds: percentile(&seconds, 0.5),
            p90_seconds: percentile(&seconds, 0.9),
            output_tokens_per_second: ratio(
                sum(|r| r.activity.output_tokens as f64),
                sum(|r| r.seconds),
            )
            .unwrap_or(0.0),
            mean_model_calls: mean(|r| f64::from(r.activity.model_calls)),
            max_model_calls: results
                .iter()
                .map(|r| r.activity.model_calls)
                .max()
                .unwrap_or(0),
            mean_tool_calls: mean(|r| f64::from(r.activity.tool_calls())),
            mean_total_tokens: mean(|r| r.activity.total_tokens() as f64),
            mean_output_tokens: mean(|r| r.activity.output_tokens as f64),
            tokens_per_pass: ratio(total_tokens, passed as f64),
            cache_hit_rate: ratio(
                sum(|r| r.activity.cache_read_tokens as f64),
                sum(|r| r.activity.prompt_tokens() as f64),
            ),
            asked_user: results
                .iter()
                .filter(|r| !r.activity.asked.is_empty())
                .count() as u32,
        }
    }

    pub fn success_rate(&self) -> Option<f64> {
        ratio(self.passed as f64, self.runs as f64)
    }

    pub fn tool_accuracy(&self) -> Option<f64> {
        ratio(
            (self.tool_calls - self.failed_tool_calls) as f64,
            self.tool_calls as f64,
        )
    }

    pub fn cost_per_run(&self) -> Option<f64> {
        ratio(self.cost, self.runs as f64)
    }

    pub fn cost_per_pass(&self) -> Option<f64> {
        ratio(self.cost, self.passed as f64)
    }
}

/// Every model's summary, in the order the models were named.
pub fn summaries(models: &[String], results: &[CaseResult]) -> Vec<Summary> {
    models
        .iter()
        .map(|model| {
            let runs: Vec<&CaseResult> = results.iter().filter(|r| &r.model == model).collect();
            Summary::of(model, &runs)
        })
        .collect()
}

fn ratio(numerator: f64, denominator: f64) -> Option<f64> {
    (denominator > 0.0).then(|| numerator / denominator)
}

/// The value at `fraction` of the way through `sorted`, by nearest rank;
/// 0 when empty.
fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (fraction * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

#[cfg(test)]
mod tests;
