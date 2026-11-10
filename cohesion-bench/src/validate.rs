//! Checks the cases without a model: each case's setup runs, its check
//! gives a verdict and fails on the untouched data, so a case can only pass
//! by the agent's doing, and passes once the case's reference solution has
//! run, so it can pass at all. A change to the script API that breaks a case
//! shows here, and in the crate's tests, before anyone pays for a run.

use serde_json::Value;

use crate::case::Case;
use crate::run::judge;
use crate::scratch::Scratch;

/// What is wrong with `case`, if anything.
pub async fn validate(case: &Case) -> Result<(), String> {
    let scratch = Scratch::open(None).await?;
    let checked = check_case(&scratch, case).await;
    scratch.close().await;
    checked
}

async fn check_case(scratch: &Scratch, case: &Case) -> Result<(), String> {
    if let Some(setup) = &case.setup {
        scratch
            .run_script(setup.clone())
            .await
            .map_err(|e| format!("setup: {e}"))?;
    }
    let verdict = scratch
        .run_script(case.check_script(""))
        .await
        .map_err(|e| format!("check: {e}"))?;
    let shaped = verdict.is_boolean() || verdict.get("pass").is_some_and(|pass| pass.is_boolean());
    if !shaped {
        return Err(format!(
            "check: returned {verdict}, not true, false or {{ pass, reason }}"
        ));
    }
    if judge(&verdict).0 {
        return Err("check: passes before the agent has done anything".to_string());
    }
    let solution = case
        .solution
        .clone()
        .ok_or_else(|| "no solution.js".to_string())?;
    let reply = match scratch
        .run_script(solution)
        .await
        .map_err(|e| format!("solution: {e}"))?
    {
        Value::String(reply) => reply,
        _ => String::new(),
    };
    let verdict = scratch
        .run_script(case.check_script(&reply))
        .await
        .map_err(|e| format!("check: {e}"))?;
    match judge(&verdict) {
        (true, _) => Ok(()),
        (false, reason) => Err(format!("check: fails after the solution: {reason}")),
    }
}
