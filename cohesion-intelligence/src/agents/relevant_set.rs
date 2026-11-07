//! Which of an agent's messages its conversation's thread is about: the
//! relevant set a fresh thread's status message summarises, scored by each
//! message's depth from the newest and its age.

use cohesion_core::model::{AgentMessageContent, IntelligenceAgentMessage};
use cohesion_db::{CommandError, DatabaseContext, GetCandidateAgentMessagesCommand};
use jiff::Timestamp;
use jiff::tz::TimeZone;

use super::agent_transcript;

/// A message whose relevance falls below this leaves the relevant set.
const RELEVANCE_CUTOFF: f64 = 0.44;
/// The index factor never decays below this, however deep the message.
const INDEX_FLOOR: f64 = 0.38;
/// Messages back from the newest at which the index factor is halfway to the
/// floor: about 135 fresh messages stay relevant, 50 from a day ago.
const INDEX_HALF_LIFE: f64 = 40.0;
const INDEX_POWER: f64 = 1.0;
/// Age in seconds at which the age factor halves. A power below 1 makes the
/// age factor drop quickly at first and then flatten, so deep messages leave
/// within minutes while the newest outlast about six days.
const AGE_HALF_LIFE_SECS: f64 = 100.0 * 60.0 * 60.0;
const AGE_POWER: f64 = 0.45;
/// How many characters of transcript lines the relevant set holds at most,
/// about 40K tokens, so a run of long messages can't swell a new thread's
/// status message. The newest message is always in it.
const HISTORY_CHARS: usize = 160_000;
/// How long a script's or note's line stays relevant, however recent the
/// messages around it, in seconds: they are most of a long-lived conversation,
/// little of them matters once the work moves on, and `read_message` has them.
const WORK_LIFETIME_SECS: i64 = 60 * 60;

/// Agent `agent_id`'s messages still relevant now, oldest first, up to
/// message `through` when given.
pub(super) async fn load(
    database: &DatabaseContext,
    agent_id: i64,
    through: Option<i64>,
) -> Result<Vec<IntelligenceAgentMessage>, CommandError> {
    let mut candidates = database
        .send(GetCandidateAgentMessagesCommand { agent_id })
        .await?;
    if let Some(through) = through {
        candidates.retain(|candidate| candidate.id <= through);
    }
    Ok(relevant_set(candidates, unix_now()))
}

/// The `candidates` (newest first) still relevant at `now`, oldest first,
/// within [`HISTORY_CHARS`]. Scripts and notes past [`WORK_LIFETIME_SECS`] are
/// out and don't count; each of the rest is scored by its depth from the newest
/// message and its age, so relevance only falls going back.
fn relevant_set(
    mut candidates: Vec<IntelligenceAgentMessage>,
    now: i64,
) -> Vec<IntelligenceAgentMessage> {
    candidates.retain(|candidate| !work_past_its_lifetime(candidate, now));
    // The zone only changes how a line writes its time, not its length.
    let at = Timestamp::from_second(now)
        .expect("now is in range")
        .to_zoned(TimeZone::UTC);
    let mut budget = HISTORY_CHARS;
    let mut relevant: Vec<_> = candidates
        .into_iter()
        .enumerate()
        .take_while(|(index, message)| {
            let chars = agent_transcript::line(message, &at).len();
            let fits = *index == 0 || chars <= budget;
            budget = budget.saturating_sub(chars);
            fits && relevance(*index, now - message.sent_at) >= RELEVANCE_CUTOFF
        })
        .map(|(_, message)| message)
        .collect();
    relevant.reverse();
    relevant
}

fn work_past_its_lifetime(message: &IntelligenceAgentMessage, now: i64) -> bool {
    now - message.sent_at > WORK_LIFETIME_SECS
        && matches!(
            message.message_content,
            AgentMessageContent::CodeExecution(_) | AgentMessageContent::Note { .. }
        )
}

fn unix_now() -> i64 {
    Timestamp::now().as_second()
}

/// `(floor + (1 - floor)·½^((i/Hᵢ)^pᵢ)) · ½^((age/Hₐ)^pₐ)`, where `index` is
/// 0 for the newest message.
fn relevance(index: usize, age_secs: i64) -> f64 {
    let index_ratio = index as f64 / INDEX_HALF_LIFE;
    let age_ratio = age_secs.max(0) as f64 / AGE_HALF_LIFE_SECS;
    let index_score =
        INDEX_FLOOR + (1.0 - INDEX_FLOOR) * 0.5_f64.powf(index_ratio.powf(INDEX_POWER));
    let age_score = 0.5_f64.powf(age_ratio.powf(AGE_POWER));
    index_score * age_score
}

#[cfg(test)]
mod tests;
