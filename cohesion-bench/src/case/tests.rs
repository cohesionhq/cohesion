use std::path::PathBuf;

use super::*;
use crate::starting_state::StartingState;

/// A case directory holding `files`, removed when dropped.
struct CaseDirectory(PathBuf);

impl CaseDirectory {
    fn new(name: &str, files: &[(&str, &str)]) -> Self {
        let path = std::env::temp_dir()
            .join(format!("cohesion-bench-case-{}", std::process::id()))
            .join(name);
        _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        for (file, text) in files {
            std::fs::write(path.join(file), text).unwrap();
        }
        Self(path)
    }

    fn load(&self) -> Result<Case, String> {
        let states = BTreeMap::from([("default".to_string(), "({ workspaces: [] })".to_string())]);
        Case::load(&self.0, "tasks".to_string(), "", &states)
    }
}

impl Drop for CaseDirectory {
    fn drop(&mut self) {
        _ = std::fs::remove_dir_all(&self.0);
    }
}

const VALIDATE: (&str, &str) = ("validate.ts", "export default function validate() {}\n");

#[test]
fn a_case_reads_its_prompt_and_scripts() {
    let directory = CaseDirectory::new(
        "move-it",
        &[
            (
                "prompt.md",
                "---\ntitle: Move it\ndifficulty: hard\nstate: none\nworkspaces: Personal, Acme\n\
                 timeout: 60\n---\n\nMove it to {{day:+2}}, not {{day:-1}}.\n",
            ),
            ("setup.ts", "export default function setup() { B.ws(); }\n"),
            VALIDATE,
        ],
    );
    let case = directory.load().unwrap();
    assert_eq!(case.name, "move-it");
    assert_eq!(case.difficulty, Difficulty::Hard);
    assert_eq!(case.state, None);
    assert_eq!(
        case.workspaces,
        Some(vec!["Personal".to_string(), "Acme".to_string()])
    );
    assert_eq!(case.timeout, Duration::from_secs(60));
    assert_eq!(case.solution, None);
    let setup = case.setup_script(&StartingState::empty());
    assert!(setup.contains("B.load({") && setup.contains("B.ws()"));
    assert!(setup.ends_with("B.remember();\n"));
    assert_eq!(
        case.prompt_on(jiff::civil::date(2026, 12, 31)).unwrap(),
        "Move it to 2027-01-02, not 2026-12-30."
    );
}

#[test]
fn a_case_starts_from_the_default_state_unless_it_names_another() {
    let with = |front: &str| {
        CaseDirectory::new(
            "state",
            &[
                (
                    "prompt.md",
                    &format!("---\ntitle: T\ndifficulty: easy\n{front}---\nHi\n"),
                ),
                VALIDATE,
            ],
        )
        .load()
    };
    assert_eq!(
        with("").unwrap().state.as_deref(),
        Some("({ workspaces: [] })")
    );
    assert_eq!(with("state: none\n").unwrap().state, None);
    assert!(with("state: moon\n").unwrap_err().contains("moon"));
}

#[test]
fn a_malformed_case_says_what_is_wrong() {
    let load = |prompt: &str, validate: Option<&str>| {
        let mut files = vec![("prompt.md", prompt)];
        if let Some(validate) = validate {
            files.push(("validate.ts", validate));
        }
        CaseDirectory::new("bad", &files).load().unwrap_err()
    };
    let good = "export default function validate() {}\n";
    for (prompt, validate, error) in [
        ("title: T\n", Some(good), "front matter"),
        ("---\ntitle: T\n---\nHi\n", Some(good), "no difficulty"),
        (
            "---\ntitle: T\ndifficulty: tricky\n---\nHi\n",
            Some(good),
            "difficulty",
        ),
        (
            "---\ntitle: T\ndifficulty: easy\ncolour: red\n---\nHi\n",
            Some(good),
            "unknown",
        ),
        (
            "---\ntitle: T\ndifficulty: easy\n---\n\n",
            Some(good),
            "no message",
        ),
        (
            "---\ntitle: T\ndifficulty: easy\n---\nHi\n",
            None,
            "no validate.ts",
        ),
        (
            "---\ntitle: T\ndifficulty: easy\n---\nHi\n",
            Some("function validate() {}\n"),
            "export default",
        ),
    ] {
        let found = load(prompt, validate);
        assert!(found.contains(error), "{prompt:?}: {found}");
    }
}

// A module's default export becomes the script's value, called when it is
// a function, so a case is written as an ordinary module.
#[test]
fn a_module_becomes_a_script_ending_in_its_default_export() {
    let script =
        module_value("const x = 1;\nexport default function validate() { return x; }\n").unwrap();
    assert!(script.contains("const __default = function validate()"));
    assert!(
        script
            .trim_end()
            .ends_with("typeof __default === 'function' ? __default() : __default;")
    );
    assert!(module_value("export default 1;\nexport default 2;\n").is_err());
}

// Every shipped case's prompt starts by telling the agent not to ask, so
// a run scores the work rather than a question no one answers.
#[test]
fn every_prompt_starts_with_the_prompt_prelude() {
    let line = std::fs::read_to_string(default_directory().join(PROMPT_PRELUDE)).unwrap();
    let cases = load_cases(&default_directory(), &[]).unwrap();
    assert!(!line.trim().is_empty());
    for case in cases {
        assert!(case.prompt.starts_with(line.trim()), "{}", case.name);
    }
}
