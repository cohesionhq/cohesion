//! `#[derive(Commands)]`: the glue of the protocol's `Command` enum.

use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Data, DeriveInput, Fields, Ident, Path, Type};

use crate::identifier_case::camel_case;
use crate::ts_declaration::{doc_text, jsdoc};

/// The contents of `#[reply(Type, via = Path, request = Path)]`.
struct ReplyAttr {
    reply: Type,
    via: Option<Path>,
    request: Option<Path>,
}

impl Parse for ReplyAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let reply: Type = input.parse()?;
        let mut via = None;
        let mut request = None;
        while input.parse::<Option<syn::Token![,]>>()?.is_some() && !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<syn::Token![=]>()?;
            let slot = if key == "via" {
                &mut via
            } else if key == "request" {
                &mut request
            } else {
                return Err(syn::Error::new_spanned(
                    key,
                    "expected `via = Path` or `request = Path`",
                ));
            };
            if slot.is_some() {
                return Err(syn::Error::new_spanned(
                    &key,
                    format!("`{key}` is given twice"),
                ));
            }
            *slot = Some(input.parse()?);
        }
        if !input.is_empty() {
            return Err(input.error("expected `,`"));
        }
        Ok(Self {
            reply,
            via,
            request,
        })
    }
}

pub(crate) fn commands(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let Data::Enum(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            name,
            "#[derive(Commands)] only supports enums",
        ));
    };

    let mut impls = Vec::new();
    let mut bounds = Vec::new();
    let mut arms = Vec::new();
    let mut ts_entries = Vec::new();
    for variant in &data.variants {
        let ident = &variant.ident;
        let ts_key = format!(
            "{}\t{}: ",
            jsdoc(&doc_text(&variant.attrs), "\t"),
            camel_case(&ident.to_string())
        );
        let mut attr: Option<ReplyAttr> = None;
        for a in variant.attrs.iter().filter(|a| a.path().is_ident("reply")) {
            if attr.is_some() {
                return Err(syn::Error::new_spanned(
                    a,
                    "only one `#[reply(...)]` per variant",
                ));
            }
            attr = Some(a.parse_args()?);
        }
        let Some(ReplyAttr {
            reply,
            via,
            request: named_request,
        }) = attr
        else {
            return Err(syn::Error::new_spanned(
                variant,
                "missing `#[reply(ReplyType)]`: every command names the payload of its reply",
            ));
        };
        let kind = match via {
            Some(via) => quote! { #via },
            None => quote! { ::cohesion_core::protocol::Database },
        };
        let (request, from, arm, ts_args) = match &variant.fields {
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                if let Some(named) = named_request {
                    return Err(syn::Error::new_spanned(
                        named,
                        "the request type is the variant's field; remove `request = ...`",
                    ));
                }
                let ty = &fields.unnamed[0].ty;
                let request = quote! { #ty };
                let from = quote! {
                    fn from(request: #request) -> Self {
                        Self::#ident(request)
                    }
                };
                let arm = quote! {
                    Self::#ident(request) => {
                        <Ctx as ::cohesion_core::protocol::Route<#kind, #request>>::route(ctx, request)
                            .await
                            .map(::cohesion_core::protocol::ReplyData::from)
                    }
                };
                let ts_args = quote! {{
                    <#request as ::cohesion_core::ts_wire::TsWire>::declare(out);
                    <#request as ::cohesion_core::ts_wire::TsWire>::ts_type()
                }};
                (request, from, arm, ts_args)
            }
            Fields::Unit => {
                let Some(named) = named_request else {
                    return Err(syn::Error::new_spanned(
                        variant,
                        "a unit variant needs `request = ...`: add it to `#[reply(...)]`, naming a unit struct",
                    ));
                };
                let request = quote! { #named };
                let from = quote! {
                    fn from(_: #request) -> Self {
                        Self::#ident
                    }
                };
                let arm = quote! {
                    Self::#ident => {
                        <Ctx as ::cohesion_core::protocol::Route<#kind, #request>>::route(ctx, #named)
                            .await
                            .map(::cohesion_core::protocol::ReplyData::from)
                    }
                };
                // A unit variant's `args` key is absent, whatever its struct.
                (
                    request,
                    from,
                    arm,
                    quote! { ::std::string::String::from("NoArgs") },
                )
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    variant,
                    "#[derive(Commands)] variants must be a tuple variant with one field (the request) or a unit variant",
                ));
            }
        };
        impls.push(quote! {
            impl ::cohesion_core::protocol::Call for #request {
                type Reply = #reply;
            }

            impl ::core::convert::From<#request> for #name {
                #from
            }
        });
        bounds.push(quote! { Ctx: ::cohesion_core::protocol::Route<#kind, #request> });
        arms.push(arm);
        ts_entries.push(quote! {
            <#reply as ::cohesion_core::ts_wire::TsWire>::declare(out);
            requests.push_str(#ts_key);
            requests.push_str(&#ts_args);
            requests.push_str(";\n");
            replies.push_str(#ts_key);
            replies.push_str(&<#reply as ::cohesion_core::ts_wire::TsWire>::ts_type());
            replies.push_str(";\n");
        });
    }

    Ok(quote! {
        #(#impls)*

        impl #name {
            /// Adds every request and reply type to `out`, and returns the
            /// TypeScript `Requests` and `Replies` interfaces, which map each
            /// command's `type` to its `args` and its reply's `data`.
            pub fn declare_ts(out: &mut ::cohesion_core::ts_wire::Declarations) -> ::std::string::String {
                let mut requests = ::std::string::String::from(
                    "/** Every request `type` and the shape of its `args`. */\nexport interface Requests {\n",
                );
                let mut replies = ::std::string::String::from(
                    "/** The `ok.data` each request type answers with. */\nexport interface Replies {\n",
                );
                #(#ts_entries)*
                requests + "}\n\n" + &replies + "}\n"
            }

            /// Runs this command with `ctx`'s route for its request type.
            pub async fn dispatch<Ctx: ::core::marker::Sync>(
                self,
                ctx: &Ctx,
            ) -> ::cohesion_core::protocol::Outcome
            where
                #(#bounds,)*
            {
                let result = match self {
                    #(#arms)*
                };
                match result {
                    ::core::result::Result::Ok(data) => ::cohesion_core::protocol::Outcome::Ok(data),
                    ::core::result::Result::Err(error) => {
                        ::cohesion_core::protocol::Outcome::Error(error)
                    }
                }
            }
        }
    })
}
