//! `cohesion-bench`: runs the bench's cases on one or more models and
//! writes `report.html`, `cases.csv` and `summary.csv`.
//!
//! ```sh
//! cargo run -p cohesion-bench -- setup        # asks for providers, keys and
//!                                             # models, and saves bench.json
//! cargo run --release -p cohesion-bench -- run
//! cargo run -p cohesion-bench -- validate     # the cases alone, no model
//! ```

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use cohesion_bench::case::{Case, default_directory, load_cases};
use cohesion_bench::config::{self, Config};
use cohesion_bench::metrics::{CaseResult, Outcome, summaries};
use cohesion_bench::model_spec::ModelUnderTest;
use cohesion_bench::report::{self, RunInfo};
use cohesion_bench::run::run_case;
use cohesion_bench::types;
use cohesion_bench::validate::validate_all;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

const USAGE: &str = "\
usage:
  cohesion-bench setup [options]   save providers, keys, models and options to
                                   bench.json, asking for each on a terminal
  cohesion-bench run [options]     run the cases, starting from bench.json
  cohesion-bench validate [--tests DIR] [--only CASE,…]
                                   check every case can fail and pass, no model
  cohesion-bench list [--tests DIR]
  cohesion-bench types [--tests DIR]
                                   write types/cohesion.d.ts, the script API's
                                   declarations, for an editor (every command
                                   does this too)

options (each overrides bench.json):
  --model PROVIDER/MODEL
                    a model to run, repeated for more. PROVIDER is one in
                    bench.json, or a preset (openai, anthropic, xai, mistral,
                    fireworks, deepinfra, togetherai, openrouter) with its key
                    in <PRESET>_API_KEY
  --provider NAME=DIALECT,BASE_URL[,CATALOG[,KEY_VARIABLE]]
                    describe a provider, such as
                    local=chatCompletions,http://localhost:11434/v1
  --tests DIR       the cases directory (default: the bench's own cases/)
  --only CASE,…     run only these cases; --all runs every case
  --repeat N        run each case N times (default 1)
  --jobs N          run N cases at once (default 4)
  --out DIR         where to write the results (default target/cohesion-bench/<time>)
  --no-prices       don't read prices from the models.dev catalog; --prices does";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, flags)) = args.split_first() else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("a tokio runtime");
    let outcome = match command.as_str() {
        "setup" => setup(flags),
        "types" => options(flags).and_then(|config| {
            let path = types::write(&tests_directory(&config))?;
            println!("Wrote {}", path.display());
            Ok(())
        }),
        "run" | "validate" | "list" => options(flags).and_then(|config| {
            // Kept in step with the API this build runs, for editors.
            types::write(&tests_directory(&config))?;
            let cases = load_cases(&tests_directory(&config), &config.only)?;
            match command.as_str() {
                "list" => {
                    for case in &cases {
                        println!(
                            "{:<28} {:<12} {:<7} {}",
                            case.name,
                            case.category,
                            case.difficulty.as_str(),
                            case.title
                        );
                    }
                    println!("{} cases", cases.len());
                    Ok(())
                }
                "validate" => runtime.block_on(validate_cases(&cases, config.jobs.max(8) as usize)),
                _ => runtime.block_on(run(config, cases)),
            }
        }),
        _ => Err(USAGE.to_string()),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn tests_directory(config: &Config) -> PathBuf {
    config.tests.clone().unwrap_or_else(default_directory)
}

/// `bench.json` with `flags` applied.
fn options(flags: &[String]) -> Result<Config, String> {
    let mut config = Config::load(&config::path())?;
    config
        .apply(flags)
        .map_err(|error| format!("{error}\n\n{USAGE}"))?;
    Ok(config)
}

fn setup(flags: &[String]) -> Result<(), String> {
    let config = config::ask(options(flags)?)?;
    // Read them now, so a typo shows here rather than at the next run.
    for model in &config.models {
        if let Err(error) = ModelUnderTest::parse(model, &config.providers) {
            eprintln!("Warning: {error}");
        }
    }
    let path = config::path();
    config.save(&path)?;
    println!("Saved {}", path.display());
    Ok(())
}

async fn validate_cases(cases: &[Case], jobs: usize) -> Result<(), String> {
    let outcomes = validate_all(cases, jobs).await;
    let mut broken = 0;
    for (case, outcome) in cases.iter().zip(outcomes) {
        match outcome {
            Ok(()) => println!("ok      {}", case.name),
            Err(error) => {
                broken += 1;
                println!("broken  {}: {error}", case.name);
            }
        }
    }
    match broken {
        0 => {
            println!("All {} cases can fail and pass.", cases.len());
            Ok(())
        }
        _ => Err(format!("{broken} of {} cases are broken", cases.len())),
    }
}

async fn run(config: Config, cases: Vec<Case>) -> Result<(), String> {
    if config.models.is_empty() {
        return Err(format!(
            "No models to run: run `cohesion-bench setup`, or name one with --model\n\n{USAGE}"
        ));
    }
    let mut models = config.models_under_test()?;
    let labels: Vec<String> = models.iter().map(|model| model.label.clone()).collect();
    if config.prices {
        cohesion_bench::prices::price(&mut models).await?;
    }
    let now = jiff::Zoned::now();
    let out = config.out.clone().unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../target/cohesion-bench")
            .join(now.strftime("%Y-%m-%d_%H-%M-%S").to_string())
    });

    let mut work = Vec::new();
    for model in &models {
        for case in &cases {
            for repeat in 1..=config.repeat {
                let directory = out
                    .join("runs")
                    .join(model.slug())
                    .join(format!("{}-{repeat}", case.name));
                std::fs::create_dir_all(&directory)
                    .map_err(|e| format!("{}: {e}", directory.display()))?;
                work.push((model.clone(), case.clone(), repeat, directory));
            }
        }
    }
    let total = work.len();
    eprintln!(
        "Running {} case{} × {} on {} model{}, {} at once, into {}",
        cases.len(),
        if cases.len() == 1 { "" } else { "s" },
        config.repeat,
        models.len(),
        if models.len() == 1 { "" } else { "s" },
        config.jobs,
        out.display()
    );

    let today = now.date();
    let slots = Arc::new(Semaphore::new(config.jobs as usize));
    let mut running = JoinSet::new();
    for (index, (model, case, repeat, directory)) in work.into_iter().enumerate() {
        let slots = slots.clone();
        running.spawn(async move {
            let _slot = slots
                .acquire_owned()
                .await
                .expect("the semaphore stays open");
            let database = directory.join("cohesion.db");
            let result = run_case(&case, &model, repeat, &database, today).await;
            (index, result)
        });
    }
    let mut results: Vec<(usize, CaseResult)> = Vec::with_capacity(total);
    while let Some(joined) = running.join_next().await {
        let (index, result) = joined.map_err(|e| format!("a case's task failed: {e}"))?;
        eprintln!(
            "[{}/{total}] {:<7} {} · {} #{} · {:.1}s · {} model calls · {}{}",
            results.len() + 1,
            result.outcome.as_str(),
            result.model,
            result.case,
            result.repeat,
            result.seconds,
            result.activity.model_calls,
            match result.priced {
                true => format!("${:.4}", result.activity.cost),
                false => "unpriced".to_string(),
            },
            match result.outcome {
                Outcome::Passed => String::new(),
                _ => format!(" · {}", result.reason),
            }
        );
        results.push((index, result));
    }
    results.sort_by_key(|(index, _)| *index);
    let results: Vec<CaseResult> = results.into_iter().map(|(_, result)| result).collect();

    let summaries = summaries(&labels, &results);
    let info = RunInfo {
        started: now.strftime("%Y-%m-%d %H:%M %Z").to_string(),
        models: &labels,
        cases: &cases,
        repeats: config.repeat,
    };
    report::write(&out, &info, &results, &summaries)
        .map_err(|e| format!("{}: {e}", out.display()))?;
    let score = |value: Option<f64>| value.map_or("–".to_string(), |v| format!("{v:.1}"));
    for summary in &summaries {
        eprintln!(
            "{}: overall {}, accuracy {}, efficiency {}, {}/{} passed, {}",
            summary.model,
            score(summary.overall),
            score(summary.accuracy),
            score(summary.efficiency),
            summary.passed,
            summary.runs,
            match summary.priced {
                true => format!("${:.4}", summary.cost),
                false => "unpriced".to_string(),
            }
        );
    }
    println!("{}", out.join("report.html").display());
    Ok(())
}
