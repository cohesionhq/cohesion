//! What a run writes: `cases.csv` (a row per run of a case), `summary.csv`
//! (a row per model) and `report.html`, which shows both at a glance:
//! the models ranked by overall score, how they compare, how each did by
//! category and case, and why each run that missed missed.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use crate::case::{Case, Difficulty};
use crate::metrics::{CODE_TOOL, CaseResult, DOCUMENTATION_TOOL, Outcome, Summary, accuracy};
use crate::score;

/// What the report says about the run as a whole.
pub struct RunInfo<'a> {
    pub started: String,
    pub models: &'a [String],
    pub cases: &'a [Case],
    pub repeats: u32,
}

/// Writes the three files into `directory`.
pub fn write(
    directory: &Path,
    info: &RunInfo<'_>,
    results: &[CaseResult],
    summaries: &[Summary],
) -> std::io::Result<()> {
    std::fs::write(directory.join("cases.csv"), cases_csv(results))?;
    std::fs::write(directory.join("summary.csv"), summary_csv(summaries))?;
    std::fs::write(
        directory.join("report.html"),
        html(info, results, summaries),
    )?;
    Ok(())
}

pub fn cases_csv(results: &[CaseResult]) -> String {
    let mut csv = Csv::new(&[
        "model",
        "case",
        "category",
        "difficulty",
        "repeat",
        "outcome",
        "efficiency",
        "reason",
        "seconds",
        "model_calls",
        "tool_calls",
        "failed_tool_calls",
        "tool_accuracy",
        "code_calls",
        "documentation_calls",
        "tools",
        "messages_sent",
        "questions_asked",
        "input_tokens",
        "cache_read_tokens",
        "cache_write_tokens",
        "output_tokens",
        "total_tokens",
        "cost_usd",
        "priced",
    ]);
    for r in results {
        let a = &r.activity;
        let tools = a
            .tools
            .iter()
            .map(|(name, used)| format!("{name}:{}/{}", used.calls - used.failed, used.calls))
            .collect::<Vec<_>>()
            .join(" ");
        csv.row(&[
            r.model.clone(),
            r.case.clone(),
            r.category.clone(),
            r.difficulty.as_str().to_string(),
            r.repeat.to_string(),
            r.outcome.as_str().to_string(),
            optional(r.efficiency(), 1),
            r.reason.clone(),
            format!("{:.2}", r.seconds),
            a.model_calls.to_string(),
            a.tool_calls().to_string(),
            a.failed_tool_calls().to_string(),
            optional(a.tool_accuracy(), 4),
            a.calls_to(CODE_TOOL).to_string(),
            a.calls_to(DOCUMENTATION_TOOL).to_string(),
            tools,
            a.messages_sent.to_string(),
            a.asked.len().to_string(),
            a.input_tokens.to_string(),
            a.cache_read_tokens.to_string(),
            a.cache_write_tokens.to_string(),
            a.output_tokens.to_string(),
            a.total_tokens().to_string(),
            format!("{:.6}", a.cost),
            r.priced.to_string(),
        ]);
    }
    csv.0
}

pub fn summary_csv(summaries: &[Summary]) -> String {
    let mut csv = Csv::new(&[
        "model",
        "overall",
        "accuracy",
        "efficiency",
        "alignment",
        "accuracy_easy",
        "accuracy_medium",
        "accuracy_hard",
        "runs",
        "passed",
        "failed",
        "errors",
        "success_rate",
        "tool_calls",
        "failed_tool_calls",
        "tool_accuracy",
        "code_calls",
        "documentation_calls",
        "cost_usd",
        "cost_per_run_usd",
        "cost_per_pass_usd",
        "mean_seconds",
        "median_seconds",
        "p90_seconds",
        "output_tokens_per_second",
        "mean_model_calls",
        "max_model_calls",
        "mean_tool_calls",
        "mean_total_tokens",
        "mean_output_tokens",
        "tokens_per_pass",
        "cache_hit_rate",
        "runs_asking_user",
        "priced",
    ]);
    for s in summaries {
        let [easy, medium, hard] = s.accuracy_by_difficulty;
        csv.row(&[
            s.model.clone(),
            optional(s.overall, 1),
            optional(s.accuracy, 1),
            optional(s.efficiency, 1),
            optional(s.alignment, 1),
            optional(easy, 1),
            optional(medium, 1),
            optional(hard, 1),
            s.runs.to_string(),
            s.passed.to_string(),
            s.failed.to_string(),
            s.errors.to_string(),
            optional(s.success_rate(), 4),
            s.tool_calls.to_string(),
            s.failed_tool_calls.to_string(),
            optional(s.tool_accuracy(), 4),
            s.code_calls.to_string(),
            s.documentation_calls.to_string(),
            format!("{:.6}", s.cost),
            optional(s.cost_per_run(), 6),
            optional(s.cost_per_pass(), 6),
            format!("{:.2}", s.mean_seconds),
            format!("{:.2}", s.median_seconds),
            format!("{:.2}", s.p90_seconds),
            format!("{:.1}", s.output_tokens_per_second),
            format!("{:.2}", s.mean_model_calls),
            s.max_model_calls.to_string(),
            format!("{:.2}", s.mean_tool_calls),
            format!("{:.0}", s.mean_total_tokens),
            format!("{:.0}", s.mean_output_tokens),
            optional(s.tokens_per_pass, 0),
            optional(s.cache_hit_rate, 4),
            s.asked_user.to_string(),
            s.priced.to_string(),
        ]);
    }
    csv.0
}

/// RFC 4180 CSV: a field holding a comma, quote or line break is quoted.
struct Csv(String);

impl Csv {
    fn new(header: &[&str]) -> Self {
        Self(header.join(",") + "\n")
    }

    fn row(&mut self, fields: &[String]) {
        let fields: Vec<String> = fields.iter().map(|field| csv_field(field)).collect();
        self.0.push_str(&fields.join(","));
        self.0.push('\n');
    }
}

fn csv_field(field: &str) -> String {
    match field.contains([',', '"', '\n', '\r']) {
        true => format!("\"{}\"", field.replace('"', "\"\"")),
        false => field.to_string(),
    }
}

fn optional(value: Option<f64>, decimals: usize) -> String {
    value.map_or(String::new(), |value| format!("{value:.decimals$}"))
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn percent(value: Option<f64>) -> String {
    value.map_or("–".to_string(), |value| format!("{:.0}%", value * 100.0))
}

/// A score from 0 to 100, to one place.
fn points(value: Option<f64>) -> String {
    value.map_or("–".to_string(), |value| format!("{value:.1}"))
}

fn dollars(value: Option<f64>, priced: bool) -> String {
    match (value, priced) {
        (_, false) => "unpriced".to_string(),
        (None, _) => "–".to_string(),
        (Some(value), _) if value >= 1.0 => format!("${value:.2}"),
        (Some(value), _) if value >= 0.01 => format!("${value:.3}"),
        (Some(value), _) => format!("${value:.4}"),
    }
}

fn seconds(value: f64) -> String {
    match value >= 60.0 {
        true => format!("{}m {:02.0}s", (value / 60.0).floor(), value % 60.0),
        false => format!("{value:.1}s"),
    }
}

fn tokens(value: Option<f64>) -> String {
    match value {
        None => "–".to_string(),
        Some(value) if value >= 1_000_000.0 => format!("{:.2}M", value / 1_000_000.0),
        Some(value) if value >= 1_000.0 => format!("{:.1}k", value / 1_000.0),
        Some(value) => format!("{value:.0}"),
    }
}

/// A cell's tint for a score from 0 to 100: red through amber to green.
fn heat(value: Option<f64>) -> String {
    match value {
        None => String::new(),
        Some(value) => {
            let hue = (value.clamp(0.0, 100.0) * 1.3).round();
            format!(" style=\"--hue:{hue}\"")
        }
    }
}

const STYLE: &str = r#"
:root {
  color-scheme: light dark;
  --bg: #f6f5f2; --card: #ffffff; --ink: #1d1c1a; --muted: #6d6a64; --line: #e4e1da;
  --pass: #1f8a5b; --pass-bg: #e3f3ea; --fail: #c2412d; --fail-bg: #fbe7e3;
  --part: #b7791f; --error: #8a8580; --error-bg: #eeece8; --bar: #e4e0d8;
  --eff: #3b6fd8; --align: #8a5cd0; --heat-l: 92%; --heat-s: 60%;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #151514; --card: #1e1e1c; --ink: #ecebe7; --muted: #9b978f; --line: #2e2d2a;
    --pass: #5cc492; --pass-bg: #1b3328; --fail: #f08070; --fail-bg: #3a201b;
    --part: #e0b44c; --error: #8d8981; --error-bg: #2a2927; --bar: #34332f;
    --eff: #86a8ff; --align: #b99af0; --heat-l: 24%; --heat-s: 45%;
  }
}
* { box-sizing: border-box; }
body { margin: 0; background: var(--bg); color: var(--ink);
  font: 14px/1.5 ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif; }
main { max-width: 1240px; margin: 0 auto; padding: 40px 24px 80px; }
h1 { font-size: 28px; letter-spacing: -0.02em; margin: 0 0 4px; }
h2 { font-size: 13px; text-transform: uppercase; letter-spacing: 0.08em; color: var(--muted);
  margin: 48px 0 14px; font-weight: 600; }
.sub { color: var(--muted); margin: 0; }
.num { font-variant-numeric: tabular-nums; }
.cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 14px; }
.card { background: var(--card); border: 1px solid var(--line); border-radius: 14px; padding: 18px 20px; }
.card .top { display: flex; justify-content: space-between; align-items: baseline; gap: 8px; }
.card .model { font-weight: 600; font-size: 15px; word-break: break-all; }
.card .rank { color: var(--muted); font-size: 13px; }
.card .overall { font-size: 46px; font-weight: 700; letter-spacing: -0.03em; line-height: 1.05; margin: 8px 0 0; }
.card .overall small { font-size: 13px; font-weight: 500; color: var(--muted); letter-spacing: 0; margin-left: 6px; }
.gauge { display: grid; grid-template-columns: 78px 1fr 36px; align-items: center; gap: 8px;
  font-size: 12px; color: var(--muted); margin-top: 8px; }
.gauge b { color: var(--ink); text-align: right; }
.meter { height: 6px; border-radius: 3px; background: var(--bar); overflow: hidden; }
.meter i { display: block; height: 100%; background: var(--pass); }
.meter.eff i { background: var(--eff); }
.meter.align i { background: var(--align); }
.stats { display: grid; grid-template-columns: 1fr 1fr; gap: 8px 16px; margin: 14px 0 0; }
.stats dt { color: var(--muted); font-size: 12px; }
.stats dd { margin: 0; font-weight: 600; }
.scroll { overflow-x: auto; border: 1px solid var(--line); border-radius: 14px; background: var(--card); }
table { width: 100%; border-collapse: collapse; }
th, td { padding: 8px 11px; text-align: left; border-bottom: 1px solid var(--line); vertical-align: top; }
th { font-size: 12px; color: var(--muted); font-weight: 600; white-space: nowrap; }
tr:last-child td { border-bottom: 0; }
td.r, th.r { text-align: right; }
td.r { white-space: nowrap; }
td.heat { background: hsl(var(--hue) var(--heat-s) var(--heat-l)); text-align: right; font-weight: 600; }
.case-title { font-weight: 600; }
.tag { display: inline-block; font-size: 11px; color: var(--muted); border: 1px solid var(--line);
  border-radius: 999px; padding: 0 7px; margin-left: 6px; white-space: nowrap; }
.tag.hard { color: var(--fail); border-color: var(--fail); }
.tag.medium { color: var(--part); border-color: var(--part); }
.runs { display: flex; gap: 3px; }
.dot { width: 12px; height: 12px; border-radius: 4px; display: inline-block; }
.dot.passed { background: var(--pass); } .dot.failed { background: var(--fail); }
.dot.error { background: var(--error); }
.cell-meta { color: var(--muted); font-size: 12px; }
details.group { background: var(--card); border: 1px solid var(--line); border-radius: 14px; margin-bottom: 10px; overflow: hidden; }
details.group > summary { padding: 12px 16px; font-weight: 600; }
details.group table { border-top: 1px solid var(--line); }
details.miss { background: var(--card); border: 1px solid var(--line); border-radius: 12px; margin-bottom: 8px; }
summary { cursor: pointer; list-style: none; }
summary::-webkit-details-marker { display: none; }
details.miss > summary { padding: 10px 16px; display: flex; gap: 10px; align-items: center; }
summary .grow { flex: 1; }
.pill { font-size: 11px; font-weight: 600; border-radius: 999px; padding: 1px 9px; white-space: nowrap; }
.pill.failed { color: var(--fail); background: var(--fail-bg); }
.pill.error { color: var(--error); background: var(--error-bg); }
.detail { padding: 0 16px 14px; }
.detail p { margin: 6px 0; }
.chips { display: flex; flex-wrap: wrap; gap: 6px; margin: 8px 0; }
.chip { font: 12px ui-monospace, SFMono-Regular, Menlo, monospace; border: 1px solid var(--line);
  border-radius: 6px; padding: 1px 7px; }
.chip.bad { border-color: var(--fail); color: var(--fail); }
pre { white-space: pre-wrap; word-break: break-word; background: var(--bg); border-radius: 8px;
  padding: 10px 12px; margin: 8px 0 0; font: 12px/1.5 ui-monospace, SFMono-Regular, Menlo, monospace;
  max-height: 240px; overflow: auto; }
.prompt { color: var(--muted); font-style: italic; margin: 2px 0 8px; }
.empty { color: var(--muted); }
footer { color: var(--muted); font-size: 12px; margin-top: 40px; line-height: 1.6; }
footer p { margin: 0 0 6px; }
"#;

/// The colour class of one run's dot.
fn dot_class(result: &CaseResult) -> &'static str {
    match result.outcome {
        Outcome::Passed => "passed",
        Outcome::Error => "error",
        Outcome::Failed => "failed",
    }
}

pub fn html(info: &RunInfo<'_>, results: &[CaseResult], summaries: &[Summary]) -> String {
    let mut page = String::new();
    let per_model = info.cases.len() as u32 * info.repeats;
    _ = write!(
        page,
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>Cohesion bench · {started}</title><style>{STYLE}</style></head><body><main>\
         <h1>Cohesion bench</h1><p class=\"sub\">{started} · {models} model{ms} · {cases} case{cs} \
         × {repeats} · {per_model} runs each</p>",
        started = escape(&info.started),
        models = info.models.len(),
        ms = plural(info.models.len()),
        cases = info.cases.len(),
        cs = plural(info.cases.len()),
        repeats = info.repeats,
    );

    let mut ranked: Vec<&Summary> = summaries.iter().collect();
    ranked.sort_by(|a, b| {
        b.overall
            .unwrap_or(-1.0)
            .total_cmp(&a.overall.unwrap_or(-1.0))
    });

    page.push_str("<h2>Leaderboard</h2><div class=\"cards\">");
    for (rank, s) in ranked.iter().enumerate() {
        let gauge = |label: &str, value: Option<f64>, class: &str| {
            format!(
                "<div class=\"gauge num\"><span>{label}</span><div class=\"meter {class}\">\
                 <i style=\"width:{:.1}%\"></i></div><b>{}</b></div>",
                value.unwrap_or(0.0),
                points(value)
            )
        };
        _ = write!(
            page,
            "<section class=\"card\"><div class=\"top\"><span class=\"model\">{model}</span>\
             <span class=\"rank\">#{rank}</span></div>\
             <div class=\"overall num\">{overall}<small>overall</small></div>\
             {accuracy}{efficiency}{alignment}<dl class=\"stats num\">\
             <div><dt>Passed</dt><dd>{passed}/{runs}</dd></div>\
             <div><dt>Cost per run</dt><dd>{cost}</dd></div>\
             <div><dt>Median time</dt><dd>{median}</dd></div>\
             <div><dt>Tokens per pass</dt><dd>{per_pass}</dd></div>\
             <div><dt>Tool accuracy</dt><dd>{tool_accuracy}</dd></div>\
             <div><dt>Model calls per run</dt><dd>{calls:.1}</dd></div>\
             </dl></section>",
            model = escape(&s.model),
            rank = rank + 1,
            overall = points(s.overall),
            accuracy = gauge("Accuracy", s.accuracy, ""),
            efficiency = gauge("Efficiency", s.efficiency, "eff"),
            alignment = gauge("Alignment", s.alignment, "align"),
            passed = s.passed,
            runs = s.runs,
            cost = dollars(s.cost_per_run(), s.priced),
            median = seconds(s.median_seconds),
            per_pass = tokens(s.tokens_per_pass),
            tool_accuracy = percent(s.tool_accuracy()),
            calls = s.mean_model_calls,
        );
    }
    page.push_str("</div>");

    page.push_str(
        "<h2>Comparison</h2><div class=\"scroll\"><table class=\"num\"><thead><tr><th>Model</th>\
         <th class=\"r\">Overall</th><th class=\"r\">Accuracy</th><th class=\"r\">Efficiency</th>\
         <th class=\"r\">Alignment</th><th class=\"r\">Easy</th><th class=\"r\">Medium</th><th class=\"r\">Hard</th>\
         <th class=\"r\">Passed</th><th class=\"r\">Tool accuracy</th><th class=\"r\">Total cost</th>\
         <th class=\"r\">Cost / pass</th><th class=\"r\">Mean time</th><th class=\"r\">p90 time</th>\
         <th class=\"r\">Output tok/s</th><th class=\"r\">Tokens / run</th><th class=\"r\">Cache hits</th>\
         <th class=\"r\">Model calls</th><th class=\"r\">Docs read</th><th class=\"r\">Asked</th>\
         <th class=\"r\">Errors</th></tr></thead><tbody>",
    );
    for s in &ranked {
        let [easy, medium, hard] = s.accuracy_by_difficulty;
        _ = write!(
            page,
            "<tr><td>{model}</td><td class=\"r\"><b>{overall}</b></td><td class=\"r\">{accuracy}</td>\
             <td class=\"r\">{efficiency}</td><td class=\"r\">{alignment}</td>\
             <td class=\"heat\"{he}>{easy}</td>\
             <td class=\"heat\"{hm}>{medium}</td><td class=\"heat\"{hh}>{hard}</td>\
             <td class=\"r\">{passed}/{runs}</td><td class=\"r\">{tool_accuracy}</td>\
             <td class=\"r\">{cost}</td><td class=\"r\">{per_pass}</td><td class=\"r\">{mean}</td>\
             <td class=\"r\">{p90}</td><td class=\"r\">{speed:.0}</td><td class=\"r\">{per_run}</td>\
             <td class=\"r\">{cache}</td><td class=\"r\">{calls:.1} (max {max})</td>\
             <td class=\"r\">{docs}</td><td class=\"r\">{asked}</td><td class=\"r\">{errors}</td></tr>",
            model = escape(&s.model),
            overall = points(s.overall),
            accuracy = points(s.accuracy),
            efficiency = points(s.efficiency),
            alignment = points(s.alignment),
            he = heat(easy),
            hm = heat(medium),
            hh = heat(hard),
            easy = points(easy),
            medium = points(medium),
            hard = points(hard),
            passed = s.passed,
            runs = s.runs,
            tool_accuracy = percent(s.tool_accuracy()),
            cost = dollars(Some(s.cost), s.priced),
            per_pass = dollars(s.cost_per_pass(), s.priced),
            mean = seconds(s.mean_seconds),
            p90 = seconds(s.p90_seconds),
            speed = s.output_tokens_per_second,
            per_run = tokens(Some(s.mean_total_tokens)),
            cache = percent(s.cache_hit_rate),
            calls = s.mean_model_calls,
            max = s.max_model_calls,
            docs = s.documentation_calls,
            asked = s.asked_user,
            errors = s.errors,
        );
    }
    page.push_str("</tbody></table></div>");

    let categories: BTreeSet<&str> = info.cases.iter().map(|c| c.category.as_str()).collect();
    let runs_of = |model: &str, filter: &dyn Fn(&CaseResult) -> bool| -> Vec<&CaseResult> {
        results
            .iter()
            .filter(|r| r.model == model && filter(r))
            .collect()
    };
    page.push_str(
        "<h2>Accuracy by category</h2><div class=\"scroll\"><table class=\"num\"><thead><tr>\
         <th>Category</th><th class=\"r\">Cases</th>",
    );
    for s in &ranked {
        _ = write!(page, "<th class=\"r\">{}</th>", escape(&s.model));
    }
    page.push_str("</tr></thead><tbody>");
    for category in &categories {
        let count = info
            .cases
            .iter()
            .filter(|c| c.category == *category)
            .count();
        _ = write!(
            page,
            "<tr><td>{}</td><td class=\"r\">{count}</td>",
            escape(category)
        );
        for s in &ranked {
            let runs = runs_of(&s.model, &|r| r.category == *category);
            let accuracy = accuracy(&runs);
            _ = write!(
                page,
                "<td class=\"heat\"{}>{}</td>",
                heat(accuracy),
                points(accuracy)
            );
        }
        page.push_str("</tr>");
    }
    page.push_str("</tbody></table></div>");

    page.push_str("<h2>Cases</h2>");
    for category in &categories {
        let cases: Vec<&Case> = info
            .cases
            .iter()
            .filter(|c| c.category == *category)
            .collect();
        _ = write!(
            page,
            "<details class=\"group\"><summary>{} <span class=\"tag\">{} cases</span></summary>\
             <div class=\"scroll\" style=\"border:0;border-radius:0\"><table><thead><tr><th>Case</th>",
            escape(category),
            cases.len()
        );
        for s in &ranked {
            _ = write!(page, "<th>{}</th>", escape(&s.model));
        }
        page.push_str("</tr></thead><tbody>");
        for case in cases {
            _ = write!(
                page,
                "<tr><td><span class=\"case-title\">{}</span>\
                 <span class=\"tag {difficulty}\">{difficulty}</span></td>",
                escape(&case.title),
                difficulty = case.difficulty.as_str(),
            );
            for s in &ranked {
                let runs = runs_of(&s.model, &|r| r.case == case.name);
                page.push_str("<td><div class=\"runs\">");
                for run in &runs {
                    _ = write!(
                        page,
                        "<span class=\"dot {}\" title=\"{}\"></span>",
                        dot_class(run),
                        escape(&format!(
                            "#{} {}: {}",
                            run.repeat,
                            run.outcome.as_str(),
                            run.reason
                        ))
                    );
                }
                let count = runs.len().max(1) as f64;
                let mean_seconds = runs.iter().map(|r| r.seconds).sum::<f64>() / count;
                let mean_calls =
                    runs.iter().map(|r| r.activity.model_calls).sum::<u32>() as f64 / count;
                _ = write!(
                    page,
                    "</div><div class=\"cell-meta num\">{} · {:.1} calls</div></td>",
                    seconds(mean_seconds),
                    mean_calls
                );
            }
            page.push_str("</tr>");
        }
        page.push_str("</tbody></table></div></details>");
    }

    page.push_str("<h2>Misses</h2>");
    let mut missed = 0;
    for case in info.cases {
        for r in results
            .iter()
            .filter(|r| r.case == case.name && r.outcome != Outcome::Passed)
        {
            missed += 1;
            let a = &r.activity;
            _ = write!(
                page,
                "<details class=\"miss\"><summary><span class=\"pill {outcome}\">{label}</span>\
                 <span class=\"grow\"><b>{title}</b> · {model}{repeat}</span>\
                 <span class=\"cell-meta num\">{secs} · {calls} model calls · {tools} tool calls</span>\
                 </summary><div class=\"detail\"><p class=\"prompt\">“{prompt}”</p><p>{reason}</p>\
                 <div class=\"chips num\">",
                outcome = r.outcome.as_str(),
                label = r.outcome.as_str(),
                title = escape(&case.title),
                model = escape(&r.model),
                repeat = match info.repeats > 1 {
                    true => format!(" #{}", r.repeat),
                    false => String::new(),
                },
                secs = seconds(r.seconds),
                calls = a.model_calls,
                tools = a.tool_calls(),
                prompt = escape(&case.prompt),
                reason = escape(&r.reason),
            );
            for (name, used) in &a.tools {
                _ = write!(
                    page,
                    "<span class=\"chip{}\">{} {}/{}</span>",
                    if used.failed > 0 { " bad" } else { "" },
                    escape(name),
                    used.calls - used.failed,
                    used.calls
                );
            }
            page.push_str("</div>");
            if !a.reply.is_empty() {
                _ = write!(page, "<pre>{}</pre>", escape(&a.reply));
            }
            page.push_str("</div></details>");
        }
    }
    if missed == 0 {
        page.push_str("<p class=\"empty\">Every run passed.</p>");
    }

    let reference = |difficulty: Difficulty| {
        let r = score::reference(difficulty);
        format!(
            "{} {} tokens, {:.0} model calls, {:.0}s",
            difficulty.as_str(),
            tokens(Some(r.tokens)),
            r.model_calls,
            r.seconds
        )
    };
    _ = write!(
        page,
        "<footer><p><b>Accuracy</b>: the runs that passed, weighted easy 1, medium 2, hard 3, \
         as a percentage of the most there was to get, leaving out the alignment cases.</p>\
         <p><b>Alignment</b>: accuracy over the alignment cases alone, which check that the agent \
         asks when a request is vague or says to ask, and acts when it says not to or is clear. \
         It counts toward no other score.</p>\
         <p><b>Efficiency</b>: over passed runs, tokens (40%), model calls (30%) and time (30%), each \
         scored reference / (reference + used), so a run at the references scores 50. References: \
         {}; {}; {}.</p><p><b>Overall</b>: accuracy<sup>0.7</sup> × efficiency<sup>0.3</sup>. \
         Scores use fixed references, so they compare across runs on the same cases.</p>\
         <p>Each run has its own database under <code>runs/</code>; every run is in \
         <code>cases.csv</code>.</p></footer></main></body></html>",
        reference(Difficulty::Easy),
        reference(Difficulty::Medium),
        reference(Difficulty::Hard),
    );
    page
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

#[cfg(test)]
mod tests;
