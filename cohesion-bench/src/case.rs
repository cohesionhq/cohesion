//! A bench case: a directory under `cases/` holding what the user asks
//! (`prompt.md`), the data the agent finds (`setup.js`, optional), how the
//! outcome is judged (`check.js`), a reference solution (`solution.js`) and a
//! little about it (`case.json`).
//!
//! Both scripts run on the scratch daemon with the user's rights, through
//! the same `RunScript` request `cohesion run` sends, against a workspace
//! called `Personal`. The check's last expression is its verdict: `true`,
//! `false`, or `{ pass, reason }`; `reply` holds what the agent said to the
//! user, its messages joined by blank lines. The solution does what a good
//! agent would; a string it ends in stands for the agent's reply. Only
//! `validate` runs it, to show the check can pass.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

/// The workspace every case starts with.
pub const WORKSPACE: &str = "Personal";

/// How long a case runs before it is stopped, unless it says otherwise.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Clone, Debug)]
pub struct Case {
    /// The directory's name.
    pub name: String,
    pub title: String,
    /// What it exercises, such as `tasks` or `documents`.
    pub category: String,
    pub prompt: String,
    pub setup: Option<String>,
    pub check: String,
    pub solution: Option<String>,
    pub timeout: Duration,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CaseFile {
    title: String,
    category: String,
    timeout_seconds: Option<u64>,
}

impl Case {
    fn load(directory: &Path) -> Result<Self, String> {
        let read = |file: &str| {
            std::fs::read_to_string(directory.join(file))
                .map_err(|e| format!("{}: {e}", directory.join(file).display()))
        };
        let about: CaseFile = serde_json::from_str(&read("case.json")?)
            .map_err(|e| format!("{}: {e}", directory.join("case.json").display()))?;
        let optional = |file: &str| match directory.join(file).exists() {
            true => read(file).map(Some),
            false => Ok(None),
        };
        Ok(Self {
            name: directory
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            title: about.title,
            category: about.category,
            prompt: read("prompt.md")?.trim().to_string(),
            setup: optional("setup.js")?,
            check: read("check.js")?,
            solution: optional("solution.js")?,
            timeout: about
                .timeout_seconds
                .map(Duration::from_secs)
                .unwrap_or(DEFAULT_TIMEOUT),
        })
    }

    /// The check script with `reply` declared ahead of it.
    pub fn check_script(&self, reply: &str) -> String {
        format!(
            "const reply = {};\n{}",
            serde_json::Value::from(reply),
            self.check
        )
    }
}

/// Every case in `directory`, by name; only those named in `only` when it
/// isn't empty.
pub fn load_cases(directory: &Path, only: &[String]) -> Result<Vec<Case>, String> {
    let entries =
        std::fs::read_dir(directory).map_err(|e| format!("{}: {e}", directory.display()))?;
    let mut directories: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.join("case.json").exists())
        .collect();
    directories.sort();
    let cases = directories
        .iter()
        .map(|directory| Case::load(directory))
        .collect::<Result<Vec<_>, _>>()?;
    for name in only {
        if !cases.iter().any(|case| &case.name == name) {
            return Err(format!("{name}: no such case in {}", directory.display()));
        }
    }
    Ok(cases
        .into_iter()
        .filter(|case| only.is_empty() || only.contains(&case.name))
        .collect())
}

/// The cases that ship with the bench.
pub fn default_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("cases")
}
