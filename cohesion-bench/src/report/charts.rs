//! The report's charts, drawn as inline SVG with no script: each mark
//! carries a `<title>`, which the browser shows on hover. A model keeps
//! its colour wherever it appears, from the order the models were named,
//! so a ranking never repaints it; past the eighth, models share the
//! muted ink.

use std::fmt::Write as _;

use super::escape;

/// How many models get a colour of their own.
const SERIES: usize = 8;

/// The colour of the model named `slot`th.
pub fn color(slot: usize) -> String {
    match slot < SERIES {
        true => format!("var(--s{})", slot + 1),
        false => "var(--muted)".to_string(),
    }
}

/// A model's name in a chart: past its provider's prefix, which the
/// mark's tooltip still gives in full.
pub fn short(model: &str) -> &str {
    model.rsplit('/').next().unwrap_or(model)
}

/// A swatch and name for each model, above a chart that colours by model.
pub fn legend(models: &[(String, usize)]) -> String {
    let mut out = String::from("<div class=\"legend\">");
    for (model, slot) in models {
        _ = write!(
            out,
            "<span title=\"{}\"><i style=\"background:{}\"></i>{}</span>",
            escape(model),
            color(*slot),
            escape(short(model))
        );
    }
    out.push_str("</div>");
    out
}

/// One model's point on a scatter.
pub struct Point {
    pub label: String,
    pub slot: usize,
    pub x: f64,
    pub y: f64,
    pub tip: String,
}

/// A 1-2-5 sequence of ticks covering `low..=high`, about `count` of them.
fn ticks(low: f64, high: f64, count: usize) -> Vec<f64> {
    let span = (high - low).max(f64::EPSILON);
    let raw = span / count.max(1) as f64;
    let magnitude = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|m| m * magnitude)
        .find(|step| span / step <= count as f64)
        .unwrap_or(10.0 * magnitude);
    let mut tick = (low / step).ceil() * step;
    let mut out = Vec::new();
    while tick <= high + step * 1e-9 {
        out.push(tick);
        tick += step;
    }
    out
}

/// Ticks on a log scale over `low..=high`: each power of ten, and its 2
/// and 5 when the range spans little.
fn log_ticks(low: f64, high: f64) -> Vec<f64> {
    let first = low.log10().floor() as i32;
    let last = high.log10().ceil() as i32;
    let multiples: &[f64] = if last - first <= 2 {
        &[1.0, 2.0, 5.0]
    } else {
        &[1.0]
    };
    (first..=last)
        .flat_map(|power| multiples.iter().map(move |m| m * 10f64.powi(power)))
        .filter(|tick| *tick >= low && *tick <= high)
        .collect()
}

/// Where a score axis starts: a round ten below the lowest of `values`,
/// so the scores fill the chart. Dots and lines need no zero baseline the
/// way bars do; the axis says where it starts.
fn floor_of(values: impl Iterator<Item = f64>) -> f64 {
    let low = values.fold(100.0, f64::min);
    (((low - 5.0) / 10.0).floor() * 10.0).clamp(0.0, 90.0)
}

/// Spreads labels wanted at `ys` apart by at least `gap`, keeping their
/// order, inside `top..=bottom`; the positions, in the input's order.
fn spread(ys: &[f64], gap: f64, top: f64, bottom: f64) -> Vec<f64> {
    let mut order: Vec<usize> = (0..ys.len()).collect();
    order.sort_by(|a, b| ys[*a].total_cmp(&ys[*b]));
    let mut placed = vec![0.0; ys.len()];
    let mut last = f64::NEG_INFINITY;
    for &i in &order {
        let y = ys[i].max(last + gap).max(top);
        placed[i] = y;
        last = y;
    }
    // Pushed past the bottom: walk back up.
    let mut next = f64::INFINITY;
    for &i in order.iter().rev() {
        let y = placed[i].min(next - gap).min(bottom);
        placed[i] = y;
        next = y;
    }
    placed
}

/// A scatter of a score (0 to 100, up) against a measure on a log scale
/// (along), each model labelled beside its point.
pub fn scatter(points: &[Point], x_title: &str, format_x: fn(f64) -> String) -> String {
    const W: f64 = 560.0;
    const H: f64 = 300.0;
    const LEFT: f64 = 40.0;
    const RIGHT: f64 = 150.0;
    const TOP: f64 = 14.0;
    const BOTTOM: f64 = 44.0;
    let points: Vec<&Point> = points.iter().filter(|p| p.x > 0.0).collect();
    let mut out = format!(
        "<svg class=\"chart\" viewBox=\"0 0 {W} {H}\" role=\"img\" aria-label=\"Accuracy against {}\">",
        escape(x_title)
    );
    if points.is_empty() {
        out.push_str("</svg>");
        return out;
    }
    let low = points.iter().map(|p| p.x).fold(f64::INFINITY, f64::min) / 1.6;
    let high = points.iter().map(|p| p.x).fold(0.0, f64::max) * 1.6;
    let x = |v: f64| LEFT + (v.ln() - low.ln()) / (high.ln() - low.ln()) * (W - LEFT - RIGHT);
    let floor = floor_of(points.iter().map(|p| p.y));
    let y = |v: f64| TOP + (100.0 - v.clamp(floor, 100.0)) / (100.0 - floor) * (H - TOP - BOTTOM);
    for tick in ticks(floor, 100.0, 5) {
        _ = write!(
            out,
            "<line class=\"grid\" x1=\"{LEFT}\" x2=\"{:.1}\" y1=\"{y:.1}\" y2=\"{y:.1}\"/>\
             <text class=\"tick\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\">{tick:.0}</text>",
            W - RIGHT,
            LEFT - 6.0,
            y(tick) + 4.0,
            y = y(tick),
        );
    }
    for tick in log_ticks(low, high) {
        _ = write!(
            out,
            "<line class=\"grid\" x1=\"{x:.1}\" x2=\"{x:.1}\" y1=\"{TOP}\" y2=\"{:.1}\"/>\
             <text class=\"tick\" x=\"{x:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{}</text>",
            H - BOTTOM,
            H - BOTTOM + 16.0,
            escape(&format_x(tick)),
            x = x(tick),
        );
    }
    _ = write!(
        out,
        "<line class=\"axis\" x1=\"{LEFT}\" x2=\"{:.1}\" y1=\"{b:.1}\" y2=\"{b:.1}\"/>\
         <text class=\"axis-title\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{} (log scale) →</text>\
         <text class=\"axis-title\" x=\"{LEFT}\" y=\"{:.1}\" text-anchor=\"start\">↑ accuracy</text>",
        W - RIGHT,
        (LEFT + W - RIGHT) / 2.0,
        H - 8.0,
        escape(x_title),
        TOP - 2.0,
        b = H - BOTTOM,
    );
    let labels = spread(
        &points.iter().map(|p| y(p.y) + 4.0).collect::<Vec<_>>(),
        13.0,
        TOP + 4.0,
        H - BOTTOM,
    );
    let label_x = W - RIGHT + 14.0;
    for (point, label_y) in points.iter().zip(labels) {
        let (px, py) = (x(point.x), y(point.y));
        _ = write!(
            out,
            "<line class=\"leader\" x1=\"{:.1}\" y1=\"{py:.1}\" x2=\"{:.1}\" y2=\"{:.1}\"/>\
             <text class=\"label\" x=\"{label_x:.1}\" y=\"{label_y:.1}\">{}</text>\
             <g class=\"mark\"><title>{}</title><circle cx=\"{px:.1}\" cy=\"{py:.1}\" r=\"11\" class=\"hit\"/>\
             <circle cx=\"{px:.1}\" cy=\"{py:.1}\" r=\"6\" fill=\"{}\" class=\"ring\"/></g>",
            px + 8.0,
            label_x - 4.0,
            label_y - 4.0,
            escape(short(&point.label)),
            escape(&point.tip),
            color(point.slot),
        );
    }
    out.push_str("</svg>");
    out
}

/// One model's line across the difficulties.
pub struct Slope {
    pub label: String,
    pub slot: usize,
    /// Easy to hard, `None` where it has no runs.
    pub values: [Option<f64>; 3],
}

/// Accuracy at each difficulty as a line per model, labelled at its end.
pub fn slopes(lines: &[Slope]) -> String {
    const W: f64 = 560.0;
    const H: f64 = 300.0;
    const LEFT: f64 = 40.0;
    const RIGHT: f64 = 150.0;
    const TOP: f64 = 14.0;
    const BOTTOM: f64 = 44.0;
    let x = |i: usize| LEFT + 24.0 + i as f64 * (W - LEFT - RIGHT - 48.0) / 2.0;
    let floor = floor_of(
        lines
            .iter()
            .flat_map(|line| line.values.iter().flatten().copied()),
    );
    let y = |v: f64| TOP + (100.0 - v.clamp(floor, 100.0)) / (100.0 - floor) * (H - TOP - BOTTOM);
    let mut out = format!(
        "<svg class=\"chart\" viewBox=\"0 0 {W} {H}\" role=\"img\" aria-label=\"Accuracy by difficulty\">"
    );
    for tick in ticks(floor, 100.0, 5) {
        _ = write!(
            out,
            "<line class=\"grid\" x1=\"{LEFT}\" x2=\"{:.1}\" y1=\"{y:.1}\" y2=\"{y:.1}\"/>\
             <text class=\"tick\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\">{tick:.0}</text>",
            W - RIGHT,
            LEFT - 6.0,
            y(tick) + 4.0,
            y = y(tick),
        );
    }
    for (i, name) in ["Easy", "Medium", "Hard"].iter().enumerate() {
        _ = write!(
            out,
            "<text class=\"axis-title\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{name}</text>",
            x(i),
            H - BOTTOM + 20.0
        );
    }
    for line in lines {
        let points: Vec<(f64, f64)> = line
            .values
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.map(|v| (x(i), y(v))))
            .collect();
        let path: Vec<String> = points
            .iter()
            .map(|(px, py)| format!("{px:.1},{py:.1}"))
            .collect();
        _ = write!(
            out,
            "<polyline class=\"line\" points=\"{}\" stroke=\"{}\"/>",
            path.join(" "),
            color(line.slot)
        );
    }
    for line in lines {
        for (i, value) in line.values.iter().enumerate() {
            if let Some(value) = value {
                _ = write!(
                    out,
                    "<g class=\"mark\"><title>{} · {}: {value:.1}</title>\
                     <circle cx=\"{px:.1}\" cy=\"{py:.1}\" r=\"10\" class=\"hit\"/>\
                     <circle cx=\"{px:.1}\" cy=\"{py:.1}\" r=\"4.5\" fill=\"{}\" class=\"ring\"/></g>",
                    escape(&line.label),
                    ["easy", "medium", "hard"][i],
                    color(line.slot),
                    px = x(i),
                    py = y(*value),
                );
            }
        }
    }
    let ends: Vec<f64> = lines
        .iter()
        .map(|line| {
            line.values
                .iter()
                .rev()
                .flatten()
                .next()
                .map_or(H, |v| y(*v))
                + 4.0
        })
        .collect();
    let label_x = x(2) + 14.0;
    for (line, label_y) in lines.iter().zip(spread(&ends, 13.0, TOP + 4.0, H - BOTTOM)) {
        _ = write!(
            out,
            "<text class=\"label\" x=\"{label_x:.1}\" y=\"{label_y:.1}\">{}</text>",
            escape(short(&line.label))
        );
    }
    out.push_str("</svg>");
    out
}

/// One category's scores, a model each.
pub struct CategoryRow {
    pub category: String,
    pub cases: usize,
    /// `(model, slot, accuracy)`.
    pub scores: Vec<(String, usize, Option<f64>)>,
}

/// Each category as a row on one 0–100 scale: a dot per model, joined by
/// the range between the best and the worst, so where models agree and
/// where they part shows at a glance.
pub fn category_dots(rows: &[CategoryRow]) -> String {
    const W: f64 = 1000.0;
    const LEFT: f64 = 200.0;
    const RIGHT: f64 = 70.0;
    const TOP: f64 = 30.0;
    const ROW: f64 = 30.0;
    let h = TOP + ROW * rows.len() as f64 + 10.0;
    let floor = floor_of(
        rows.iter()
            .flat_map(|row| row.scores.iter().filter_map(|(_, _, v)| *v)),
    );
    let x = |v: f64| LEFT + (v.clamp(floor, 100.0) - floor) / (100.0 - floor) * (W - LEFT - RIGHT);
    let mut out = format!(
        "<svg class=\"chart\" viewBox=\"0 0 {W} {h}\" role=\"img\" aria-label=\"Accuracy by category, a dot per model\">"
    );
    for tick in ticks(floor, 100.0, 5) {
        _ = write!(
            out,
            "<line class=\"grid\" x1=\"{x:.1}\" x2=\"{x:.1}\" y1=\"{TOP}\" y2=\"{:.1}\"/>\
             <text class=\"tick\" x=\"{x:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{tick:.0}</text>",
            h - 10.0,
            TOP - 10.0,
            x = x(tick),
        );
    }
    for (i, row) in rows.iter().enumerate() {
        let cy = TOP + ROW * (i as f64 + 0.5);
        let values: Vec<f64> = row.scores.iter().filter_map(|(_, _, v)| *v).collect();
        _ = write!(
            out,
            "<text class=\"row-label\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\">{}\
             <tspan class=\"tick\"> · {}</tspan></text>",
            LEFT - 14.0,
            cy + 4.0,
            escape(&row.category),
            row.cases
        );
        if let (Some(min), Some(max)) = (
            values.iter().copied().reduce(f64::min),
            values.iter().copied().reduce(f64::max),
        ) {
            _ = write!(
                out,
                "<line class=\"range\" x1=\"{:.1}\" x2=\"{:.1}\" y1=\"{cy:.1}\" y2=\"{cy:.1}\"/>\
                 <text class=\"tick\" x=\"{:.1}\" y=\"{:.1}\">{:.0} pt spread</text>",
                x(min),
                x(max),
                W - RIGHT + 10.0,
                cy + 4.0,
                max - min,
            );
        }
        for (model, slot, value) in &row.scores {
            if let Some(value) = value {
                _ = write!(
                    out,
                    "<g class=\"mark\"><title>{} · {}: {value:.1}</title>\
                     <circle cx=\"{cx:.1}\" cy=\"{cy:.1}\" r=\"11\" class=\"hit\"/>\
                     <circle cx=\"{cx:.1}\" cy=\"{cy:.1}\" r=\"6\" fill=\"{}\" class=\"ring\"/></g>",
                    escape(model),
                    escape(&row.category),
                    color(*slot),
                    cx = x(*value),
                );
            }
        }
    }
    out.push_str("</svg>");
    out
}

/// How one model did on one case, over its repeats.
#[derive(Clone, Copy)]
pub enum Cell {
    Passed,
    Mixed,
    Failed,
    Error,
    Missing,
}

/// A column of the case matrix: a case, and how each model did on it.
pub struct Column {
    pub category: String,
    pub tip: String,
    pub cells: Vec<Cell>,
}

/// Every case as a column and every model as a row, grouped by category
/// and wrapped into bands that fit the page at actual size:
/// a case everyone missed stands out as a red column, often a sign the
/// case itself needs a look.
pub fn case_matrix(models: &[String], columns: &[Column]) -> String {
    const WIDTH: f64 = 1150.0;
    const LEFT: f64 = 150.0;
    const HEAD: f64 = 34.0;
    const CELL: f64 = 6.0;
    const PITCH: f64 = 8.0;
    const ROW: f64 = 16.0;
    const GROUP_GAP: f64 = 16.0;
    const BAND_GAP: f64 = 18.0;
    // Categories in order, each with its columns.
    let mut groups: Vec<(&str, Vec<&Column>)> = Vec::new();
    for column in columns {
        match groups.last_mut() {
            Some((category, list)) if *category == column.category => list.push(column),
            _ => groups.push((&column.category, vec![column])),
        }
    }
    // Bands of whole categories that fit the width, at actual size.
    let span = |group: &(&str, Vec<&Column>)| group.1.len() as f64 * PITCH;
    let mut bands: Vec<Vec<&(&str, Vec<&Column>)>> = vec![Vec::new()];
    let mut used = LEFT;
    for group in &groups {
        let band = bands.last_mut().expect("there is always a band");
        if !band.is_empty() && used + span(group) > WIDTH {
            bands.push(Vec::new());
            used = LEFT;
        }
        bands.last_mut().expect("just pushed").push(group);
        used += span(group) + GROUP_GAP;
    }
    let band_height = HEAD + ROW * models.len() as f64;
    let h = bands.len() as f64 * (band_height + BAND_GAP);
    let mut out = format!(
        "<svg class=\"chart matrix\" viewBox=\"0 0 {WIDTH} {h:.0}\" width=\"{WIDTH}\" role=\"img\" \
         aria-label=\"Every case by every model\">"
    );
    for (b, band) in bands.iter().enumerate() {
        let top = b as f64 * (band_height + BAND_GAP);
        for (row, model) in models.iter().enumerate() {
            _ = write!(
                out,
                "<text class=\"row-label\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\">{}<title>{}</title></text>",
                LEFT - 12.0,
                top + HEAD + ROW * row as f64 + 10.0,
                escape(short(model)),
                escape(model)
            );
        }
        let mut x = LEFT;
        for (category, list) in band.iter().map(|g| (g.0, &g.1)) {
            _ = write!(
                out,
                "<text class=\"label\" x=\"{x:.1}\" y=\"{:.1}\">{}</text>\
                 <line class=\"axis\" x1=\"{x:.1}\" x2=\"{:.1}\" y1=\"{:.1}\" y2=\"{:.1}\"/>",
                top + HEAD - 14.0,
                escape(category),
                x + list.len() as f64 * PITCH - (PITCH - CELL),
                top + HEAD - 7.0,
                top + HEAD - 7.0,
            );
            for column in list.iter() {
                for (row, model) in models.iter().enumerate() {
                    let (class, word) =
                        match column.cells.get(row).copied().unwrap_or(Cell::Missing) {
                            Cell::Passed => ("c-pass", "passed"),
                            Cell::Mixed => ("c-mixed", "passed some repeats"),
                            Cell::Failed => ("c-fail", "failed"),
                            Cell::Error => ("c-error", "the bench couldn't run it"),
                            Cell::Missing => continue,
                        };
                    _ = write!(
                        out,
                        "<rect class=\"{class}\" x=\"{x:.1}\" y=\"{:.1}\" width=\"{CELL}\" height=\"{:.1}\" rx=\"1.5\">\
                         <title>{} · {}: {word}</title></rect>",
                        top + HEAD + ROW * row as f64,
                        ROW - 3.0,
                        escape(model),
                        escape(&column.tip),
                    );
                }
                x += PITCH;
            }
            x += GROUP_GAP - (PITCH - CELL);
        }
    }
    out.push_str("</svg>");
    out
}

/// One model's run times.
pub struct Times {
    pub label: String,
    pub slot: usize,
    pub seconds: Vec<f64>,
}

/// Each model's runs as ticks along one log time scale, its median marked
/// and named, so a long tail shows where a mean would hide it.
pub fn time_strips(rows: &[Times], format_seconds: fn(f64) -> String) -> String {
    const W: f64 = 560.0;
    const LEFT: f64 = 140.0;
    const RIGHT: f64 = 64.0;
    const TOP: f64 = 10.0;
    const ROW: f64 = 34.0;
    const BOTTOM: f64 = 40.0;
    let h = TOP + ROW * rows.len() as f64 + BOTTOM;
    let all = rows
        .iter()
        .flat_map(|r| r.seconds.iter().copied())
        .filter(|s| *s > 0.0);
    let low = all.clone().fold(f64::INFINITY, f64::min).min(1.0) / 1.2;
    let high = all.fold(0.0, f64::max).max(2.0) * 1.2;
    let x =
        |v: f64| LEFT + (v.max(low).ln() - low.ln()) / (high.ln() - low.ln()) * (W - LEFT - RIGHT);
    let mut out = format!(
        "<svg class=\"chart\" viewBox=\"0 0 {W} {h:.0}\" role=\"img\" aria-label=\"Run times by model\">"
    );
    for tick in log_ticks(low, high) {
        _ = write!(
            out,
            "<line class=\"grid\" x1=\"{x:.1}\" x2=\"{x:.1}\" y1=\"{TOP}\" y2=\"{:.1}\"/>\
             <text class=\"tick\" x=\"{x:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{}</text>",
            h - BOTTOM,
            h - BOTTOM + 16.0,
            escape(&format_seconds(tick)),
            x = x(tick),
        );
    }
    _ = write!(
        out,
        "<text class=\"axis-title\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\">seconds per run (log scale) →</text>",
        (LEFT + W - RIGHT) / 2.0,
        h - 6.0
    );
    for (i, row) in rows.iter().enumerate() {
        let cy = TOP + ROW * (i as f64 + 0.5);
        _ = write!(
            out,
            "<text class=\"row-label\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\">{}<title>{}</title></text>",
            LEFT - 12.0,
            cy + 4.0,
            escape(short(&row.label)),
            escape(&row.label)
        );
        for seconds in &row.seconds {
            _ = write!(
                out,
                "<line class=\"run\" x1=\"{x:.1}\" x2=\"{x:.1}\" y1=\"{:.1}\" y2=\"{:.1}\" stroke=\"{}\"/>",
                cy - 8.0,
                cy + 8.0,
                color(row.slot),
                x = x(*seconds),
            );
        }
        let mut sorted = row.seconds.clone();
        sorted.sort_by(f64::total_cmp);
        if let Some(median) = sorted.get(sorted.len() / 2) {
            _ = write!(
                out,
                "<g class=\"mark\"><title>{} · median {}</title>\
                 <rect x=\"{:.1}\" y=\"{:.1}\" width=\"4\" height=\"24\" rx=\"2\" class=\"median\"/></g>\
                 <text class=\"tick\" x=\"{:.1}\" y=\"{:.1}\">{}</text>",
                escape(&row.label),
                escape(&format_seconds(*median)),
                x(*median) - 2.0,
                cy - 12.0,
                W - RIGHT + 8.0,
                cy + 4.0,
                escape(&format_seconds(*median)),
            );
        }
    }
    out.push_str("</svg>");
    out
}
