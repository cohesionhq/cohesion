//! `#[derive(TsInterface)]`: declares a serde-read struct as a TypeScript
//! interface.

use quote::quote;
use syn::{Attribute, Data, DataStruct, DeriveInput, Fields, LitStr};

use crate::ts_declaration::{InterfaceField, doc_text, render_interface};

pub(crate) fn ts_interface(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let Data::Struct(DataStruct {
        fields: Fields::Named(fields),
        ..
    }) = &input.data
    else {
        return Err(syn::Error::new_spanned(
            name,
            "#[derive(TsInterface)] needs a struct with named fields",
        ));
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
