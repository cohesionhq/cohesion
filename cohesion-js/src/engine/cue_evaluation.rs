use crate::script_output::ScriptOutput;

/// What calling an agent's cue with a hook's subject gave.
#[derive(Debug)]
pub struct CueEvaluation {
    /// Whether the cue's function returned something truthy.
    pub fired: bool,
    /// The first argument as JSON (`"null"` when there is none), for the agent's wake-up message.
    pub subject: ScriptOutput,
    /// What the run receives, as JSON: the subject when the function returned exactly `true`,
    /// otherwise what it returned.
    pub payload: ScriptOutput,
}
