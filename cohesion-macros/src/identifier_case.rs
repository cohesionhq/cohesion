//! Rust identifiers respelled the way scripts see them.

/// `workspace_id` -> `workspaceId`: a field's name in scripts.
pub(crate) fn js_field_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut upper = false;
    for c in name.chars() {
        if c == '_' {
            upper = !out.is_empty();
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// `PascalCase` to `camelCase`, the same way serde's `rename_all` does it for
/// variants: only the first character is lowercased.
pub(crate) fn camel_case(ident: &str) -> String {
    let mut chars = ident.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}
