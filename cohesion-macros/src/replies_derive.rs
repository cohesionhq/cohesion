//! `#[derive(Replies)]`: the glue of the protocol's `ReplyData` enum.

use quote::quote;
use syn::{Data, DeriveInput, Fields, GenericArgument, PathArguments, Type};

/// `T` when `ty` is written `Box<T>`.
fn boxed(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else { return None };
    let last = path.path.segments.last()?;
    if last.ident != "Box" {
        return None;
    }
    let PathArguments::AngleBracketed(arguments) = &last.arguments else {
        return None;
    };
    match arguments.args.first()? {
        GenericArgument::Type(inner) => Some(inner),
        _ => None,
    }
}

pub(crate) fn replies(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let Data::Enum(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            name,
            "#[derive(Replies)] only supports enums",
        ));
    };

    let mut seen: Vec<String> = Vec::new();
    let mut impls = Vec::new();
    for variant in &data.variants {
        let ident = &variant.ident;
        let ty = match &variant.fields {
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => &fields.unnamed[0].ty,
            _ => {
                return Err(syn::Error::new_spanned(
                    variant,
                    "#[derive(Replies)] variants must be tuple variants with one field (the payload)",
                ));
            }
        };
        // A boxed payload converts from and to what it boxes, so a large
        // model can be boxed without its commands knowing.
        let (ty, wrap, unwrap) = match boxed(ty) {
            Some(inner) => (
                inner,
                quote! { ::std::boxed::Box::new(value) },
                quote! { *value },
            ),
            None => (ty, quote! { value }, quote! { value }),
        };
        let key = quote! { #ty }.to_string();
        if seen.contains(&key) {
            return Err(syn::Error::new_spanned(
                variant,
                format!(
                    "`{}` already has a variant; each payload type maps back to one variant",
                    key.replace(' ', "")
                ),
            ));
        }
        seen.push(key);
        impls.push(quote! {
            impl ::core::convert::From<#ty> for #name {
                fn from(value: #ty) -> Self {
                    Self::#ident(#wrap)
                }
            }

            impl ::core::convert::TryFrom<#name> for #ty {
                type Error = #name;

                fn try_from(reply: #name) -> ::core::result::Result<Self, #name> {
                    match reply {
                        #name::#ident(value) => ::core::result::Result::Ok(#unwrap),
                        #[allow(unreachable_patterns)]
                        other => ::core::result::Result::Err(other),
                    }
                }
            }

            impl ::cohesion_core::protocol::ReplyPayload for #ty {}
        });
    }

    Ok(quote! { #(#impls)* })
}
