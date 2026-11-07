//! Helpers for reading a derive's input.

use syn::{Data, DeriveInput, Fields};

pub(crate) fn named_fields<'a>(
    input: &'a DeriveInput,
    derive: &str,
) -> syn::Result<impl Iterator<Item = &'a syn::Field>> {
    let error = || {
        syn::Error::new_spanned(
            &input.ident,
            format!("#[derive({derive})] needs a struct with named fields"),
        )
    };
    match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => Ok(fields.named.iter()),
            _ => Err(error()),
        },
        _ => Err(error()),
    }
}
