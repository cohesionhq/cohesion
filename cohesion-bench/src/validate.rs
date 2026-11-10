//! Checks the cases without a model: each case's setup runs, its check
//! gives a verdict and fails on the untouched data, so a case can only pass
//! by the agent's doing, and passes once the case's reference solution has
//! run, so it can pass at all. A change to the script API that breaks a case
//! shows here, and in the crate's tests, before anyone pays for a run.

use std::sync::Arc;

use serde_json::Value;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::case::{Case, Left};
use crate::run::{judge, prepare};
use crate::scratch::Scratch;

/// Validates `cases`, `jobs` at once; what is wrong with each, in their
/// order.
pub async fn validate_all(cases: &[Case], jobs: usize) -> Vec<Result<(), String>> {
    let slots = Arc::new(Semaphore::new(jobs.max(1)));
    let mut running = JoinSet::new();
    for (index, case) in cases.iter().cloned().enumerate() {
        let slots = slots.clone();
        running.spawn(async move {
            let _slot = slots
                .acquire_owned()
                .await
                .expect("the semaphore stays open");
            (index, validate(&case).await)
        });
    }
    let mut outcomes: Vec<(usize, Result<(), String>)> = Vec::with_capacity(cases.len());
    while let Some(joined) = running.join_next().await {
        outcomes
            .push(joined.unwrap_or_else(|e| (usize::MAX, Err(format!("its task failed: {e}")))));
    }
    outcomes.sort_by_key(|(index, _)| *index);
    outcomes.into_iter().map(|(_, outcome)| outcome).collect()
}

/// What is wrong with `case`, if anything.
pub async fn validate(case: &Case) -> Result<(), String> {
    let scratch = Scratch::open(None).await?;
    let checked = check_case(&scratch, case).await;
    scratch.close().await;
    checked
}

async fn check_case(scratch: &Scratch, case: &Case) -> Result<(), String> {
    case.prompt_on(jiff::Zoned::now().date())?;
    prepare(scratch, case).await?;
    // On the untouched data the check must fail: by saying why, or by
    // throwing on something the agent hasn't made yet.
    if let Ok(untouched) = scratch
        .run_script(case.validate_script(&Left::default()))
        .await
    {
        match judge(&untouched) {
            Ok(()) => {
                return Err("validate.ts passes before the agent has done anything".to_string());
            }
            Err(reason) if !untouched.is_string() => return Err(reason),
            Err(_) => {}
        }
    }
    let solution = case.solution.as_deref().ok_or("no solution.ts")?;
    let left = match scratch
        .run_script(case.script(solution))
        .await
        .map_err(|e| format!("solution.ts: {e}"))?
    {
        Value::String(reply) => Left {
            reply,
            ..Left::default()
        },
        Value::Object(object) => serde_json::from_value(Value::Object(object))
            .map_err(|e| format!("solution.ts: what it returned isn't a `bench`: {e}"))?,
        _ => Left::default(),
    };
    let solved = scratch
        .run_script(case.validate_script(&left))
        .await
        .map_err(|e| format!("validate.ts: {e}"))?;
    judge(&solved).map_err(|reason| format!("validate.ts fails after solution.ts: {reason}"))
}
