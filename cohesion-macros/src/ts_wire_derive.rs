//! `#[derive(TsWire)]`: declares a serde type as TypeScript, following the
//! serde attributes that shape its JSON.
//!
//! Only the serde attributes the protocol uses are modelled; any other is a
//! compile error, so the declaration can't quietly drift from the JSON.

use quote::quote;
use syn::{
    Attribute, Data, DeriveInput, Fields, FieldsNamed, Ident, LitStr, Type, meta::ParseNestedMeta,
};

use crate::identifier_case::camel_case;
use crate::ts_declaration::{doc_text, jsdoc};

/// A declaration being assembled: literal TypeScript and the types it names,
/// whose spelling comes from their own `TsWire` impls at run time.
#[derive(Default)]
struct Ts {
    pieces: Vec<Piece>,
    referenced: Vec<Type>,
}

enum Piece {
    Text(String),
    Type(Type),
}

impl Ts {
    fn text(&mut self, text: impl Into<String>) {
        self.pieces.push(Piece::Text(text.into()));
    }

    fn ty(&mut self, ty: &Type) {
        self.pieces.push(Piece::Type(ty.clone()));
        self.referenced.push(ty.clone());
    }
}

/// `#[serde(...)]` on the struct or enum.
#[derive(Default)]
struct Container {
    rename_all: Option<String>,
    tag: Option<String>,
    content: Option<String>,
    /// `from`/`into`: the JSON is another type's, which serde doesn't show.
    converted: bool,
}

/// `#[ts_wire(...)]` on the struct or enum.
#[derive(Default)]
struct Options {
    /// The whole declaration's right-hand side, for types serde converts.
    ts_type: Option<String>,
    /// Read from clients: `Option` fields may be left out.
    input: bool,
}

pub(crate) fn ts_wire(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let options = options(&input.attrs)?;
    let mut ts = Ts::default();
    ts.text(jsdoc(&doc_text(&input.attrs), ""));
    if let Some(ts_type) = &options.ts_type {
        ts.text(format!("export type {name} = {ts_type};"));
    } else {
        let container = container(&input.attrs)?;
        if container.converted {
            return Err(syn::Error::new_spanned(
                name,
                "serde converts this type; give its JSON with `#[ts_wire(type = \"...\")]`",
            ));
        }
        match &input.data {
            Data::Struct(data) => declare_struct(&mut ts, name, &data.fields, options.input)?,
            Data::Enum(data) => {
                ts.text(format!("export type {name} ="));
                match text_enum_texts(&input.attrs, data)? {
                    Some(texts) => {
                        for text in texts {
                            ts.text(format!("\n\t| {text:?}"));
                        }
                    }
                    None => {
                        for variant in &data.variants {
                            ts.text("\n\t| ");
                            declare_variant(&mut ts, &container, variant, options.input)?;
                        }
                    }
                }
                ts.text(";");
            }
            Data::Union(_) => {
                return Err(syn::Error::new_spanned(
                    name,
                    "TsWire doesn't support unions",
                ));
            }
        }
    }

    let key = name.to_string();
    let pieces = ts.pieces.iter().map(|piece| match piece {
        Piece::Text(text) => quote! { ts.push_str(#text); },
        Piece::Type(ty) => {
            quote! { ts.push_str(&<#ty as ::cohesion_core::ts_wire::TsWire>::ts_type()); }
        }
    });
    let referenced = &ts.referenced;
    Ok(quote! {
        impl ::cohesion_core::ts_wire::TsWire for #name {
            fn ts_type() -> ::std::string::String {
                ::std::string::String::from(#key)
            }

            fn declare(out: &mut ::cohesion_core::ts_wire::Declarations) {
                if out.contains_key(#key) {
                    return;
                }
                // Claimed before recursing, so a type that refers back to
                // this one stops here.
                out.insert(#key, ::std::string::String::new());
                #(<#referenced as ::cohesion_core::ts_wire::TsWire>::declare(out);)*
                let mut ts = ::std::string::String::new();
                #(#pieces)*
                out.insert(#key, ts);
            }
        }
    })
}

fn declare_struct(ts: &mut Ts, name: &Ident, fields: &Fields, input: bool) -> syn::Result<()> {
    match fields {
        Fields::Named(fields) if fields.named.is_empty() => {
            ts.text(format!("export type {name} = Record<string, never>;"));
        }
        Fields::Named(fields) => {
            let flattened = flattened(fields)?;
            if flattened.is_empty() {
                ts.text(format!("export interface {name} "));
                object(ts, None, fields, input)?;
            } else {
                ts.text(format!("export type {name} = "));
                object(ts, None, fields, input)?;
                ts.text(";");
            }
        }
        Fields::Unit => ts.text(format!("export type {name} = null;")),
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
            ts.text(format!("export type {name} = "));
            ts.ty(&fields.unnamed[0].ty);
            ts.text(";");
        }
        Fields::Unnamed(_) => {
            return Err(syn::Error::new_spanned(
                name,
                "TsWire supports tuple structs of one field only",
            ));
        }
    }
    Ok(())
}

/// One member of an enum's union, shaped by how serde tags it.
fn declare_variant(
    ts: &mut Ts,
    container: &Container,
    variant: &syn::Variant,
    input: bool,
) -> syn::Result<()> {
    let mut rename = None;
    let mut untagged = false;
    serde_attrs(&variant.attrs, |meta| {
        if meta.path.is_ident("rename") {
            rename = Some(meta.value()?.parse::<LitStr>()?.value());
        } else if meta.path.is_ident("untagged") {
            untagged = true;
        } else {
            return Err(unsupported(&meta));
        }
        Ok(())
    })?;
    let text = match (rename, container.rename_all.as_deref()) {
        (Some(text), _) => text,
        (None, Some(_)) => camel_case(&variant.ident.to_string()),
        (None, None) => variant.ident.to_string(),
    };
    let fields = &variant.fields;
    let newtype = match fields {
        Fields::Unnamed(unnamed) if unnamed.unnamed.len() == 1 => Some(&unnamed.unnamed[0].ty),
        Fields::Unnamed(_) => {
            return Err(syn::Error::new_spanned(
                variant,
                "TsWire supports tuple variants of one field only",
            ));
        }
        _ => None,
    };

    if untagged {
        match (fields, newtype) {
            (Fields::Unit, _) => ts.text("null"),
            (_, Some(ty)) => ts.ty(ty),
            (Fields::Named(named), _) => object(ts, None, named, input)?,
            _ => unreachable!("every variant shape is covered above"),
        }
        return Ok(());
    }
    match (&container.tag, &container.content) {
        (Some(tag), Some(content)) => {
            let tag = format!("{tag}: {text:?}");
            match (fields, newtype) {
                (Fields::Unit, _) => ts.text(format!("{{ {tag} }}")),
                (_, Some(ty)) => {
                    ts.text(format!("{{ {tag}; {content}: "));
                    ts.ty(ty);
                    ts.text(" }");
                }
                (Fields::Named(named), _) => {
                    ts.text(format!("{{ {tag}; {content}: "));
                    object(ts, None, named, input)?;
                    ts.text(" }");
                }
                _ => unreachable!("every variant shape is covered above"),
            }
        }
        (Some(tag), None) => {
            let tag = format!("{tag}: {text:?}");
            match (fields, newtype) {
                (Fields::Unit, _) => ts.text(format!("{{ {tag} }}")),
                (_, Some(ty)) => {
                    ts.text(format!("({{ {tag} }} & "));
                    ts.ty(ty);
                    ts.text(")");
                }
                (Fields::Named(named), _) => object(ts, Some(&tag), named, input)?,
                _ => unreachable!("every variant shape is covered above"),
            }
        }
        (None, Some(_)) => {
            return Err(syn::Error::new_spanned(
                variant,
                "serde's `content` needs a `tag`",
            ));
        }
        (None, None) => match (fields, newtype) {
            (Fields::Unit, _) => ts.text(format!("{text:?}")),
            (_, Some(ty)) => {
                ts.text(format!("{{ {text:?}: "));
                ts.ty(ty);
                ts.text(" }");
            }
            (Fields::Named(named), _) => {
                ts.text(format!("{{ {text:?}: "));
                object(ts, None, named, input)?;
                ts.text(" }");
            }
            _ => unreachable!("every variant shape is covered above"),
        },
    }
    Ok(())
}

/// `{ ...members }`, starting with `tag` when given, then `& Type` for each
/// flattened field.
fn object(ts: &mut Ts, tag: Option<&str>, fields: &FieldsNamed, input: bool) -> syn::Result<()> {
    ts.text("{\n");
    if let Some(tag) = tag {
        ts.text(format!("\t{tag};\n"));
    }
    let mut flattened = Vec::new();
    for field in &fields.named {
        let attrs = field_attrs(&field.attrs)?;
        if attrs.skip {
            continue;
        }
        if attrs.flatten {
            flattened.push(&field.ty);
            continue;
        }
        let name = match attrs.rename {
            Some(rename) => rename,
            None => field
                .ident
                .as_ref()
                .expect("named fields have names")
                .to_string(),
        };
        let optional =
            attrs.optional || (input && crate::ts_type_mapping::option_inner(&field.ty).is_some());
        ts.text(jsdoc(&doc_text(&field.attrs), "\t"));
        ts.text(format!("\t{name}{}: ", if optional { "?" } else { "" }));
        ts.ty(&field.ty);
        ts.text(";\n");
    }
    ts.text("}");
    for ty in flattened {
        ts.text(" & ");
        ts.ty(ty);
    }
    Ok(())
}

fn flattened(fields: &FieldsNamed) -> syn::Result<Vec<&Type>> {
    let mut out = Vec::new();
    for field in &fields.named {
        if field_attrs(&field.attrs)?.flatten {
            out.push(&field.ty);
        }
    }
    Ok(out)
}

/// `#[serde(...)]` on a field.
#[derive(Default)]
struct FieldAttrs {
    flatten: bool,
    /// `skip`: serde neither writes nor reads the field, so the wire has no key.
    skip: bool,
    /// `default` or `skip_serializing_if`: the key may be absent.
    optional: bool,
    /// `rename`: the key serde writes instead of the field's name.
    rename: Option<String>,
}

fn field_attrs(attrs: &[Attribute]) -> syn::Result<FieldAttrs> {
    let mut out = FieldAttrs::default();
    serde_attrs(attrs, |meta| {
        if meta.path.is_ident("flatten") {
            out.flatten = true;
        } else if meta.path.is_ident("skip") {
            out.skip = true;
        } else if meta.path.is_ident("default") {
            if meta.input.peek(syn::Token![=]) {
                meta.value()?.parse::<LitStr>()?;
            }
            out.optional = true;
        } else if meta.path.is_ident("skip_serializing_if") {
            meta.value()?.parse::<LitStr>()?;
            out.optional = true;
        } else if meta.path.is_ident("rename") {
            out.rename = Some(meta.value()?.parse::<LitStr>()?.value());
        } else if meta.path.is_ident("alias") {
            // Another key serde reads; the wire still writes the field's own.
            meta.value()?.parse::<LitStr>()?;
        } else {
            return Err(unsupported(&meta));
        }
        Ok(())
    })?;
    Ok(out)
}

fn container(attrs: &[Attribute]) -> syn::Result<Container> {
    let mut out = Container::default();
    serde_attrs(attrs, |meta| {
        if meta.path.is_ident("rename_all") {
            let rule: LitStr = meta.value()?.parse()?;
            if rule.value() != "camelCase" {
                return Err(syn::Error::new_spanned(
                    rule,
                    "TsWire supports `rename_all = \"camelCase\"` only",
                ));
            }
            out.rename_all = Some(rule.value());
        } else if meta.path.is_ident("tag") {
            out.tag = Some(meta.value()?.parse::<LitStr>()?.value());
        } else if meta.path.is_ident("content") {
            out.content = Some(meta.value()?.parse::<LitStr>()?.value());
        } else if meta.path.is_ident("transparent") {
            // A newtype is its field on the wire either way.
        } else if meta.path.is_ident("from") || meta.path.is_ident("into") {
            meta.value()?.parse::<LitStr>()?;
            out.converted = true;
        } else {
            return Err(unsupported(&meta));
        }
        Ok(())
    })?;
    Ok(out)
}

fn options(attrs: &[Attribute]) -> syn::Result<Options> {
    let mut out = Options::default();
    for attr in attrs.iter().filter(|a| a.path().is_ident("ts_wire")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("type") {
                out.ts_type = Some(meta.value()?.parse::<LitStr>()?.value());
            } else if meta.path.is_ident("input") {
                out.input = true;
            } else {
                return Err(meta.error("expected `type = \"...\"` or `input`"));
            }
            Ok(())
        })?;
    }
    Ok(out)
}

/// Each variant's `#[text_enum("...")]` when `#[derive(TextEnum)]` with
/// `rename_all = "custom"` implements serde, and so decides the JSON.
fn text_enum_texts(attrs: &[Attribute], data: &syn::DataEnum) -> syn::Result<Option<Vec<String>>> {
    let mut custom = false;
    for attr in attrs.iter().filter(|a| a.path().is_ident("text_enum")) {
        attr.parse_nested_meta(|meta| {
            let value: syn::Expr = meta.value()?.parse()?;
            if meta.path.is_ident("rename_all") {
                custom = matches!(
                    value,
                    syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(rule), .. }) if rule.value() == "custom"
                );
            }
            Ok(())
        })?;
    }
    if !custom {
        return Ok(None);
    }
    data.variants
        .iter()
        .map(|variant| {
            let attr = variant
                .attrs
                .iter()
                .find(|a| a.path().is_ident("text_enum"))
                .ok_or_else(|| {
                    syn::Error::new_spanned(variant, "missing `#[text_enum(\"...\")]`")
                })?;
            Ok(attr.parse_args::<LitStr>()?.value())
        })
        .collect::<syn::Result<_>>()
        .map(Some)
}

fn serde_attrs(
    attrs: &[Attribute],
    mut each: impl FnMut(ParseNestedMeta) -> syn::Result<()>,
) -> syn::Result<()> {
    for attr in attrs.iter().filter(|a| a.path().is_ident("serde")) {
        attr.parse_nested_meta(&mut each)?;
    }
    Ok(())
}

fn unsupported(meta: &ParseNestedMeta) -> syn::Error {
    meta.error("TsWire doesn't model this serde attribute; teach it in ts_wire_derive.rs")
}
