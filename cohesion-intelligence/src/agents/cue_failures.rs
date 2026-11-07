use cohesion_core::model::IntelligenceAgentCue;
use cohesion_db::{CommandError, DatabaseContext, DatabaseEventTrace, ReportAgentErrorCommand};
use cohesion_js::ScriptError;

/// How many characters of a cue's error its feed message keeps, so a long
/// stack can't fill the feed.
const ERROR_CHARS: usize = 500;

/// Tells the user that `cue`'s function failed with `error`, every time it
/// does (a timer cue that keeps failing says so each minute): a fork's
/// `error` message in its agent's feed continuing `trace`. An engine that
/// stopped before replying (the daemon shutting down) is no fault of the
/// cue's, and reports nothing.
pub(super) async fn report(
    database: &DatabaseContext,
    cue: &IntelligenceAgentCue,
    error: &ScriptError,
    trace: DatabaseEventTrace,
) -> Result<(), CommandError> {
    if let ScriptError::Cancelled = error {
        return Ok(());
    }
    database
        .send_traced(
            ReportAgentErrorCommand {
                agent_id: cue.agent_id,
                error: failure_text(cue, error),
            },
            trace,
        )
        .await?;
    Ok(())
}

/// What the feed says of a failed cue: `The cue "Triage" failed on
/// task:created: script threw: …`, the error cut to [`ERROR_CHARS`].
pub(super) fn failure_text(cue: &IntelligenceAgentCue, error: &ScriptError) -> String {
    let error = error.to_string();
    let error = match error.char_indices().nth(ERROR_CHARS) {
        Some((end, _)) => format!("{}…", error[..end].trim_end()),
        None => error,
    };
    format!("The cue \"{}\" failed on {}: {error}", cue.name, cue.hook)
}
