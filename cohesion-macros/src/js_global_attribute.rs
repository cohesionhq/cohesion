//! `#[js_global]`: declares a host object's methods as a TypeScript
//! namespace.

use quote::quote;
use syn::LitStr;

use crate::ts_declaration::{doc_text, render_namespace};
use crate::ts_method_members::members;

pub(crate) fn js_global_impl(
    name: LitStr,
    mut item: syn::ItemImpl,
) -> syn::Result<proc_macro2::TokenStream> {
    let members = members(&mut item.items, Vec::new())?;
    let consts = render_namespace(&name.value(), &doc_text(&item.attrs), &members).consts();
    let self_ty = &item.self_ty;
    Ok(quote! {
        #item

        impl ::cohesion_core::ts_documentation::TsDocumentation for #self_ty {
            #consts
        }
    })
}
