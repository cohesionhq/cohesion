//! The `.d.ts` of the installed services, rendered at runtime: one
//! `interface ServiceMap` block that merges into the empty one the static
//! `services` global declares in [`TS_DECLARATIONS`], so
//! `services.use("<identifier>")` is typed by the service it names.
//!
//! [`TS_DECLARATIONS`]: crate::script_api::TS_DECLARATIONS

use std::fmt::Write;

use cohesion_core::model::InstalledService;

/// The note on a function a read-only caller cannot call.
const NOT_READ_ONLY: &str = "Not available in read-only contexts.";

/// The `ServiceMap` merge declaring every installed service, keyed by its
/// extension's script identifier; empty when there is none. Every context
/// sees the same view: functions not declared read-only say so.
pub fn service_declarations(services: &[InstalledService]) -> String {
    if services.is_empty() {
        return String::new();
    }
    let mut out = String::from("interface ServiceMap {\n");
    for service in services {
        let summary = match service.description.as_str() {
            "" => service.name.clone(),
            description => format!("{}: {description}", service.name),
        };
        out += &jsdoc(&summary, "  ");
        // A JSON string is a TypeScript string literal, so the key stays
        // one key whatever the identifier holds.
        let key = serde_json::to_string(&service.script_identifier).unwrap_or_default();
        _ = writeln!(out, "  {key}: {{");
        for function in &service.functions {
            let description = match (function.read_only, function.description.as_str()) {
                (true, description) => description.to_string(),
                (false, "") => NOT_READ_ONLY.to_string(),
                (false, description) => format!("{description} {NOT_READ_ONLY}"),
            };
            out += &jsdoc(&description, "    ");
            let args = match &function.args {
                Some(args) => format!("args: {}", type_text(args)),
                None => String::new(),
            };
            let returns = function.returns.as_deref().map_or("unknown", type_text);
            _ = writeln!(out, "    {}({args}): {returns};", function.name);
        }
        out += "  };\n";
    }
    out.push('}');
    out
}

/// `text` as a one-line JSDoc comment at `indent`, or nothing for no text.
/// Install already collapsed it to one line; `*/` is escaped so it cannot
/// close the comment.
fn jsdoc(text: &str, indent: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    format!("{indent}/** {} */\n", text.replace("*/", "*\\/"))
}

/// An extension-written type, or `unknown` when it could reach past its
/// slot in the signature: brackets that don't balance, an unclosed string,
/// a `/` (which could open a comment), or a `,` or `;` outside any bracket
/// (another parameter, or the end of the declaration).
fn type_text(text: &str) -> &str {
    if contained(text) { text } else { "unknown" }
}

/// Whether `text` stays one TypeScript type when spliced into a signature.
fn contained(text: &str) -> bool {
    let mut open = Vec::new();
    let mut quote = None;
    let mut chars = text.chars().peekable();
    let mut previous = ' ';
    while let Some(c) = chars.next() {
        if let Some(q) = quote {
            match c {
                '\\' => {
                    chars.next();
                }
                _ if c == q => quote = None,
                _ => {}
            }
            previous = c;
            continue;
        }
        match c {
            '"' | '\'' | '`' => quote = Some(c),
            '(' | '[' | '{' | '<' => open.push(c),
            // The `>` of `=>` closes nothing.
            '>' if previous == '=' => {}
            ')' | ']' | '}' | '>' => {
                let expected = match c {
                    ')' => '(',
                    ']' => '[',
                    '}' => '{',
                    _ => '<',
                };
                if open.pop() != Some(expected) {
                    return false;
                }
            }
            '/' => return false,
            ',' | ';' if open.is_empty() => return false,
            _ => {}
        }
        previous = c;
    }
    quote.is_none() && open.is_empty() && !text.trim().is_empty()
}

#[cfg(test)]
mod tests;
