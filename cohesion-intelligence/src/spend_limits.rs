use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use cohesion_core::model::{IntelligenceSpend, SpendPeriod};
use cohesion_db::{
    CommandError, DatabaseContext, DatabaseEventTrace, ListIntelligenceSpendCommand,
    ReportAgentErrorCommand,
};

/// Keeps work the user isn't watching (forks) from making model
/// calls once a spend limit is reached, telling each agent so in its feed
/// once per period, then again only after that period falls back under
/// its limit. The user's own turns aren't held back. Which agents were told
/// is kept in memory: a restart tells an agent still held back once more.
#[derive(Default)]
pub(crate) struct SpendLimits {
    told: Mutex<HashSet<(i64, SpendPeriod)>>,
}

impl SpendLimits {
    /// The spend of the shortest period whose limit is reached, if any,
    /// which keeps agent `agent_id`'s forks from their next model
    /// call. The first time a period holds the agent back, says so in its
    /// feed as a fork's `error` message continuing `trace`.
    pub(crate) async fn reached(
        &self,
        database: &DatabaseContext,
        agent_id: i64,
        trace: &DatabaseEventTrace,
    ) -> Result<Option<IntelligenceSpend>, CommandError> {
        let reached: Vec<IntelligenceSpend> = database
            .send(ListIntelligenceSpendCommand {})
            .await?
            .into_iter()
            .filter(|spend| spend.reached)
            .collect();
        // A period back under its limit tells every agent again next time.
        self.told()
            .retain(|(_, period)| reached.iter().any(|spend| spend.period == *period));
        let Some(spend) = reached.into_iter().next() else {
            return Ok(None);
        };
        // Claimed before the post, so two checks at once tell it once.
        if !self.told().insert((agent_id, spend.period)) {
            return Ok(Some(spend));
        }
        let posted = database
            .send_traced(
                ReportAgentErrorCommand {
                    agent_id,
                    error: notice_text(&spend),
                },
                trace.clone(),
            )
            .await;
        if let Err(e) = posted {
            self.told().remove(&(agent_id, spend.period));
            return Err(e);
        }
        Ok(Some(spend))
    }

    fn told(&self) -> std::sync::MutexGuard<'_, HashSet<(i64, SpendPeriod)>> {
        self.told
            .lock()
            .expect("the spend limits lock is not poisoned")
    }
}

/// What holds a session's model calls to the spend limits: agent
/// `agent_id`'s forks, whose notice continues `trace`.
pub(crate) struct SpendCap {
    pub(crate) limits: Arc<SpendLimits>,
    pub(crate) agent_id: i64,
    pub(crate) trace: DatabaseEventTrace,
}

/// What a reached limit is, as a sentence: `The daily spend limit of $5.00
/// is reached ($5.12 in the last 24 hours).`
pub(crate) fn limit_text(spend: &IntelligenceSpend) -> String {
    let (limit, period) = match spend.period {
        SpendPeriod::Hour => ("hourly", "60 minutes"),
        SpendPeriod::Day => ("daily", "24 hours"),
        SpendPeriod::Month => ("monthly", "30 days"),
    };
    format!(
        "The {limit} spend limit of ${:.2} is reached (${:.2} in the last {period}).",
        spend.limit.unwrap_or_default(),
        spend.spent
    )
}

/// What an agent's feed says when a limit first holds its forks
/// back.
fn notice_text(spend: &IntelligenceSpend) -> String {
    format!(
        "{} Agents make no model calls but to answer your messages until spending falls \
         below it.",
        limit_text(spend)
    )
}
