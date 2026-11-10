//! `bench.json`: the providers, their API keys, the models and the options
//! `cohesion-bench setup` saved, which `run` starts from, so a bench needs
//! no flags once set up. It sits beside the crate's `Cargo.toml`; git
//! ignores it, and it is written readable by its owner only, since it holds
//! keys.

use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model_spec::{ModelUnderTest, Provider, dialect_named};

/// What `run` is told to do, from `bench.json` and then its flags.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct Config {
    /// The providers models name, with their keys.
    pub providers: Vec<Provider>,
    /// `provider/model`, as `--model` takes it.
    pub models: Vec<String>,
    /// The cases directory; the bench's own `cases/` when unset.
    pub tests: Option<PathBuf>,
    /// The cases to run, by name; every case when empty.
    pub only: Vec<String>,
    pub repeat: u32,
    pub jobs: u32,
    /// Whether to price runs from the models.dev catalog.
    pub prices: bool,
    /// Where results go; `target/cohesion-bench/<time>` when unset.
    pub out: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            providers: Vec::new(),
            models: Vec::new(),
            tests: None,
            only: Vec::new(),
            repeat: 1,
            jobs: 4,
            prices: true,
            out: None,
        }
    }
}

/// Where `bench.json` lives.
pub fn path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("bench.json")
}

impl Config {
    /// The saved options, or the defaults when there are none.
    pub fn load(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// Writes the options to `path`, readable by its owner only.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).expect("plain data serializes") + "\n";
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            options.mode(0o600);
            // An existing file keeps its mode through `open`; set it too.
            if path.exists() {
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                    .map_err(fail)?;
            }
        }
        options
            .open(path)
            .and_then(|mut file| file.write_all(text.as_bytes()))
            .map_err(fail)
    }

    /// Applies command-line flags over these options. A `--model` or
    /// `--provider` replaces the saved list rather than adding to it.
    pub fn apply(&mut self, args: &[String]) -> Result<(), String> {
        let mut models = Vec::new();
        let mut providers = Vec::new();
        let mut args = args.iter();
        while let Some(flag) = args.next() {
            let mut value = || {
                args.next()
                    .cloned()
                    .ok_or_else(|| format!("{flag} needs a value"))
            };
            let number = |text: String| {
                text.parse::<u32>()
                    .ok()
                    .filter(|n| *n > 0)
                    .ok_or_else(|| format!("{flag}: {text} isn't a positive number"))
            };
            match flag.as_str() {
                "--model" => models.push(value()?),
                "--provider" => providers.push(Provider::described(&value()?)?),
                "--tests" => self.tests = Some(PathBuf::from(value()?)),
                "--only" => self.only = list(&value()?),
                "--all" => self.only.clear(),
                "--repeat" => self.repeat = number(value()?)?,
                "--jobs" => self.jobs = number(value()?)?,
                "--out" => self.out = Some(PathBuf::from(value()?)),
                "--no-prices" => self.prices = false,
                "--prices" => self.prices = true,
                other => return Err(format!("{other}: unknown option")),
            }
        }
        if !models.is_empty() {
            self.models = models;
        }
        if !providers.is_empty() {
            self.providers = providers;
        }
        Ok(())
    }

    /// The models, read, their keys taken from the file or the environment.
    pub fn models_under_test(&self) -> Result<Vec<ModelUnderTest>, String> {
        self.models
            .iter()
            .map(|text| ModelUnderTest::parse(text, &self.providers))
            .collect()
    }
}

fn list(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(String::from)
        .collect()
}

/// Asks for each option on the terminal, offering what `config` holds, and
/// returns the answers. Off a terminal it asks nothing and returns `config`
/// as it is, so an agent sets the options with flags alone.
pub fn ask(mut config: Config) -> Result<Config, String> {
    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        return Ok(config);
    }
    let mut lines = stdin.lock().lines();
    let mut ask = |question: &str, current: &str| -> Result<String, String> {
        match current.is_empty() {
            true => print!("{question}: "),
            false => print!("{question} [{current}]: "),
        }
        std::io::stdout().flush().map_err(|e| e.to_string())?;
        let answer = match lines.next() {
            Some(line) => line.map_err(|e| e.to_string())?,
            None => String::new(),
        };
        Ok(match answer.trim() {
            "" => current.to_string(),
            "-" => String::new(),
            answer => answer.to_string(),
        })
    };
    println!(
        "Press return to keep the value in brackets, or type - to clear it.\n\
         The answers, API keys included, go in {}, which git ignores.\n",
        path().display()
    );

    let mut providers = Vec::new();
    for provider in &config.providers {
        let keep = ask(&format!("Keep provider {}? (yes/no)", provider.name), "yes")?;
        if yes(&keep)? {
            providers.push(provider.clone());
        }
    }
    loop {
        let name = ask(
            "Add a provider: its name (empty when done), such as anthropic or local",
            "",
        )?;
        if name.is_empty() {
            break;
        }
        let mut provider = Provider::preset(&name);
        let preset = ask(
            &format!(
                "  A built-in provider ({}), or empty to describe one",
                cohesionkit::model::IntelligenceProvider::CASES.join(", ")
            ),
            if cohesionkit::model::IntelligenceProvider::CASES.contains(&name.as_str()) {
                &name
            } else {
                ""
            },
        )?;
        if preset.is_empty() {
            provider.dialect = Some(dialect_named(&ask(
                "  Its dialect (chatCompletions, messages, responses, statelessResponses)",
                "chatCompletions",
            )?)?);
            provider.base_url = Some(ask("  Its base URL", "")?);
            let catalog = ask("  Its models.dev provider, for prices (empty for none)", "")?;
            provider.catalog = (!catalog.is_empty()).then_some(catalog);
        } else if preset != name {
            provider.preset = Some(preset);
        }
        let key = ask("  Its API key (empty to read it from the environment)", "")?;
        if key.is_empty() {
            let variable = ask(
                "  The environment variable holding it (empty for <PRESET>_API_KEY, or none)",
                "",
            )?;
            provider.api_key_env = (!variable.is_empty()).then_some(variable);
        } else {
            provider.api_key = Some(key);
        }
        providers.push(provider);
    }
    config.providers = providers;
    config.models = list(&ask(
        "Models, as provider/model, comma-separated",
        &config.models.join(", "),
    )?);
    let tests = ask(
        "Cases directory (empty for the bench's own)",
        &config
            .tests
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
    )?;
    config.tests = (!tests.is_empty()).then(|| PathBuf::from(tests));
    config.only = list(&ask(
        "Cases to run, comma-separated (empty for all)",
        &config.only.join(", "),
    )?);
    config.repeat = positive(&ask("Runs of each case", &config.repeat.to_string())?)?;
    config.jobs = positive(&ask("Cases at once", &config.jobs.to_string())?)?;
    config.prices = yes(&ask(
        "Read prices from models.dev (yes/no)",
        if config.prices { "yes" } else { "no" },
    )?)?;
    Ok(config)
}

fn yes(answer: &str) -> Result<bool, String> {
    match answer.to_lowercase().as_str() {
        "y" | "yes" => Ok(true),
        "n" | "no" => Ok(false),
        other => Err(format!("{other}: answer yes or no")),
    }
}

fn positive(answer: &str) -> Result<u32, String> {
    answer
        .parse()
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(|| format!("{answer}: not a positive number"))
}

#[cfg(test)]
mod tests;
