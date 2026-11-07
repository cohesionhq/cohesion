//! Checking the `service` block of a manifest, which comes from an
//! extension's script: its names become JavaScript and TypeScript, and its
//! text lands in the `.d.ts` agents and editors read.

use cohesion_core::model::{ExtensionManifest, ExtensionPermissionKind};
use cohesion_core::protocol::ServerError;

/// The longest function name, in characters.
const MAX_NAME: usize = 64;
/// The most functions one service declares.
const MAX_FUNCTIONS: usize = 128;
/// The longest description, in characters.
const MAX_DESCRIPTION: usize = 1024;
/// The longest `args` or `returns` type, in characters.
const MAX_TYPE: usize = 4096;

/// JavaScript's reserved words, and the property names an object already
/// has, none of which can name a function.
const RESERVED: &[&str] = &[
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "interface",
    "let",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
    "arguments",
    "eval",
    "undefined",
    "NaN",
    "Infinity",
    "__proto__",
    "constructor",
    "prototype",
    "hasOwnProperty",
    "isPrototypeOf",
    "propertyIsEnumerable",
    "toJSON",
    "toLocaleString",
    "toString",
    "valueOf",
];

/// Checks `manifest`'s `service` block, if it has one, and collapses its
/// text to single lines. The block needs `service` or `service:isolated`,
/// and a manifest never lists both.
pub(super) fn check_service(manifest: &mut ExtensionManifest) -> Result<(), ServerError> {
    let lists = |permission| manifest.permissions.contains(&permission);
    let service = lists(ExtensionPermissionKind::Service);
    let isolated = lists(ExtensionPermissionKind::ServiceIsolated);
    if service && isolated {
        return Err(invalid(
            "A manifest lists `service` or `service:isolated`, not both.".to_string(),
        ));
    }
    let Some(declaration) = manifest.service.as_mut() else {
        return Ok(());
    };
    if !service && !isolated {
        return Err(invalid(
            "A `service` block requires the `service` or `service:isolated` permission."
                .to_string(),
        ));
    }
    if declaration.functions.len() > MAX_FUNCTIONS {
        return Err(invalid(format!(
            "A service declares at most {MAX_FUNCTIONS} functions."
        )));
    }
    declaration.description = one_line(declaration.description.take(), MAX_DESCRIPTION)?;
    for (name, function) in &mut declaration.functions {
        check_name(name)?;
        function.description = one_line(function.description.take(), MAX_DESCRIPTION)?;
        function.args = one_line(function.args.take(), MAX_TYPE)?;
        function.returns = one_line(function.returns.take(), MAX_TYPE)?;
    }
    Ok(())
}

/// Fails unless `name` is an ASCII JavaScript identifier of at most
/// [`MAX_NAME`] characters that isn't [`RESERVED`].
fn check_name(name: &str) -> Result<(), ServerError> {
    let mut chars = name.chars();
    let identifier = chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_' || first == '$')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
    if !identifier || name.len() > MAX_NAME || RESERVED.contains(&name) {
        return Err(invalid(format!(
            "The service function `{name}` must be a JavaScript identifier of at most \
             {MAX_NAME} characters, and not a reserved word."
        )));
    }
    Ok(())
}

/// `text` with its whitespace runs, line breaks included, collapsed to one
/// space; `None` when nothing is left. Fails when longer than `max`
/// characters.
fn one_line(text: Option<String>, max: usize) -> Result<Option<String>, ServerError> {
    let Some(text) = text else {
        return Ok(None);
    };
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() > max {
        return Err(invalid(format!(
            "A service's descriptions are at most {MAX_DESCRIPTION} characters, and its \
             types at most {MAX_TYPE}."
        )));
    }
    Ok((!text.is_empty()).then_some(text))
}

fn invalid(message: String) -> ServerError {
    ServerError::validation(None, message)
}
