//! `cohesion-bench`: runs the bench's cases on one or more models and
//! writes `report.html`, `cases.csv` and `summary.csv`.
//!
//! ```sh
//! export ANTHROPIC_API_KEY=… OPENAI_API_KEY=…
//! cargo run --release -p cohesion-bench -- run \
//!     --model anthropic/claude-sonnet-5-5 --model openai/gpt-6.1-sol --repeat 3
//! cargo run -p cohesion-bench -- validate   # the cases alone, no model
//! ```

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use cohesion_bench::case::{Case, default_directory, load_cases};
use cohesion_bench::metrics::{CaseResult, Outcome, summaries};
use cohesion_bench::model_spec::{Endpoint, ModelUnderTest};
use cohesion_bench::report::{self, RunInfo};
use cohesion_bench::run::run_case;
use cohesion_bench::validate::validate;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

const USAGE: &str = "\
usage:
  cohesion-bench run --model ENDPOINT/MODEL [--model …] [options]
  cohesion-bench validate [--cases DIR] [--only CASE,…]
  cohesion-bench list [--cases DIR]

ENDPOINT is a provider (openai, anthropic, xai, mistral, fireworks, deepinfra,
togetherai, openrouter), its key read from <PROVIDER>_API_KEY, or one described
with --endpoint.

options:
  --endpoint NAME=DIALECT,BASE_URL[,PROVIDER[,KEY_VARIABLE]]
                    describe an endpoint, such as
                    local=chatCompletions,http://localhost:11434/v1
  --cases DIR       the cases to run (default: the bench's own)
  --only CASE,…     run only these cases
  --repeat N        run each case N times (default 1)
  --jobs N          run N cases at once (default 4)
  --out DIR         where to write the results (default target/cohesion-bench/<time>)
  --no-prices       don't read prices from the models.dev catalog";

struct Options {
    models: Vec<String>,
    endpoints: Vec<Endpoint>,
    cases: PathBuf,
    only: Vec<String>,
    repeat: u32,
    jobs: usize,
    out: Option<PathBuf>,
    prices: bool,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        models: Vec::new(),
        endpoints: Vec::new(),
        cases: default_directory(),
        only: Vec::new(),
        repeat: 1,
        jobs: 4,
        out: None,
        prices: true,
    };
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
            "--model" => options.models.push(value()?),
            "--endpoint" => options.endpoints.push(Endpoint::described(&value()?)?),
            "--cases" => options.cases = PathBuf::from(value()?),
            "--only" => options
                .only
                .extend(value()?.split(',').map(str::trim).map(String::from)),
            "--repeat" => options.repeat = number(value()?)?,
            "--jobs" => options.jobs = number(value()?)? as usize,
            "--out" => options.out = Some(PathBuf::from(value()?)),
            "--no-prices" => options.prices = false,
            other => return Err(format!("{other}: unknown option")),
        }
    }
    Ok(options)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, rest)) = args.split_first() else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    let options = match parse(rest) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    let cases = match load_cases(&options.cases, &options.only) {
        Ok(cases) => cases,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("a tokio runtime");
    let outcome = match command.as_str() {
        "list" => {
            for case in &cases {
                println!("{:<24} {:<12} {}", case.name, case.category, case.title);
            }
            Ok(())
        }
        "validate" => runtime.block_on(validate_all(&cases)),
        "run" => runtime.block_on(run(options, cases)),
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

async fn validate_all(cases: &[Case]) -> Result<(), String> {
    let mut broken = 0;
    for case in cases {
        match validate(case).await {
            Ok(()) => println!("ok      {}", case.name),
            Err(error) => {
                broken += 1;
                println!("broken  {}: {error}", case.name);
            }
        }
    }
    match broken {
        0 => Ok(()),
        _ => Err(format!("{broken} of {} cases are broken", cases.len())),
    }
}

async fn run(options: Options, cases: Vec<Case>) -> Result<(), String> {
    if options.models.is_empty() {
        return Err(format!("name at least one --model\n\n{USAGE}"));
    }
    let mut models = options
        .models
        .iter()
        .map(|text| ModelUnderTest::parse(text, &options.endpoints))
        .collect::<Result<Vec<_>, _>>()?;
    let labels: Vec<String> = models.iter().map(|model| model.label.clone()).collect();
    if options.prices {
        cohesion_bench::prices::price(&mut models).await?;
    }
    let now = jiff::Zoned::now();
    let out = options.out.unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../target/cohesion-bench")
            .join(now.strftime("%Y-%m-%d_%H-%M-%S").to_string())
    });

    let mut work = Vec::new();
    for model in &models {
        for case in &cases {
            for repeat in 1..=options.repeat {
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
        options.repeat,
        models.len(),
        if models.len() == 1 { "" } else { "s" },
        options.jobs,
        out.display()
    );

    let slots = Arc::new(Semaphore::new(options.jobs));
    let mut running = JoinSet::new();
    for (index, (model, case, repeat, directory)) in work.into_iter().enumerate() {
        let slots = slots.clone();
        running.spawn(async move {
            let _slot = slots
                .acquire_owned()
                .await
                .expect("the semaphore stays open");
            let result = run_case(&case, &model, repeat, &directory.join("cohesion.db")).await;
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
        repeats: options.repeat,
    };
    report::write(&out, &info, &results, &summaries)
        .map_err(|e| format!("{}: {e}", out.display()))?;
    for summary in &summaries {
        eprintln!(
            "{}: {}/{} passed, tool accuracy {}, {}",
            summary.model,
            summary.passed,
            summary.runs,
            summary
                .tool_accuracy()
                .map_or("–".to_string(), |a| format!("{:.0}%", a * 100.0)),
            match summary.priced {
                true => format!("${:.4}", summary.cost),
                false => "unpriced".to_string(),
            }
        );
    }
    println!("{}", out.join("report.html").display());
    Ok(())
}
