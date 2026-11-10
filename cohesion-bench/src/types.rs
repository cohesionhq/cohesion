//! `types/cohesion.d.ts` in the cases directory: the script API's
//! declarations, as agents read them, so an editor checks a case against
//! the API it runs on. Every bench command rewrites it from the API it was
//! built with, so it never falls behind, and git ignores it.
//! `types/bench.d.ts` beside it declares the bench's own globals.

use std::path::{Path, PathBuf};

use cohesion_js::script_api::TS_DECLARATIONS;

const HEADER: &str =
    "// Written by `cohesion-bench types` from Cohesion's script API; don't edit.\n\n";

/// What `types/cohesion.d.ts` should hold.
pub fn declarations() -> String {
    format!("{HEADER}{TS_DECLARATIONS}")
}

/// Rewrites `types/cohesion.d.ts` under `cases`, returning its path.
pub fn write(cases: &Path) -> Result<PathBuf, String> {
    let path = cases.join("types").join("cohesion.d.ts");
    std::fs::write(&path, declarations()).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}
