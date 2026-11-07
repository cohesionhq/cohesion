//! Aligned lists of rows, built with `row!`, and the cell formatters they use.

use jiff::{Timestamp, tz::TimeZone};

/// One cell of a `Row`, padded to its column's width.
pub enum Cell {
    Left(String),
    Right(String),
}

/// One line of a list, built with `row!`.
pub struct Row {
    cells: Vec<Cell>,
    details: Vec<String>,
}

impl Row {
    pub fn new(cells: Vec<Cell>) -> Self {
        Self {
            cells,
            details: Vec::new(),
        }
    }

    /// A line printed under the row's last column, after any added before it; `None` adds
    /// nothing.
    pub fn detail(mut self, detail: Option<String>) -> Self {
        self.details.extend(detail);
        self
    }
}

/// Prints one aligned row per item, or `empty` (a hint at how to add one) when there are none.
pub fn list<T>(items: &[T], empty: &str, row: impl Fn(&T) -> Row) {
    print_rows(items, empty, "", row);
}

/// Like `list`, under a `title` heading.
pub fn section<T>(title: &str, items: &[T], empty: &str, row: impl Fn(&T) -> Row) {
    println!("{title}");
    print_rows(items, empty, "  ", row);
}

fn print_rows<T>(items: &[T], empty: &str, indent: &str, row: impl Fn(&T) -> Row) {
    if items.is_empty() {
        println!("{indent}{empty}");
        return;
    }
    let rows: Vec<_> = items.iter().map(row).collect();
    for line in render(&rows) {
        println!("{indent}{line}");
    }
}

/// `rows` as lines, each column as wide as its widest cell and two spaces apart.
fn render(rows: &[Row]) -> Vec<String> {
    let mut widths = Vec::new();
    for row in rows {
        widths.resize(widths.len().max(row.cells.len()), 0);
        for (width, cell) in widths.iter_mut().zip(&row.cells) {
            let (Cell::Left(text) | Cell::Right(text)) = cell;
            *width = (*width).max(text.chars().count());
        }
    }

    let mut lines = Vec::new();
    for row in rows {
        let cells: Vec<_> = row
            .cells
            .iter()
            .zip(&widths)
            .map(|(cell, &width)| match cell {
                Cell::Left(text) => format!("{text:width$}"),
                Cell::Right(text) => format!("{text:>width$}"),
            })
            .collect();
        lines.push(cells.join("  ").trim_end().to_string());
        let last = row.cells.len().saturating_sub(1);
        let indent: usize = widths[..last].iter().map(|width| width + 2).sum();
        for detail in &row.details {
            lines.push(format!("{:indent$}{detail}", ""));
        }
    }
    lines
}

/// `text` cut to at most `max` characters, ending in `…` when cut.
pub fn truncate(text: &str, max: usize) -> String {
    match text.chars().count() > max {
        true => text.chars().take(max - 1).chain(['…']).collect(),
        false => text.to_string(),
    }
}

/// `seconds` as a short relative time, like `5m ago`.
pub fn ago(seconds: i64) -> String {
    match seconds {
        ..60 => "just now".to_string(),
        60..3600 => format!("{}m ago", seconds / 60),
        3600..86400 => format!("{}h ago", seconds / 3600),
        _ => format!("{}d ago", seconds / 86400),
    }
}

/// Unix `seconds` as a local date and time, like `Oct 14 10:30`.
pub fn clock(seconds: i64) -> String {
    Timestamp::from_second(seconds)
        .expect("the daemon stores valid timestamps")
        .to_zoned(TimeZone::system())
        .strftime("%b %-d %H:%M")
        .to_string()
}

/// `seconds` as a short duration, like `42s` or `3m 05s`.
pub fn duration(seconds: i64) -> String {
    match seconds {
        ..60 => format!("{seconds}s"),
        60..3600 => format!("{}m {:02}s", seconds / 60, seconds % 60),
        _ => format!("{}h {:02}m", seconds / 3600, seconds % 3600 / 60),
    }
}
