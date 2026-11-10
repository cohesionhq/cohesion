//! Running a case: a scratch daemon of its own, the case's setup, the
//! user's message to an agent on the model under test, then the check and
//! what Cohesion recorded.

use std::path::Path;
use std::time::Instant;

use serde_json::Value;

use crate::case::Case;
use crate::metrics::{Activity, CaseResult, Outcome};
use crate::model_spec::ModelUnderTest;
use crate::scratch::Scratch;

/// Runs `case` on `model` with its database at `database`, its `repeat`th
/// time.
pub async fn run_case(
    case: &Case,
    model: &ModelUnderTest,
    repeat: u32,
    database: &Path,
) -> CaseResult {
    let mut result = CaseResult {
        model: model.label.clone(),
        case: case.name.clone(),
        title: case.title.clone(),
        category: case.category.clone(),
        repeat,
        outcome: Outcome::Error,
        reason: String::new(),
        seconds: 0.0,
        priced: model.prices.is_some(),
        activity: Activity::default(),
    };
    let scratch = match Scratch::open(Some(database)).await {
        Ok(scratch) => scratch,
        Err(error) => {
            result.reason = error;
            return result;
        }
    };
    if let Err(error) = converse(&scratch, case, model, &mut result).await {
        result.outcome = Outcome::Error;
        result.reason = error;
    }
    scratch.close().await;
    result
}

async fn converse(
    scratch: &Scratch,
    case: &Case,
    model: &ModelUnderTest,
    result: &mut CaseResult,
) -> Result<(), String> {
    if let Some(setup) = &case.setup {
        scratch
            .run_script(setup.clone())
            .await
            .map_err(|e| format!("setup: {e}"))?;
    }
    let agent = scratch.agent_on(model).await?;
    let started = Instant::now();
    let in_time = scratch
        .converse(&agent, case.prompt.clone(), case.timeout)
        .await?;
    result.seconds = started.elapsed().as_secs_f64();
    result.activity = Activity::read(scratch.database(), agent.id).await?;

    let verdict = scratch
        .run_script(case.check_script(&result.activity.reply))
        .await;
    let (passed, reason) = match verdict {
        Ok(verdict) => judge(&verdict),
        Err(error) => (false, format!("check threw: {error}")),
    };
    result.outcome = match passed && in_time {
        true => Outcome::Passed,
        false => Outcome::Failed,
    };
    result.reason = match (in_time, result.activity.errors.last()) {
        (false, _) => format!("timed out after {}s; {reason}", case.timeout.as_secs()),
        (true, Some(error)) if !passed => format!("{reason}; the turn failed: {error}"),
        _ => reason,
    };
    Ok(())
}

/// A check's verdict: `true`, `false` or `{ pass, reason }`.
pub fn judge(verdict: &Value) -> (bool, String) {
    match verdict {
        Value::Bool(passed) => (*passed, String::new()),
        Value::Object(object) => match object.get("pass") {
            Some(Value::Bool(passed)) => (
                *passed,
                object
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            ),
            _ => (false, format!("the check returned no `pass`: {verdict}")),
        },
        other => (false, format!("the check returned {other}")),
    }
}

#[cfg(test)]
mod tests;
