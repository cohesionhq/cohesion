//! `#[derive(TsInterface)]`: declares a serde-read struct as a TypeScript
//! interface, or a tagged enum of them as a union.

use quote::quote;
use syn::{Attribute, Data, DataEnum, DataStruct, DeriveInput, Fields, LitStr};

use crate::identifier_case::camel_case;
use crate::ts_declaration::{InterfaceField, doc_text, render_interface, render_union};

pub(crate) fn ts_interface(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let fields = match &input.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => fields,
        Data::Enum(data) => return ts_union(&input, data),
        _ => {
            return Err(syn::Error::new_spanned(
                name,
                "#[derive(TsInterface)] needs a struct with named fields or a tagged enum",
            ));
        }
    };
    let ts_fields = fields
        .named
        .iter()
        .map(|field| {
            Ok(InterfaceField {
                name: match serde_alias(&field.attrs)? {
                    Some(alias) => alias.value(),
                    None => field
                        .ident
                        .as_ref()
                        .expect("named fields have names")
                        .to_string(),
                },
                ty: &field.ty,
                docs: doc_text(&field.attrs),
            })
        })
        .collect::<syn::Result<Vec<_>>>()?;
    let consts = render_interface(&name.to_string(), &doc_text(&input.attrs), &ts_fields).consts();
    Ok(quote! {
        impl ::cohesion_core::ts_documentation::TsDocumentation for #name {
            #consts
        }
    })
}

/// `type Name = ({ tag: "variant" } & Struct) | ...` for an enum serde reads
/// with `tag` and `rename_all = "camelCase"`, each variant wrapping one
/// struct.
fn ts_union(input: &DeriveInput, data: &DataEnum) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let (tag, camel_case_variants) = serde_tag(&input.attrs)?;
    let tag = tag.ok_or_else(|| {
        syn::Error::new_spanned(name, "a TsInterface enum needs `#[serde(tag = \"...\")]`")
    })?;
    if !camel_case_variants {
        return Err(syn::Error::new_spanned(
            name,
            "a TsInterface enum needs `#[serde(rename_all = \"camelCase\")]`",
        ));
    }
    let variants = data
        .variants
        .iter()
        .map(|variant| match &variant.fields {
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => Ok((
                camel_case(&variant.ident.to_string()),
                &fields.unnamed[0].ty,
            )),
            _ => Err(syn::Error::new_spanned(
                variant,
                "a TsInterface enum variant wraps one struct: `Text(TextBlock)`",
            )),
        })
        .collect::<syn::Result<Vec<_>>>()?;
    let consts = render_union(
        &name.to_string(),
        &doc_text(&input.attrs),
        &tag.value(),
        &variants,
    )
    .consts();
    Ok(quote! {
        impl ::cohesion_core::ts_documentation::TsDocumentation for #name {
            #consts
        }
    })
}

/// An enum's `#[serde(tag = "...")]`, and whether it renames its variants
/// `camelCase`.
fn serde_tag(attrs: &[Attribute]) -> syn::Result<(Option<LitStr>, bool)> {
    let mut tag = None;
    let mut camel_case = false;
    for attr in attrs.iter().filter(|a| a.path().is_ident("serde")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("tag") {
                tag = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("rename_all") {
                camel_case = meta.value()?.parse::<LitStr>()?.value() == "camelCase";
            } else if meta.input.peek(syn::Token![=]) {
                meta.value()?.parse::<syn::Expr>()?;
            }
            Ok(())
        })?;
    }
    Ok((tag, camel_case))
}

/// A field's `#[serde(alias = "...")]` or `#[serde(rename = "...")]`, the key
/// a script writes for it. Other serde attributes are left to serde.
fn serde_alias(attrs: &[Attribute]) -> syn::Result<Option<LitStr>> {
    let mut alias = None;
    for attr in attrs.iter().filter(|a| a.path().is_ident("serde")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("alias") || meta.path.is_ident("rename") {
                alias = Some(meta.value()?.parse()?);
            } else if meta.input.peek(syn::Token![=]) {
                meta.value()?.parse::<syn::Expr>()?;
            }
            Ok(())
        })?;
    }
    Ok(alias)
}
