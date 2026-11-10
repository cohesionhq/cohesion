//! A bench case: a directory `<category>/<name>/` in the cases directory
//! (the bench's own `cases/`, or `tests` in `bench.json`), holding:
//!
//! - `prompt.md`: a front-matter block of `key: value` lines between `---`
//!   lines, then what the user says. The keys are `title` and `difficulty`
//!   (`easy`, `medium` or `hard`, which weighs the case in the accuracy
//!   score and sets its efficiency references), and optionally `state`
//!   (the starting state in `states/`: `default` when left out, `none` for
//!   an empty `Personal` workspace), `workspaces` (the workspaces to make,
//!   comma-separated, when not the state's) and `timeout` (seconds; 300
//!   when left out). In the message, `{{day:N}}` is the local date N days
//!   from today (`{{day:0}}` is today), as `YYYY-MM-DD`.
//! - `validate.ts`: `export default function validate()`, which passes by
//!   returning nothing (`undefined`, `null` or `false`) and fails by
//!   returning a string saying what is wrong. It reads the database
//!   through `cohesion`, and `bench`: what the agent told the user
//!   (`reply`, also declared alone), the questions it asked (`asked`), the
//!   cues it set (`cues`) and the memories it holds (`memories`).
//! - `setup.ts` (optional): `export default function setup()`, adding to
//!   the starting state what only this case needs.
//! - `solution.ts`: `export default function solution()`, doing what a
//!   good agent would and returning what it would leave (a `bench`, or a
//!   string standing for its reply). Only `validate` runs it, to show the
//!   case can pass.
//!
//! Every prompt starts with `prompt-prelude.md`, which tells the agent not
//! to ask before acting: a run has no one to answer, so the bench scores the
//! work done, not whether the agent checked first.
//!
//! Every script runs on the scratch daemon with the user's rights, through
//! the same `RunScript` request `cohesion run` sends, after `prelude.ts`,
//! whose `B` holds the helpers they share. The types in `types/` and
//! `tsconfig.json` let an editor check a case as it is written. The setup
//! applies the starting state first and ends by recording the state it
//! leaves, so a check can tell whether anything changed (`B.unchanged()`).

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::starting_state::StartingState;

/// The starting state a case starts from when it names none.
const DEFAULT_STATE: &str = "default";

/// How long a case runs before it is stopped, unless it says otherwise.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

/// The file in the cases directory whose text starts every case's prompt.
const PROMPT_PRELUDE: &str = "prompt-prelude.md";

/// Directories in the cases directory that hold no cases.
const NOT_CATEGORIES: [&str; 3] = ["states", "types", "node_modules"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}

impl Difficulty {
    pub const ALL: [Self; 3] = [Self::Easy, Self::Medium, Self::Hard];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Easy => "easy",
            Self::Medium => "medium",
            Self::Hard => "hard",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.as_str() == text)
    }
}

#[derive(Clone, Debug)]
pub struct Case {
    /// The directory's name, unique across categories.
    pub name: String,
    pub title: String,
    /// The directory it is in, such as `tasks-edit` or `documents`.
    pub category: String,
    pub difficulty: Difficulty,
    /// The script whose value is the starting state; `None` for an empty
    /// `Personal` workspace.
    pub state: Option<String>,
    /// The workspaces to make, when the case names them rather than taking
    /// the state's.
    pub workspaces: Option<Vec<String>>,
    /// The user's message, with `{{day:N}}` still in it.
    pub prompt: String,
    /// `setup.ts`, `validate.ts` and `solution.ts`, as scripts whose value is
    /// what their function returns.
    pub setup: Option<String>,
    pub validate: String,
    pub solution: Option<String>,
    pub timeout: Duration,
    /// `prelude.ts`, run ahead of every script.
    pub prelude: String,
}

/// What the agent left that a check reads besides the database.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Left {
    pub reply: String,
    pub asked: Vec<String>,
    pub cues: Vec<CueLeft>,
    pub memories: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CueLeft {
    pub name: String,
    pub hook: String,
    pub code: String,
    pub instructions: String,
    pub effort: String,
}

const KEYS: [&str; 5] = ["title", "difficulty", "state", "workspaces", "timeout"];

/// A module's default export as a script whose value is that export, or
/// what it returns when it is a function: `export default` becomes a
/// binding the script ends by reading.
pub fn module_value(source: &str) -> Result<String, String> {
    const EXPORT: &str = "export default ";
    let count = source.matches(EXPORT).count();
    if count != 1 {
        return Err(format!(
            "has {count} `export default`s; it needs exactly one"
        ));
    }
    Ok(format!(
        "{}\n;typeof __default === 'function' ? __default() : __default;\n",
        source.replacen(EXPORT, "const __default = ", 1)
    ))
}

/// The front matter of a `prompt.md` and the message after it.
fn front_matter(text: &str) -> Result<(BTreeMap<String, String>, String), String> {
    let rest = text
        .strip_prefix("---\n")
        .ok_or("prompt.md starts with a `---` line opening its front matter")?;
    let (header, message) = rest
        .split_once("\n---\n")
        .ok_or("prompt.md's front matter has no closing `---` line")?;
    let mut keys = BTreeMap::new();
    for line in header.lines().filter(|line| !line.trim().is_empty()) {
        let (key, value) = line
            .split_once(':')
            .ok_or_else(|| format!("front matter line `{line}` isn't `key: value`"))?;
        let key = key.trim();
        if !KEYS.contains(&key) {
            return Err(format!("unknown front matter key `{key}`"));
        }
        keys.insert(key.to_string(), value.trim().to_string());
    }
    Ok((keys, message.trim().to_string()))
}

impl Case {
    /// Reads the case in `directory`, its state one of `states`, by name.
    pub fn load(
        directory: &Path,
        category: String,
        prelude: &str,
        states: &BTreeMap<String, String>,
    ) -> Result<Self, String> {
        let read = |file: &str| -> Result<Option<String>, String> {
            let path = directory.join(file);
            match std::fs::read_to_string(&path) {
                Ok(text) => Ok(Some(text)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(format!("{file}: {e}")),
            }
        };
        let module = |file: &str| -> Result<Option<String>, String> {
            read(file)?
                .map(|source| module_value(&source).map_err(|e| format!("{file} {e}")))
                .transpose()
        };
        let (mut keys, prompt) = front_matter(&read("prompt.md")?.ok_or("no prompt.md")?)?;
        if prompt.is_empty() {
            return Err("prompt.md has no message".to_string());
        }
        let title = keys.remove("title").ok_or("prompt.md has no title")?;
        let difficulty = keys
            .remove("difficulty")
            .ok_or("prompt.md has no difficulty")
            .and_then(|text| {
                Difficulty::parse(&text).ok_or("difficulty isn't easy, medium or hard")
            })?;
        let state = match keys
            .remove("state")
            .unwrap_or_else(|| DEFAULT_STATE.to_string())
            .as_str()
        {
            "none" => None,
            name => Some(
                states
                    .get(name)
                    .cloned()
                    .ok_or_else(|| format!("no starting state called `{name}` in states/"))?,
            ),
        };
        let timeout = match keys.remove("timeout") {
            Some(seconds) => Duration::from_secs(
                seconds
                    .parse()
                    .map_err(|_| format!("timeout `{seconds}` isn't a number of seconds"))?,
            ),
            None => DEFAULT_TIMEOUT,
        };
        Ok(Self {
            name: file_name(directory),
            title,
            category,
            difficulty,
            state,
            workspaces: keys
                .remove("workspaces")
                .map(|list| list.split(',').map(|w| w.trim().to_string()).collect()),
            prompt,
            setup: module("setup.ts")?,
            validate: module("validate.ts")?.ok_or("no validate.ts")?,
            solution: module("solution.ts")?,
            timeout,
            prelude: prelude.to_string(),
        })
    }

    /// The prompt with each `{{day:N}}` replaced by the date N days from
    /// `today`.
    pub fn prompt_on(&self, today: jiff::civil::Date) -> Result<String, String> {
        let mut prompt = String::new();
        let mut rest = self.prompt.as_str();
        while let Some(start) = rest.find("{{day:") {
            prompt.push_str(&rest[..start]);
            let after = &rest[start + "{{day:".len()..];
            let end = after.find("}}").ok_or("an unclosed {{day:")?;
            let days: i64 = after[..end]
                .trim()
                .trim_start_matches('+')
                .parse()
                .map_err(|_| format!("`{}` isn't a number of days", &after[..end]))?;
            let date = today
                .checked_add(jiff::Span::new().days(days))
                .map_err(|e| e.to_string())?;
            prompt.push_str(&date.to_string());
            rest = &after[end + 2..];
        }
        prompt.push_str(rest);
        Ok(prompt)
    }

    /// The setup that fills the workspaces: the state applied, the case's
    /// own setup, then the snapshot `B.unchanged()` compares with.
    pub fn setup_script(&self, state: &StartingState) -> String {
        self.script(&format!(
            "B.load({});\n{}\nB.remember();\n",
            serde_json::to_string(state).expect("plain data serializes"),
            self.setup.as_deref().map(scoped).unwrap_or_default()
        ))
    }

    /// The check, with `bench` and `reply` declared ahead of it.
    pub fn validate_script(&self, left: &Left) -> String {
        let bench = serde_json::to_string(left).expect("plain data serializes");
        self.script(&format!(
            "const bench = {bench};\nconst reply = bench.reply;\n{}",
            self.validate
        ))
    }

    /// `script` after the prelude.
    pub fn script(&self, script: &str) -> String {
        format!("{}\n{script}", self.prelude)
    }
}

/// `script` in a block of its own, so its bindings don't meet the ones
/// around it.
fn scoped(script: &str) -> String {
    format!("{{\n{script}\n}}")
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Every case under `directory`, by category and name; only those named
/// in `only` when it isn't empty.
pub fn load_cases(directory: &Path, only: &[String]) -> Result<Vec<Case>, String> {
    let read =
        |path: &Path| std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()));
    let prelude = read(&directory.join("prelude.ts"))?;
    let prompt_prelude = read(&directory.join(PROMPT_PRELUDE))?;
    let prompt_prelude = prompt_prelude.trim();
    let mut states = BTreeMap::new();
    for file in sorted_entries(&directory.join("states"))? {
        if file.extension().is_some_and(|extension| extension == "ts") {
            let source = read(&file)?;
            let script = module_value(&source).map_err(|e| format!("{}: {e}", file.display()))?;
            let name = file
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            states.insert(name, script);
        }
    }
    let mut names = HashSet::new();
    let mut cases = Vec::new();
    for category in sorted_entries(directory)? {
        let category_name = file_name(&category);
        if !category.is_dir() || NOT_CATEGORIES.contains(&category_name.as_str()) {
            continue;
        }
        for case_directory in sorted_entries(&category)? {
            if !case_directory.join("prompt.md").exists() {
                continue;
            }
            let name = file_name(&case_directory);
            if !names.insert(name.clone()) {
                return Err(format!(
                    "{}: another case is called {name}",
                    case_directory.display()
                ));
            }
            let mut case = Case::load(&case_directory, category_name.clone(), &prelude, &states)
                .map_err(|e| format!("{}: {e}", case_directory.display()))?;
            if !prompt_prelude.is_empty() {
                case.prompt = format!("{prompt_prelude}\n\n{}", case.prompt);
            }
            cases.push(case);
        }
    }
    for name in only {
        if !names.contains(name) {
            return Err(format!("{name}: no such case in {}", directory.display()));
        }
    }
    Ok(cases
        .into_iter()
        .filter(|case| only.is_empty() || only.contains(&case.name))
        .collect())
}

fn sorted_entries(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(directory)
        .map_err(|e| format!("{}: {e}", directory.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    entries.sort();
    Ok(entries)
}

/// The cases that ship with the bench.
pub fn default_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("cases")
}

#[cfg(test)]
mod tests;
