use std::time::Duration;

/// How much a [`Sandbox`](super::Sandbox) may use: memory for its runtime,
/// and time for each job it runs.
#[derive(Clone, Copy)]
pub struct SandboxOptions {
    pub memory_limit_bytes: usize,
    pub time_limit: Duration,
}

impl SandboxOptions {
    /// For scripts from someone trusted: the user's, agents', cues' and
    /// installed extensions'. 20 MB, and a minute per job.
    pub const STANDARD: Self = Self {
        memory_limit_bytes: 20 * 1024 * 1024,
        time_limit: Duration::from_secs(60),
    };

    /// For drawing a Mermaid diagram. The densest diagram Pele's own limits let
    /// through needs 26 MB and 6.9 s to lay out for a phone, so this leaves
    /// room for a slower machine.
    pub const MERMAID: Self = Self {
        memory_limit_bytes: 64 * 1024 * 1024,
        time_limit: Duration::from_secs(15),
    };

    /// For computing a table formula for one row: the row in, a value
    /// out. A formula is arithmetic over a few values, so 16 MB for a
    /// runtime and its cached formulas, and a second per row, is room to
    /// spare for a real one, while a loop that never ends costs a second.
    pub const FORMULA: Self = Self {
        memory_limit_bytes: 16 * 1024 * 1024,
        time_limit: Duration::from_secs(1),
    };

    /// For source read before anyone has trusted it, such as an extension's
    /// script while its manifest is read. Reading a manifest only defines
    /// functions and calls one, so 4 MB and a second per job is room to
    /// spare for a real script and little for a hostile one.
    pub const ISOLATED: Self = Self {
        memory_limit_bytes: 4 * 1024 * 1024,
        time_limit: Duration::from_secs(1),
    };
}
