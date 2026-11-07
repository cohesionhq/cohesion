//! `#[derive(JsOptions)]`: reads a script's options object into a struct or
//! tagged enum, and declares its TypeScript shape.

use quote::quote;
use syn::{Attribute, Data, DataStruct, DeriveInput, Fields, Ident, LitStr};

use crate::identifier_case::{camel_case, js_field_name};
use crate::ts_declaration::{
    Declaration, InterfaceField, doc_text, render_interface, render_union,
};
use crate::ts_type_mapping::option_inner;

pub(crate) fn js_options(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "#[derive(JsOptions)] types cannot be generic",
        ));
    }
    let (body, declaration) = match &input.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => {
            if let Some(tag) = js_options_attr(&input.attrs, "tag")? {
                return Err(syn::Error::new_spanned(tag, "`tag` goes on an enum"));
            }
            js_options_struct(name, &input.attrs, fields)?
        }
        Data::Enum(data) => {
            let tag = js_options_attr(&input.attrs, "tag")?.ok_or_else(|| {
                syn::Error::new_spanned(
                    name,
                    "an options enum needs `#[js_options(tag = \"kind\")]`",
                )
            })?;
            js_options_enum(name, &input.attrs, &tag.value(), data)?
        }
        _ => {
            return Err(syn::Error::new_spanned(
                name,
                "#[derive(JsOptions)] needs a struct with named fields or an enum",
            ));
        }
    };
    let consts = declaration.consts();

    Ok(quote! {
        impl<'js> ::rquickjs::FromJs<'js> for #name {
            fn from_js(
                ctx: &::rquickjs::Ctx<'js>,
                value: ::rquickjs::Value<'js>,
            ) -> ::rquickjs::Result<Self> {
                let object = if value.is_undefined() || value.is_null() {
                    ::rquickjs::Object::new(ctx.clone())?
                } else {
                    ::rquickjs::Object::from_js(ctx, value)?
                };
                #body
            }
        }

        impl ::cohesion_core::ts_documentation::TsDocumentation for #name {
            #consts
        }
    })
}

/// The body of a struct's `from_js`, reading each field from `object`, and
/// its `interface` declaration.
fn js_options_struct(
    name: &Ident,
    attrs: &[Attribute],
    fields: &syn::FieldsNamed,
) -> syn::Result<(proc_macro2::TokenStream, Declaration)> {
    let mut reads = Vec::new();
    let mut ts_fields = Vec::new();
    for field in &fields.named {
        let ident = field.ident.as_ref().expect("named fields have names");
        let key = match js_options_attr(&field.attrs, "rename")? {
            Some(rename) => rename.value(),
            None => js_field_name(&ident.to_string()),
        };
        let optional = option_inner(&field.ty).is_some();
        let read = if optional {
            quote! { crate::conversion::FromJsField::from_js_field(ctx, object.get(#key)?)? }
        } else {
            let message = format!("missing required option `{key}`");
            quote! {{
                let value: ::rquickjs::Value<'js> = object.get(#key)?;
                if value.is_undefined() {
                    return ::core::result::Result::Err(
                        ::rquickjs::Exception::throw_type(ctx, #message)
                    );
                }
                crate::conversion::FromJsField::from_js_field(ctx, value)?
            }}
        };
        reads.push(quote! { #ident: #read });
        ts_fields.push(InterfaceField {
            name: key,
            ty: &field.ty,
            docs: doc_text(&field.attrs),
        });
    }
    let ts = render_interface(&name.to_string(), &doc_text(attrs), &ts_fields);
    Ok((
        quote! { ::core::result::Result::Ok(Self { #(#reads,)* }) },
        ts,
    ))
}

/// The body of an enum's `from_js`, which picks the variant named by the
/// `tag` property and reads its struct from the same object, and its union
/// type declaration.
fn js_options_enum(
    name: &Ident,
    attrs: &[Attribute],
    tag: &str,
    data: &syn::DataEnum,
) -> syn::Result<(proc_macro2::TokenStream, Declaration)> {
    let mut arms = Vec::new();
    let mut ts_variants = Vec::new();
    for variant in &data.variants {
        let Fields::Unnamed(fields) = &variant.fields else {
            return Err(syn::Error::new_spanned(
                variant,
                "an options enum variant wraps one `#[derive(JsOptions)]` struct: `Custom(CustomOptions)`",
            ));
        };
        if fields.unnamed.len() != 1 {
            return Err(syn::Error::new_spanned(
                fields,
                "an options enum variant wraps exactly one struct",
            ));
        }
        let ident = &variant.ident;
        let text = camel_case(&ident.to_string());
        arms.push(quote! {
            #text => ::core::result::Result::Ok(Self::#ident(
                ::rquickjs::FromJs::from_js(ctx, object.into_value())?
            )),
        });
        ts_variants.push((text, &fields.unnamed[0].ty));
    }

    let missing = format!("missing required option `{tag}`");
    let expected = ts_variants
        .iter()
        .map(|(text, _)| format!("{text:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    let unknown = format!("unknown `{tag}` {{other:?}}; expected one of {expected}");
    let body = quote! {
        let value: ::rquickjs::Value<'js> = object.get(#tag)?;
        if value.is_undefined() {
            return ::core::result::Result::Err(::rquickjs::Exception::throw_type(ctx, #missing));
        }
        match <::std::string::String as ::rquickjs::FromJs>::from_js(ctx, value)?.as_str() {
            #(#arms)*
            other => ::core::result::Result::Err(
                ::rquickjs::Exception::throw_type(ctx, &::std::format!(#unknown))
            ),
        }
    };
    let ts = render_union(&name.to_string(), &doc_text(attrs), tag, &ts_variants);
    Ok((body, ts))
}

/// The string value of `key` in an item's `#[js_options(...)]`, which takes
/// `tag = "..."` on an enum and `rename = "..."` on a struct field.
fn js_options_attr(attrs: &[Attribute], key: &str) -> syn::Result<Option<LitStr>> {
    let mut found = None;
    for attr in attrs.iter().filter(|a| a.path().is_ident("js_options")) {
        attr.parse_nested_meta(|meta| {
            let value: LitStr = meta.value()?.parse()?;
            if meta.path.is_ident(key) {
                found = Some(value);
                Ok(())
            } else if meta.path.is_ident("tag") || meta.path.is_ident("rename") {
                Err(meta.error(format!("expected `{key} = \"...\"` here")))
            } else {
                Err(meta.error("expected `tag = \"...\"` or `rename = \"...\"`"))
            }
        })?;
    }
    Ok(found)
}
