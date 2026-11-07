//! Options objects the agent globals take.

use cohesion_core::model::{HookKind, IntelligenceEffort};
use cohesion_macros::JsOptions;

/// Options for `agent.cue()`.
#[derive(JsOptions)]
pub struct AgentCueOptions {
    /// A few words naming what the cue is for, such as `"Triage new tasks"`.
    ///
    /// The user sees it, and it is how you replace or remove the cue later.
    pub name: String,
    /// The hook that wakes you, such as `"task:create"`.
    pub hook: HookKind,
    /// A function expression, as source text, called with the hook's subject
    /// each time the hook fires: the `Task` for `task:*`, the `TaskFolder` for
    /// `taskfolder:*`, the `Workspace` for `workspace:*`, the journal's
    /// `Document` for `journal:settled`, the `Resource` for `resource:*`, the
    /// `AgentMessage` for `agent:message` and, as reacted to, for
    /// `agent:reaction` (never one from your own conversation), the
    /// `Activity` for `activity:*` and the `ActorSession` for `actor:moved`
    /// (load `actors`), each as it stood then, nothing for
    /// `launch`, `day:start` (6:00 local time) or `day:end` (23:00), and for
    /// `timer`, at the start of every local minute,
    /// `{ time, hour, minute, since }`: when the minute began in milliseconds
    /// since the epoch, the hour (0 to 23) and minute (0 to 59) on the local
    /// clock, and when the minute it last fired for began.
    ///
    /// Minutes missed while the computer slept fire once on waking, so `since`
    /// can be hours back. To wake at a set time, test that the time falls in
    /// `(since, time]`:
    /// `"({ since, time }) => { const at = Date.parse('2026-10-18T15:05'); return since < at && at <= time; }"`;
    /// for every day at 9:30, `"({ hour, minute }) => hour === 9 && minute === 30"`;
    /// for the start of every hour, `"({ minute }) => minute === 0"`. For
    /// `extension:update`, which fires once for each version a registry offers
    /// an installed extension at, it is
    /// `{ extensionId, identifier, name, installedVersion, version, sourceURL }`:
    /// the extension's id, manifest identifier, name and installed version, the
    /// version offered, and the URL serving that script. For
    /// `notification:send` it is the `Notification`, `{ title, message }`, that
    /// a script or another agent sent the user, and for `client:revoke` the
    /// `RevokedClient`, `{ id, name }`. Returning `true` wakes
    /// you with the whole subject. Returning any other truthy
    /// value, such as an object with only the fields you will need, wakes you
    /// with that value instead, which saves tokens. Returning something falsy
    /// does nothing. It reads `cohesion` but can't change it, and may call
    /// services' read-only functions with `services.use(...)`. Keep it cheap
    /// and side-effect free, for example
    /// `"(task) => task.priority === TaskPriority.urgent"` or
    /// `"(task) => task.status === TaskStatus.completed && { title: task.title }"`.
    pub code: String,
    /// What you, woken, read first: what to look at and what to do or ask
    /// the user.
    ///
    /// What `code` returned follows it. The fork it wakes reads `cohesion`
    /// without changing it and can't set cues, so ask it to report or to ask
    /// the user (`ask_choice`) rather than to change things itself.
    pub instructions: String,
    /// How capable a model the fork each firing wakes gets:
    /// `"low"` suits quick checks and short replies, `"medium"` ordinary work
    /// and `"high"` only hard, multi-step reasoning.
    ///
    /// Each step up costs more per run, and the cue may fire often, so pick
    /// the lowest that will do the job.
    pub effort: IntelligenceEffort,
}
