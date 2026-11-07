//! `#[derive(FromRequest)]`: builds a database command from its protocol
//! request.

use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Data, DeriveInput, Fields, Ident, Path};

/// The contents of `#[request(path::To::Request, ignore(a, b))]`.
struct RequestAttr {
    path: Path,
    ignored: Vec<Ident>,
}

impl Parse for RequestAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let path: Path = input.parse()?;
        let mut ignored = Vec::new();
        if input.parse::<Option<syn::Token![,]>>()?.is_some() && !input.is_empty() {
            let keyword: Ident = input.parse()?;
            if keyword != "ignore" {
                return Err(syn::Error::new_spanned(keyword, "expected `ignore(...)`"));
            }
            let content;
            syn::parenthesized!(content in input);
            ignored = content
                .parse_terminated(Ident::parse, syn::Token![,])?
                .into_iter()
                .collect();
            input.parse::<Option<syn::Token![,]>>()?;
        }
        Ok(Self { path, ignored })
    }
}

/// Command fields no request carries: metadata filters are for scripts, never
/// the wire, so these always start empty.
const SCRIPT_ONLY: [&str; 2] = ["metadata", "has_metadata"];

pub(crate) fn from_request(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let mut request: Option<RequestAttr> = None;
    for attr in input.attrs.iter().filter(|a| a.path().is_ident("request")) {
        request = Some(attr.parse_args()?);
    }
    let Some(RequestAttr {
        path: request,
        ignored,
    }) = request
    else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "#[derive(FromRequest)] needs #[request(path::To::Request)]",
        ));
    };

    let idents: Vec<&Ident> = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => fields
                .named
                .iter()
                .map(|f| f.ident.as_ref().expect("named fields have identifiers"))
                .collect(),
            Fields::Unit => Vec::new(),
            Fields::Unnamed(_) => {
                return Err(syn::Error::new_spanned(
                    name,
                    "#[derive(FromRequest)] needs a struct with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                name,
                "#[derive(FromRequest)] only supports structs",
            ));
        }
    };
    if let Some(both) = ignored.iter().find(|i| idents.contains(i)) {
        return Err(syn::Error::new_spanned(
            both,
            format!("`{both}` is a field of `{name}`, so it cannot also be ignored"),
        ));
    }
    let (defaulted, taken): (Vec<&Ident>, Vec<&Ident>) = idents
        .iter()
        .partition(|i| SCRIPT_ONLY.iter().any(|s| *i == s));
    let body = match &input.data {
        Data::Struct(data) if matches!(data.fields, Fields::Unit) => quote! { Self },
        _ => quote! {
            Self {
                #(#taken,)*
                #(#defaulted: ::core::default::Default::default(),)*
            }
        },
    };

    // No `..`: every request field must be taken or explicitly ignored, so a
    // request that grows a script-only field fails to compile here.
    Ok(quote! {
        impl ::core::convert::From<#request> for #name {
            fn from(value: #request) -> Self {
                let #request { #(#taken,)* #(#ignored: _,)* } = value;
                #body
            }
        }

        impl ::cohesion_db::DbRequest for #request {
            type Command = #name;
        }
    })
}
