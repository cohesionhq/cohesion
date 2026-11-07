//! `#[derive(Fields)]`: a hidden macro that hands a struct's fields to
//! other macros. `#[fields(skip)]` leaves a field out, and
//! `#[fields(rename = "jsName")]` names its script getter.

use quote::quote;
use syn::{Attribute, DeriveInput, Ident, LitStr};

use crate::derive_input::named_fields;

pub(crate) fn fields(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let mut entries = Vec::new();
    for field in named_fields(&input, "Fields")? {
        let mut skip = false;
        let mut rename = None;
        for attr in field.attrs.iter().filter(|a| a.path().is_ident("fields")) {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("skip") {
                    skip = true;
                    Ok(())
                } else if meta.path.is_ident("rename") {
                    rename = Some(meta.value()?.parse::<LitStr>()?);
                    Ok(())
                } else {
                    Err(meta.error("expected `skip` or `rename = \"...\"`"))
                }
            })?;
        }
        if skip {
            continue;
        }
        let docs = doc_attrs(&field.attrs);
        let rename = rename.map(|name| quote! { #[rename = #name] });
        let ident = &field.ident;
        let ty = &field.ty;
        entries.push(quote! { #rename #(#docs)* #ident: #ty, });
    }
    let fields_macro = Ident::new(&format!("__cohesion_fields_{name}"), name.span());
    Ok(quote! {
        /// Hands this struct's fields to another macro: calls
        /// `callback! { args... fields { #[rename = "jsName"]? /// docs ident: Type, ... } }`.
        /// How `#[js_entity]` learns the fields of a struct it cannot see.
        #[doc(hidden)]
        #[macro_export]
        macro_rules! #fields_macro {
            ($($callback:ident)::+ { $($args:tt)* }) => {
                $($callback)::+! { $($args)* fields { #(#entries)* } }
            };
        }
    })
}

/// The `#[doc = "..."]` attributes (`///` comments) among `attrs`.
fn doc_attrs(attrs: &[Attribute]) -> Vec<Attribute> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .cloned()
        .collect()
}
