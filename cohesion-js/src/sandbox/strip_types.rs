//! Erases TypeScript annotations so QuickJS can run the script. Types are
//! replaced with whitespace rather than reprinted, so line and column numbers
//! in exceptions still point into the source as written. Plain JavaScript
//! passes through unchanged.
//!
//! The parser (`parser.rs`, with the type grammar in `types.rs`) walks the
//! source without building a tree and records the spans to blank. It reads
//! JavaScript only as closely as finding the types takes: QuickJS is the
//! judge of JavaScript syntax, so source the parser can't follow is handed
//! on as it stands for QuickJS to report.

mod lexer;
mod parser;
mod types;

use std::fmt;
use std::ops::Range;

use parser::Parser;

use crate::script_error::ScriptError;

/// Strips types from `source`, read as a script (not a module) to match
/// how the sandbox evaluates it. Syntax that has runtime behaviour, such as
/// `enum`, a `namespace` with values or constructor parameter properties,
/// is rejected rather than compiled.
pub(crate) fn strip_types(source: String) -> Result<String, ScriptError> {
    let mut parser = Parser::new(&source);
    match parser.program() {
        Ok(()) | Err(Stop::Unparsed) => {}
        Err(Stop::Refused { refusal, at }) => {
            let (line, column) = line_and_column(&source, at);
            return Err(ScriptError::Exception {
                message: format!("{refusal} (line {line}, column {column})"),
                stack: None,
            });
        }
    }
    let edits = parser.into_edits();
    if edits.is_empty() {
        return Ok(source);
    }
    Ok(edits.apply(&source))
}

/// The lexer or parser met source it can't follow.
struct Unparsed;

/// Why parsing stopped early.
enum Stop {
    /// See [`Unparsed`]. What was erased before this point still applies.
    Unparsed,
    /// TypeScript that can't be erased, at this byte offset.
    Refused { refusal: Refusal, at: usize },
}

impl From<Unparsed> for Stop {
    fn from(_: Unparsed) -> Self {
        Self::Unparsed
    }
}

/// TypeScript that would have to be compiled, not erased.
#[derive(Clone, Copy)]
enum Refusal {
    Enum,
    Namespace,
    ParameterProperty,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Enum => "An `enum` can't run here. Use an object or a union of strings.",
            Self::Namespace => "A `namespace` that holds values can't run here. Use an object.",
            Self::ParameterProperty => {
                "A constructor parameter with `public`, `private`, `protected`, `readonly` or \
                 `override` can't run here. Declare the field and assign it in the constructor."
            }
        })
    }
}

/// The 1-based line and column (in characters) of byte `at`.
fn line_and_column(source: &str, at: usize) -> (usize, usize) {
    let before = &source[..at];
    let line_start = before.rfind('\n').map_or(0, |newline| newline + 1);
    (
        before.matches('\n').count() + 1,
        before[line_start..].chars().count() + 1,
    )
}

/// What stripping changes: spans to blank and single characters to write.
#[derive(Default)]
struct Edits {
    erased: Vec<Range<usize>>,
    /// A byte offset and the ASCII character that replaces the character
    /// there, whether or not the offset is erased.
    placed: Vec<(usize, u8)>,
}

impl Edits {
    fn is_empty(&self) -> bool {
        self.erased.is_empty() && self.placed.is_empty()
    }

    /// Blanks `span`.
    fn erase(&mut self, span: Range<usize>) {
        if !span.is_empty() {
            self.erased.push(span);
        }
    }

    /// Blanks `span`, a whole statement or class member, leaving a `;` at
    /// its start. Without it the line before, when it has no semicolon of
    /// its own, would run on into the line after:
    ///
    /// ```ts
    /// const a = b
    /// type T = number
    /// (c)
    /// ```
    fn erase_statement(&mut self, span: Range<usize>) {
        self.placed.retain(|(at, _)| !span.contains(at));
        self.placed.push((span.start, b';'));
        self.erase(span);
    }

    /// `source` with every edit made. An erased character becomes a space,
    /// but whitespace stays as it is, line breaks and tabs included, so
    /// every surviving character keeps its line and column.
    fn apply(mut self, source: &str) -> String {
        self.erased.sort_by_key(|span| span.start);
        self.placed.sort_unstable();
        let mut erased = self.erased.iter().peekable();
        let mut placed = self.placed.iter().peekable();
        let mut erased_until = 0;
        let mut out = String::with_capacity(source.len());
        for (at, character) in source.char_indices() {
            while let Some(span) = erased.next_if(|span| span.start <= at) {
                erased_until = erased_until.max(span.end);
            }
            let mut replacement = None;
            while let Some((_, byte)) = placed.next_if(|(position, _)| *position <= at) {
                replacement = Some(char::from(*byte));
            }
            match replacement {
                Some(replacement) => out.push(replacement),
                // Columns count UTF-16 units, so the blank is as wide.
                None if at < erased_until && !character.is_whitespace() => {
                    out.extend(std::iter::repeat_n(' ', character.len_utf16()));
                }
                None => out.push(character),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests;
