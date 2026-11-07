//! TypeScript declarations for script-facing types, built at expansion time
//! from Rust signatures, for `cohesion_core::ts_documentation::TsDocumentation`:
//! the members a declaration lists, and how each kind is rendered, in full
//! and briefly.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Expr, ExprLit, Lit, Meta, Type};

use crate::ts_type_mapping::{option_inner, ts_type};

/// A property: a model column, or a `#[qjs(get)]`/`#[qjs(set)]` pair.
pub struct Prop {
    pub name: String,
    pub ty: String,
    pub writable: bool,
    pub docs: String,
}

/// A callable member.
pub struct Method {
    pub name: String,
    /// Its type parameters, without the brackets, such as
    /// `K extends keyof ServiceMap`; empty for none.
    pub generics: String,
    pub params: Vec<String>,
    pub returns: String,
    pub docs: String,
    pub is_static: bool,
}

#[derive(Default)]
pub struct Members {
    pub props: Vec<Prop>,
    pub methods: Vec<Method>,
}

/// A declaration rendered twice: with every doc comment whole, and with
/// only each one's first paragraph.
pub struct Declaration {
    pub full: String,
    pub brief: String,
}

impl Declaration {
    fn render(render: impl Fn(Detail) -> String) -> Self {
        Self {
            full: render(Detail::Full),
            brief: render(Detail::Brief),
        }
    }

    /// The `TS` and `TS_BRIEF` items of a `TsDocumentation` impl.
    pub fn consts(&self) -> TokenStream {
        let Self { full, brief } = self;
        quote! {
            const TS: &'static str = #full;
            const TS_BRIEF: &'static str = #brief;
        }
    }
}

/// How much of each doc comment a rendering keeps.
#[derive(Clone, Copy)]
pub enum Detail {
    Full,
    /// The first paragraph, which says what the item is; the rest is
    /// detail a reader looks up when they need it.
    Brief,
}

impl Detail {
    /// `text` as this detail keeps it.
    pub fn of(self, text: &str) -> &str {
        match self {
            Self::Full => text,
            Self::Brief => text.split("\n\n").next().unwrap_or_default(),
        }
    }

    /// `text`, as this detail keeps it, as a JSDoc comment at `indent`.
    pub fn jsdoc(self, text: &str, indent: &str) -> String {
        jsdoc(self.of(text), indent)
    }
}

/// `declare class Name { ... }`: a global, as `define` mounts it, so the
/// `cohesion` namespace completes only to the API itself.
pub fn render_class(name: &str, docs: &str, members: &Members) -> Declaration {
    Declaration::render(|detail| class(name, docs, members, detail))
}

fn class(name: &str, docs: &str, members: &Members, detail: Detail) -> String {
    let mut out = detail.jsdoc(docs, "");
    out += &format!("declare class {name} {{\n  private constructor();\n");
    for prop in &members.props {
        out += &detail.jsdoc(&prop.docs, "  ");
        let readonly = if prop.writable { "" } else { "readonly " };
        out += &format!("  {readonly}{}: {};\n", property_name(&prop.name), prop.ty);
    }
    for method in &members.methods {
        out += &detail.jsdoc(&method.docs, "  ");
        let is_static = if method.is_static { "static " } else { "" };
        out += &format!("  {is_static}{};\n", signature(method));
    }
    out + "}"
}

/// `declare namespace name { function ...; const ...; }` for a host object.
pub fn render_namespace(name: &str, docs: &str, members: &Members) -> Declaration {
    Declaration::render(|detail| namespace(name, docs, members, detail))
}

fn namespace(name: &str, docs: &str, members: &Members, detail: Detail) -> String {
    let mut out = detail.jsdoc(docs, "");
    out += &format!("declare namespace {name} {{\n");
    for prop in &members.props {
        out += &detail.jsdoc(&prop.docs, "  ");
        let keyword = if prop.writable { "let" } else { "const" };
        out += &format!("  {keyword} {}: {};\n", prop.name, prop.ty);
    }
    for method in &members.methods {
        out += &detail.jsdoc(&method.docs, "  ");
        out += &format!("  function {};\n", signature(method));
    }
    out + "}"
}

/// A field of a `#[derive(JsOptions)]` or `#[derive(TsInterface)]` struct.
pub struct InterfaceField<'a> {
    pub name: String,
    pub ty: &'a Type,
    pub docs: String,
}

/// `interface Name { ... }` for an options object; `Option` fields are
/// optional members of their inner type.
pub fn render_interface(name: &str, docs: &str, fields: &[InterfaceField]) -> Declaration {
    Declaration::render(|detail| interface(name, docs, fields, detail))
}

fn interface(name: &str, docs: &str, fields: &[InterfaceField], detail: Detail) -> String {
    let mut out = detail.jsdoc(docs, "");
    out += &format!("interface {name} {{\n");
    for field in fields {
        out += &detail.jsdoc(&field.docs, "  ");
        let name = property_name(&field.name);
        out += &match option_inner(field.ty) {
            Some(inner) => format!("  {name}?: {};\n", ts_type(inner)),
            None => format!("  {name}: {};\n", ts_type(field.ty)),
        };
    }
    out + "}"
}

/// `type Name = ({ tag: "text" } & Struct) | ...` for a tagged options enum,
/// one member per `(text, struct)` variant.
pub fn render_union(
    name: &str,
    docs: &str,
    tag: &str,
    variants: &[(String, &Type)],
) -> Declaration {
    Declaration::render(|detail| union(name, docs, tag, variants, detail))
}

fn union(
    name: &str,
    docs: &str,
    tag: &str,
    variants: &[(String, &Type)],
    detail: Detail,
) -> String {
    let mut out = detail.jsdoc(docs, "");
    out += &format!("type {name} =");
    for (text, ty) in variants {
        out += &format!(
            "\n  | ({{ {}: {text:?} }} & {})",
            property_name(tag),
            ts_type(ty)
        );
    }
    out + ";"
}

fn signature(method: &Method) -> String {
    let generics = match method.generics.as_str() {
        "" => String::new(),
        generics => format!("<{generics}>"),
    };
    format!(
        "{}{generics}({}): {}",
        property_name(&method.name),
        method.params.join(", "),
        method.returns
    )
}

/// A name as a TS member key: quoted unless it is a plain identifier.
fn property_name(name: &str) -> String {
    let plain = name
        .chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$')
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '$');
    if plain {
        name.to_string()
    } else {
        format!("{name:?}")
    }
}

/// The text of an item's `///` comments.
pub fn doc_text(attrs: &[Attribute]) -> String {
    let lines: Vec<String> = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .filter_map(|attr| match &attr.meta {
            Meta::NameValue(meta) => match &meta.value {
                Expr::Lit(ExprLit {
                    lit: Lit::Str(text),
                    ..
                }) => Some(text.value()),
                _ => None,
            },
            _ => None,
        })
        .map(|line| line.strip_prefix(' ').map(str::to_string).unwrap_or(line))
        .collect();
    lines.join("\n").trim().to_string()
}

/// `text` as a JSDoc comment at `indent`, or nothing for no text.
pub fn jsdoc(text: &str, indent: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let text = text.replace("*/", "*\\/");
    if !text.contains('\n') {
        return format!("{indent}/** {text} */\n");
    }
    let mut out = format!("{indent}/**\n");
    for line in text.lines() {
        out += format!("{indent} * {line}").trim_end();
        out += "\n";
    }
    out + &format!("{indent} */\n")
}
