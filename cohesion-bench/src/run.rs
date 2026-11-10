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
use crate::starting_state::StartingState;

/// Runs `case` on `model` with its database at `database`, its `repeat`th
/// time, its prompt's dates counted from `today`.
pub async fn run_case(
    case: &Case,
    model: &ModelUnderTest,
    repeat: u32,
    database: &Path,
    today: jiff::civil::Date,
) -> CaseResult {
    let mut result = CaseResult {
        model: model.label.clone(),
        case: case.name.clone(),
        title: case.title.clone(),
        category: case.category.clone(),
        difficulty: case.difficulty,
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
    if let Err(error) = converse(&scratch, case, model, today, &mut result).await {
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
    today: jiff::civil::Date,
    result: &mut CaseResult,
) -> Result<(), String> {
    let prompt = case.prompt_on(today)?;
    prepare(scratch, case).await?;
    let agent = scratch.agent_on(model).await?;
    let started = Instant::now();
    let in_time = scratch.converse(&agent, prompt, case.timeout).await?;
    result.seconds = started.elapsed().as_secs_f64();
    result.activity = Activity::read(scratch.database(), agent.id).await?;

    let verdict = match scratch
        .run_script(case.validate_script(&result.activity.left()))
        .await
    {
        Ok(verdict) => judge(&verdict),
        Err(error) => Err(format!("validate.ts threw: {error}")),
    };
    let reason = match (&verdict, in_time) {
        (_, false) => format!(
            "timed out after {}s{}",
            case.timeout.as_secs(),
            verdict
                .as_ref()
                .err()
                .map(|reason| format!("; {reason}"))
                .unwrap_or_default()
        ),
        (Ok(()), true) => String::new(),
        (Err(reason), true) => match result.activity.errors.last() {
            Some(error) => format!("{reason}; the turn failed: {error}"),
            None => reason.clone(),
        },
    };
    let passed = verdict.is_ok() && in_time;
    result.outcome = if passed {
        Outcome::Passed
    } else {
        Outcome::Failed
    };
    result.reason = reason;
    Ok(())
}

/// Fills `scratch` as `case` starts: its starting state read and its
/// workspaces made, then the state applied, the case's own setup run and
/// the snapshot taken.
pub async fn prepare(scratch: &Scratch, case: &Case) -> Result<(), String> {
    let state = match &case.state {
        Some(script) => {
            let value = scratch
                .run_script(case.script(script))
                .await
                .map_err(|e| format!("starting state: {e}"))?;
            serde_json::from_value::<StartingState>(value)
                .map_err(|e| format!("starting state: {e}"))?
        }
        None => StartingState::empty(),
    };
    let workspaces = case
        .workspaces
        .clone()
        .unwrap_or_else(|| state.workspace_titles());
    scratch.create_workspaces(&workspaces).await?;
    scratch
        .run_script(case.setup_script(&state))
        .await
        .map_err(|e| format!("setup: {e}"))?;
    Ok(())
}

/// What `validate.ts` returned, as a pass or the reason it failed: nothing
/// (`undefined` or `null`) or `false` passes, a string fails, and anything
/// else is a check that doesn't follow the rule, so it fails too.
pub fn judge(verdict: &Value) -> Result<(), String> {
    match verdict {
        Value::Null | Value::Bool(false) => Ok(()),
        Value::String(reason) if reason.trim().is_empty() => {
            Err("validate.ts failed without saying why".to_string())
        }
        Value::String(reason) => Err(reason.clone()),
        other => Err(format!(
            "validate.ts returned {other}; return nothing to pass, or a string saying what failed"
        )),
    }
}

#[cfg(test)]
mod tests;
