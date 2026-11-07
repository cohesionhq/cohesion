//! `#[js_entity]`, second half (`__js_entity_expand!`): the script-facing
//! class, its getters, `toJSON` and TypeScript declaration.

use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Attribute, Ident, LitStr, Path, Type};

use crate::identifier_case::js_field_name;
use crate::ts_declaration::{Method, Prop, doc_text, render_class};
use crate::ts_method_members::members;
use crate::ts_type_mapping::ts_type;

/// What a struct's fields macro hands `__js_entity_expand!`: `model(Path)
/// js("Name") impl Class { ... } fields { #[rename = "jsName"]? /// docs ident: Type, ... }`.
pub(crate) struct JsEntityExpand {
    model: Path,
    js_name: LitStr,
    item: syn::ItemImpl,
    fields: Vec<EntityField>,
}

/// One field, as `#[js_entity]` exposes it.
struct EntityField {
    ident: Ident,
    ty: Type,
    name: LitStr,
    docs: Vec<Attribute>,
}

impl Parse for JsEntityExpand {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let parenthesized = |keyword: &str| -> syn::Result<proc_macro2::TokenStream> {
            let found: Ident = input.parse()?;
            if found != keyword {
                return Err(syn::Error::new_spanned(
                    found,
                    format!("expected `{keyword}(...)`"),
                ));
            }
            let content;
            syn::parenthesized!(content in input);
            content.parse()
        };
        let model = syn::parse2(parenthesized("model")?)?;
        let js_name = syn::parse2(parenthesized("js")?)?;
        let item = input.parse()?;
        let keyword: Ident = input.parse()?;
        if keyword != "fields" {
            return Err(syn::Error::new_spanned(
                keyword,
                "expected `fields { ... }`",
            ));
        }
        let content;
        syn::braced!(content in input);
        let mut fields = Vec::new();
        while !content.is_empty() {
            let mut docs = content.call(Attribute::parse_outer)?;
            let mut rename = None;
            if let Some(at) = docs.iter().position(|a| a.path().is_ident("rename")) {
                let attr = docs.remove(at);
                let value = &attr.meta.require_name_value()?.value;
                rename = Some(syn::parse2::<LitStr>(quote! { #value })?);
            }
            let ident: Ident = content.parse()?;
            content.parse::<syn::Token![:]>()?;
            let ty: Type = content.parse()?;
            let name = rename
                .unwrap_or_else(|| LitStr::new(&js_field_name(&ident.to_string()), ident.span()));
            fields.push(EntityField {
                ident,
                ty,
                name,
                docs,
            });
            content.parse::<Option<syn::Token![,]>>()?;
        }
        Ok(Self {
            model,
            js_name,
            item,
            fields,
        })
    }
}

pub(crate) fn js_entity_expand(input: JsEntityExpand) -> syn::Result<proc_macro2::TokenStream> {
    let JsEntityExpand {
        model,
        js_name,
        mut item,
        fields,
    } = input;
    let not_plain = "#[js_entity] needs a plain type name, such as `impl JsWorkspace`";
    let class = match &*item.self_ty {
        Type::Path(path) if path.qself.is_none() => match path.path.get_ident() {
            Some(ident) => ident.clone(),
            None => return Err(syn::Error::new_spanned(path, not_plain)),
        },
        other => return Err(syn::Error::new_spanned(other, not_plain)),
    };
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &item.generics,
            "#[js_entity] classes cannot be generic",
        ));
    }
    let columns = fields
        .iter()
        .map(|field| Prop {
            name: field.name.value(),
            ty: ts_type(&field.ty),
            writable: false,
            docs: doc_text(&field.docs),
        })
        .collect();
    let mut members = members(&mut item.items, columns)?;
    members.methods.insert(
        0,
        Method {
            name: "toJSON".to_string(),
            generics: String::new(),
            params: Vec::new(),
            returns: "object".to_string(),
            docs: "Every property, for `JSON.stringify`.".to_string(),
            is_static: false,
        },
    );
    let consts = render_class(&js_name.value(), &doc_text(&item.attrs), &members).consts();

    let attrs = &item.attrs;
    let methods = &item.items;
    let doc = format!("The `{}` class scripts see.", js_name.value());

    let getters = fields.iter().map(|EntityField { ident, name, .. }| {
        let getter = Ident::new(&format!("__get_{ident}"), ident.span());
        quote! {
            #[qjs(get, enumerable, rename = #name)]
            fn #getter<'js>(&self, ctx: ::rquickjs::Ctx<'js>) -> ::rquickjs::Result<::rquickjs::Value<'js>> {
                crate::conversion::ToJs::to_js(&self.inner.#ident, &ctx)
            }
        }
    });
    let idents = fields.iter().map(|field| &field.ident);
    let names = fields.iter().map(|field| &field.name);

    Ok(quote! {
        #[doc = #doc]
        #[derive(::rquickjs::class::Trace, ::rquickjs::JsLifetime)]
        #[::rquickjs::class(rename = #js_name)]
        pub struct #class {
            #[qjs(skip_trace)]
            inner: #model,
            #[qjs(skip_trace)]
            db: ::cohesion_db::GuardedDatabaseContext,
        }

        impl #class {
            /// Wraps `inner`; its methods run their commands through `db`,
            /// whose access is the sandbox's.
            pub fn new(inner: #model, db: ::cohesion_db::GuardedDatabaseContext) -> Self {
                Self { inner, db }
            }

            /// The wrapped model instance.
            #[allow(dead_code)]
            pub fn inner(&self) -> &#model {
                &self.inner
            }

            /// Runs a read command, turning any failure into a JS exception.
            #[allow(dead_code)]
            fn read<'js, C: ::cohesion_db::DatabaseCommand<Access = ::cohesion_db::Read>>(
                &self,
                ctx: &::rquickjs::Ctx<'js>,
                cmd: C,
            ) -> ::rquickjs::Result<C::Response> {
                crate::traced_command::read(ctx, &self.db, cmd)
            }

            /// Runs a write command, throwing if the sandbox may not write its
            /// area and turning any failure into a JS exception.
            #[allow(dead_code)]
            fn write<'js, A: ::cohesion_db::Area, C: ::cohesion_db::DatabaseCommand<Access = ::cohesion_db::Write<A>>>(
                &self,
                ctx: &::rquickjs::Ctx<'js>,
                cmd: C,
            ) -> ::rquickjs::Result<C::Response> {
                crate::traced_command::write(ctx, &self.db, cmd)
            }
        }

        impl ::cohesion_core::ts_documentation::TsDocumentation for #class {
            #consts
        }

        /// An instance a script passes in an options object, copied out of
        /// its class; anything else throws a `TypeError`.
        impl crate::conversion::FromJsField for #class {
            fn from_js_field<'js>(
                ctx: &::rquickjs::Ctx<'js>,
                value: ::rquickjs::Value<'js>,
            ) -> ::rquickjs::Result<Self> {
                let class = <::rquickjs::Class<'js, Self> as ::rquickjs::FromJs>::from_js(ctx, value)?;
                let entity = class.borrow();
                ::core::result::Result::Ok(Self::new(entity.inner.clone(), entity.db.clone()))
            }
        }

        #(#attrs)*
        #[::rquickjs::methods]
        impl #class {
            #(#getters)*

            /// Every field under its JS name, so `JSON.stringify` sees the
            /// fields the getters expose.
            #[qjs(rename = "toJSON")]
            fn __to_json<'js>(&self, ctx: ::rquickjs::Ctx<'js>) -> ::rquickjs::Result<::rquickjs::Object<'js>> {
                let json = ::rquickjs::Object::new(ctx.clone())?;
                #(json.set(#names, crate::conversion::ToJs::to_js(&self.inner.#idents, &ctx)?)?;)*
                ::core::result::Result::Ok(json)
            }

            #(#methods)*
        }
    })
}
