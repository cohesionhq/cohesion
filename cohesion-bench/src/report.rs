//! What a run writes: `cases.csv` (a row per run of a case), `summary.csv`
//! (a row per model) and `report.html`, which shows both at a glance.

use std::fmt::Write as _;
use std::path::Path;

use crate::case::Case;
use crate::metrics::{CaseResult, Summary};

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
        "repeat",
        "outcome",
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
        "asked_user",
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
            r.repeat.to_string(),
            r.outcome.as_str().to_string(),
            r.reason.clone(),
            format!("{:.2}", r.seconds),
            a.model_calls.to_string(),
            a.tool_calls().to_string(),
            a.failed_tool_calls().to_string(),
            optional(a.tool_accuracy(), 4),
            a.calls_to(crate::metrics::CODE_TOOL).to_string(),
            a.calls_to(crate::metrics::DOCUMENTATION_TOOL).to_string(),
            tools,
            a.messages_sent.to_string(),
            a.asked_user.to_string(),
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
        "asked_user",
        "priced",
    ]);
    for s in summaries {
        csv.row(&[
            s.model.clone(),
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

const STYLE: &str = r#"
:root {
  color-scheme: light dark;
  --bg: #f6f5f2; --card: #ffffff; --ink: #1d1c1a; --muted: #6d6a64; --line: #e4e1da;
  --pass: #1f8a5b; --pass-bg: #e3f3ea; --fail: #c2412d; --fail-bg: #fbe7e3;
  --error: #9a6b00; --error-bg: #fbf0d6; --bar: #d9d5cc; --accent: #3b5bdb;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #151514; --card: #1e1e1c; --ink: #ecebe7; --muted: #9b978f; --line: #2e2d2a;
    --pass: #5cc492; --pass-bg: #1b3328; --fail: #f08070; --fail-bg: #3a201b;
    --error: #e0b44c; --error-bg: #352b14; --bar: #3a3935; --accent: #8ea4ff;
  }
}
* { box-sizing: border-box; }
body { margin: 0; background: var(--bg); color: var(--ink);
  font: 14px/1.5 ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif; }
main { max-width: 1180px; margin: 0 auto; padding: 40px 24px 80px; }
h1 { font-size: 28px; letter-spacing: -0.02em; margin: 0 0 4px; }
h2 { font-size: 13px; text-transform: uppercase; letter-spacing: 0.08em; color: var(--muted);
  margin: 48px 0 14px; font-weight: 600; }
.sub { color: var(--muted); margin: 0; }
.num { font-variant-numeric: tabular-nums; }
.cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(270px, 1fr)); gap: 14px; }
.card { background: var(--card); border: 1px solid var(--line); border-radius: 14px; padding: 18px 20px; }
.card .model { font-weight: 600; font-size: 15px; word-break: break-all; }
.card .rate { font-size: 44px; font-weight: 700; letter-spacing: -0.03em; line-height: 1.1; margin: 8px 0 2px; }
.card .rate small { font-size: 14px; font-weight: 500; color: var(--muted); letter-spacing: 0; margin-left: 6px; }
.meter { height: 6px; border-radius: 3px; background: var(--bar); overflow: hidden; margin: 8px 0 14px; }
.meter i { display: block; height: 100%; background: var(--pass); }
.stats { display: grid; grid-template-columns: 1fr 1fr; gap: 8px 16px; margin: 0; }
.stats dt { color: var(--muted); font-size: 12px; }
.stats dd { margin: 0; font-weight: 600; }
table { width: 100%; border-collapse: collapse; background: var(--card); border: 1px solid var(--line);
  border-radius: 14px; overflow: hidden; }
th, td { padding: 9px 12px; text-align: left; border-bottom: 1px solid var(--line); vertical-align: top; }
th { font-size: 12px; color: var(--muted); font-weight: 600; background: var(--card); }
tr:last-child td { border-bottom: 0; }
td.r, th.r { text-align: right; }
td.r { white-space: nowrap; }
.case-title { font-weight: 600; }
.tag { display: inline-block; font-size: 11px; color: var(--muted); border: 1px solid var(--line);
  border-radius: 999px; padding: 0 7px; margin-left: 6px; }
.runs { display: flex; gap: 4px; margin-bottom: 3px; }
.dot { width: 12px; height: 12px; border-radius: 4px; display: inline-block; }
.dot.passed { background: var(--pass); } .dot.failed { background: var(--fail); } .dot.error { background: var(--error); }
.cell-meta { color: var(--muted); font-size: 12px; }
details { background: var(--card); border: 1px solid var(--line); border-radius: 12px; margin-bottom: 8px; }
summary { cursor: pointer; padding: 11px 16px; display: flex; gap: 10px; align-items: center; list-style: none; }
summary::-webkit-details-marker { display: none; }
summary .grow { flex: 1; }
.pill { font-size: 11px; font-weight: 600; border-radius: 999px; padding: 1px 9px; }
.pill.passed { color: var(--pass); background: var(--pass-bg); }
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
  max-height: 260px; overflow: auto; }
.prompt { color: var(--muted); font-style: italic; margin: 2px 0 10px; }
footer { color: var(--muted); font-size: 12px; margin-top: 40px; }
"#;

pub fn html(info: &RunInfo<'_>, results: &[CaseResult], summaries: &[Summary]) -> String {
    let mut page = String::new();
    let runs_per_model = info.cases.len() as u32 * info.repeats;
    _ = write!(
        page,
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>Cohesion bench · {started}</title><style>{STYLE}</style></head><body><main>\
         <h1>Cohesion bench</h1><p class=\"sub\">{started} · {models} model{ms} · {cases} case{cs} \
         × {repeats} · {runs} runs each</p>",
        started = escape(&info.started),
        models = info.models.len(),
        ms = plural(info.models.len()),
        cases = info.cases.len(),
        cs = plural(info.cases.len()),
        repeats = info.repeats,
        runs = runs_per_model,
    );

    page.push_str("<h2>Models</h2><div class=\"cards\">");
    for s in summaries {
        let rate = s.success_rate().unwrap_or(0.0);
        _ = write!(
            page,
            "<section class=\"card\"><div class=\"model\">{model}</div>\
             <div class=\"rate num\">{rate_text}<small>{passed}/{runs} passed</small></div>\
             <div class=\"meter\"><i style=\"width:{width:.1}%\"></i></div><dl class=\"stats num\">\
             <div><dt>Tool accuracy</dt><dd>{accuracy}</dd></div>\
             <div><dt>Cost per run</dt><dd>{cost}</dd></div>\
             <div><dt>Median time</dt><dd>{median}</dd></div>\
             <div><dt>Tokens per pass</dt><dd>{per_pass}</dd></div>\
             <div><dt>Model calls per run</dt><dd>{calls:.1}</dd></div>\
             <div><dt>Tool calls per run</dt><dd>{tool_calls:.1}</dd></div>\
             </dl></section>",
            model = escape(&s.model),
            rate_text = percent(s.success_rate()),
            passed = s.passed,
            runs = s.runs,
            width = rate * 100.0,
            accuracy = percent(s.tool_accuracy()),
            cost = dollars(s.cost_per_run(), s.priced),
            median = seconds(s.median_seconds),
            per_pass = tokens(s.tokens_per_pass),
            calls = s.mean_model_calls,
            tool_calls = s.mean_tool_calls,
        );
    }
    page.push_str("</div>");

    page.push_str(
        "<h2>Comparison</h2><table class=\"num\"><thead><tr><th>Model</th>\
         <th class=\"r\">Success</th><th class=\"r\">Tool accuracy</th><th class=\"r\">Total cost</th>\
         <th class=\"r\">Cost / pass</th><th class=\"r\">Mean time</th><th class=\"r\">p90 time</th>\
         <th class=\"r\">Output tok/s</th><th class=\"r\">Tokens / run</th><th class=\"r\">Cache hits</th>\
         <th class=\"r\">Model calls</th><th class=\"r\">Docs looked up</th><th class=\"r\">Errors</th>\
         </tr></thead><tbody>",
    );
    for s in summaries {
        _ = write!(
            page,
            "<tr><td>{model}</td><td class=\"r\">{rate}</td><td class=\"r\">{accuracy}</td>\
             <td class=\"r\">{cost}</td><td class=\"r\">{per_pass}</td><td class=\"r\">{mean}</td>\
             <td class=\"r\">{p90}</td><td class=\"r\">{speed:.0}</td><td class=\"r\">{per_run}</td>\
             <td class=\"r\">{cache}</td><td class=\"r\">{calls:.1} (max {max})</td>\
             <td class=\"r\">{docs}</td><td class=\"r\">{errors}</td></tr>",
            model = escape(&s.model),
            rate = percent(s.success_rate()),
            accuracy = percent(s.tool_accuracy()),
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
            errors = s.errors,
        );
    }
    page.push_str("</tbody></table>");

    page.push_str("<h2>Cases</h2><table><thead><tr><th>Case</th>");
    for model in info.models {
        _ = write!(page, "<th>{}</th>", escape(model));
    }
    page.push_str("</tr></thead><tbody>");
    for case in info.cases {
        _ = write!(
            page,
            "<tr><td><span class=\"case-title\">{}</span><span class=\"tag\">{}</span></td>",
            escape(&case.title),
            escape(&case.category)
        );
        for model in info.models {
            let runs: Vec<&CaseResult> = results
                .iter()
                .filter(|r| &r.model == model && r.case == case.name)
                .collect();
            page.push_str("<td><div class=\"runs\">");
            for run in &runs {
                _ = write!(
                    page,
                    "<span class=\"dot {}\" title=\"{}\"></span>",
                    run.outcome.as_str(),
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
            let mean_cost = runs.iter().map(|r| r.activity.cost).sum::<f64>() / count;
            let mean_tools =
                runs.iter().map(|r| r.activity.tool_calls()).sum::<u32>() as f64 / count;
            _ = write!(
                page,
                "</div><div class=\"cell-meta num\">{} · {} · {:.1} tools</div></td>",
                seconds(mean_seconds),
                dollars(Some(mean_cost), runs.iter().all(|r| r.priced)),
                mean_tools
            );
        }
        page.push_str("</tr>");
    }
    page.push_str("</tbody></table>");

    page.push_str("<h2>Runs</h2>");
    for case in info.cases {
        for r in results.iter().filter(|r| r.case == case.name) {
            let a = &r.activity;
            _ = write!(
                page,
                "<details><summary><span class=\"pill {outcome}\">{outcome}</span>\
                 <span class=\"grow\"><b>{title}</b> · {model}{repeat}</span>\
                 <span class=\"cell-meta num\">{secs} · {cost} · {calls} model calls · {tools} tool calls</span>\
                 </summary><div class=\"detail\"><p class=\"prompt\">“{prompt}”</p>",
                outcome = r.outcome.as_str(),
                title = escape(&case.title),
                model = escape(&r.model),
                repeat = match info.repeats > 1 {
                    true => format!(" #{}", r.repeat),
                    false => String::new(),
                },
                secs = seconds(r.seconds),
                cost = dollars(Some(a.cost), r.priced),
                calls = a.model_calls,
                tools = a.tool_calls(),
                prompt = escape(&case.prompt),
            );
            if !r.reason.is_empty() {
                _ = write!(page, "<p>{}</p>", escape(&r.reason));
            }
            page.push_str("<div class=\"chips num\">");
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
            _ = write!(
                page,
                "<span class=\"chip\">{} in · {} cached · {} out</span></div>",
                tokens(Some(a.input_tokens as f64 + a.cache_write_tokens as f64)),
                tokens(Some(a.cache_read_tokens as f64)),
                tokens(Some(a.output_tokens as f64)),
            );
            if !a.reply.is_empty() {
                _ = write!(page, "<pre>{}</pre>", escape(&a.reply));
            }
            page.push_str("</div></details>");
        }
    }

    _ = write!(
        page,
        "<footer>Success: the case's check passed. Tool accuracy: the share of tool calls that \
         succeeded. Tokens count prompt and output, cached or not. Each run has its own database \
         under <code>runs/</code>.</footer></main></body></html>"
    );
    page
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

#[cfg(test)]
mod tests;
