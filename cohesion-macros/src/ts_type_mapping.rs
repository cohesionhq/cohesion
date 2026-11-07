//! The TypeScript type a script sees for a Rust type.
//!
//! Types are mapped by how they are written, not resolved: `Vec<JsTask>`
//! becomes `Task[]` because of the `Js` prefix convention.

use syn::{GenericArgument, PathArguments, Type};

/// `T` if `ty` is written `Option<T>`.
pub fn option_inner(ty: &Type) -> Option<&Type> {
    match wrapper(ty) {
        (Some(name), inner) if name == "Option" => inner,
        _ => None,
    }
}

/// One argument as TS, or `None` for those rquickjs fills in itself.
pub(crate) fn param(name: &str, ty: &Type, ts: Option<String>) -> Option<String> {
    let (wrapper, inner) = wrapper(ty);
    match wrapper.as_deref() {
        Some("Ctx" | "This") => None,
        Some("Opt") => Some(format!(
            "{name}?: {}",
            ts.unwrap_or_else(|| inner.map_or("unknown".to_string(), ts_type))
        )),
        Some("Rest") => Some(format!(
            "...{name}: {}",
            ts.unwrap_or_else(|| array_of(inner.map_or("unknown".to_string(), ts_type)))
        )),
        _ => Some(format!("{name}: {}", ts.unwrap_or_else(|| ts_type(ty)))),
    }
}

/// A path type's last segment name and its first type argument.
fn wrapper(ty: &Type) -> (Option<String>, Option<&Type>) {
    match ty {
        Type::Path(path) if path.qself.is_none() => match path.path.segments.last() {
            Some(segment) => (
                Some(segment.ident.to_string()),
                type_args(&segment.arguments).next(),
            ),
            None => (None, None),
        },
        _ => (None, None),
    }
}

fn type_args(arguments: &PathArguments) -> impl Iterator<Item = &Type> {
    let args = match arguments {
        PathArguments::AngleBracketed(args) => Some(&args.args),
        _ => None,
    };
    args.into_iter().flatten().filter_map(|arg| match arg {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })
}

/// The TS type a script sees for a value of the Rust type `ty`.
pub fn ts_type(ty: &Type) -> String {
    match ty {
        Type::Reference(reference) => ts_type(&reference.elem),
        Type::Paren(paren) => ts_type(&paren.elem),
        Type::Group(group) => ts_type(&group.elem),
        Type::Tuple(tuple) if tuple.elems.is_empty() => "void".to_string(),
        Type::Slice(slice) => array_of(ts_type(&slice.elem)),
        Type::Array(array) => array_of(ts_type(&array.elem)),
        Type::Path(path) if path.qself.is_none() => {
            let Some(segment) = path.path.segments.last() else {
                return "unknown".to_string();
            };
            let name = segment.ident.to_string();
            let mut args = type_args(&segment.arguments);
            match name.as_str() {
                // `Class<'js, JsWorkspace>`: the lifetime is skipped, so the
                // first type argument is the class.
                "Result" | "Box" | "Rc" | "Arc" | "Class" => {
                    args.next().map_or("unknown".to_string(), ts_type)
                }
                "Option" => match args.next() {
                    Some(inner) => format!("{} | null", ts_type(inner)),
                    None => "unknown".to_string(),
                },
                "Vec" | "VecDeque" => array_of(args.next().map_or("unknown".to_string(), ts_type)),
                // cohesion-js's `OneOrMore`: a single value or an array of them.
                "OneOrMore" => {
                    let element = args.next().map_or("unknown".to_string(), ts_type);
                    format!("{element} | {}", array_of(element.clone()))
                }
                "HashMap" | "BTreeMap" => {
                    let key = args.next().map_or("string".to_string(), ts_type);
                    let value = args.next().map_or("unknown".to_string(), ts_type);
                    // A key from a closed set, such as a `TextEnum`, need not
                    // all be present.
                    if key == "string" {
                        format!("Record<string, {value}>")
                    } else {
                        format!("Partial<Record<{key}, {value}>>")
                    }
                }
                "String" | "str" | "char" => "string".to_string(),
                "bool" => "boolean".to_string(),
                "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64" | "usize"
                | "f32" | "f64" => "number".to_string(),
                "Value" => "unknown".to_string(),
                "Object" => "object".to_string(),
                "Function" => "(...args: unknown[]) => unknown".to_string(),
                _ => match name.strip_prefix("Js") {
                    // The `JsWorkspace` convention: the class scripts see is `Workspace`.
                    Some(rest) if rest.starts_with(char::is_uppercase) => rest.to_string(),
                    _ => name,
                },
            }
        }
        _ => "unknown".to_string(),
    }
}

fn array_of(element: String) -> String {
    if element.contains(' ') {
        format!("({element})[]")
    } else {
        format!("{element}[]")
    }
}
