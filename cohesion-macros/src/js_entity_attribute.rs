//! `#[js_entity]`, first half: asks the struct's fields macro to call
//! `__js_entity_expand!` back with the fields.

use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Ident, LitStr, Path};

/// The contents of `#[js_entity(Model, js = "Name", metadata)]`. A bare
/// `metadata` keeps the model's own; `metadata = Other` keeps `Other`'s, for
/// an entity over a projection of `Other`.
pub(crate) struct JsEntityArgs {
    model: Path,
    js_name: Option<LitStr>,
    metadata: Option<Path>,
}

impl Parse for JsEntityArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let model: Path = input.parse()?;
        let mut js_name = None;
        let mut metadata = None;
        while input.parse::<Option<syn::Token![,]>>()?.is_some() && !input.is_empty() {
            let keyword: Ident = input.parse()?;
            if keyword == "js" {
                input.parse::<syn::Token![=]>()?;
                js_name = Some(input.parse()?);
            } else if keyword == "metadata" {
                metadata = Some(match input.parse::<Option<syn::Token![=]>>()? {
                    Some(_) => input.parse()?,
                    None => model.clone(),
                });
            } else {
                return Err(syn::Error::new_spanned(
                    keyword,
                    "expected `js = \"Name\"`, `metadata` or `metadata = Model`",
                ));
            }
        }
        Ok(Self {
            model,
            js_name,
            metadata,
        })
    }
}

/// `metadata(key)` and `setMetadata(key, value)` on an entity whose model is
/// table-backed, stored in the `metadata` table under the model's `TABLE`.
fn metadata_methods(model: &Path) -> [syn::ImplItem; 2] {
    [
        syn::parse_quote! {
            /// The string stored under `key` on this item with `setMetadata`,
            /// or `null` if there is none.
            #[ts(returns = "string | null")]
            pub fn metadata<'js>(
                &self,
                ctx: ::rquickjs::Ctx<'js>,
                key: String,
            ) -> ::rquickjs::Result<::rquickjs::Value<'js>> {
                let value = self.read(
                    &ctx,
                    ::cohesion_db::GetMetadataCommand {
                        owner: <#model as ::cohesion_core::model::HasMetadata>::metadata_owner(
                            self.inner.id,
                        ),
                        key,
                    },
                )?;
                // `ToJs` makes `None` `null`; rquickjs alone would make it `undefined`.
                crate::conversion::ToJs::to_js(&value, &ctx)
            }
        },
        syn::parse_quote! {
            /// Stores the string `value` under `key` on this item, replacing
            /// any earlier value; `null` removes the key.
            ///
            /// Saved immediately. Keys are shared by every script and
            /// extension, so name them after their source (`"todoist_id"`).
            /// Find items by metadata with the `metadata` option of the list
            /// methods.
            #[qjs(rename = "setMetadata")]
            pub fn set_metadata<'js>(
                &self,
                ctx: ::rquickjs::Ctx<'js>,
                key: String,
                value: Option<String>,
            ) -> ::rquickjs::Result<()> {
                self.write(
                    &ctx,
                    ::cohesion_db::SetMetadataCommand {
                        owner: <#model as ::cohesion_core::model::HasMetadata>::metadata_owner(
                            self.inner.id,
                        ),
                        key,
                        value,
                    },
                )
            }
        },
    ]
}

pub(crate) fn js_entity_impl(
    args: JsEntityArgs,
    mut item: syn::ItemImpl,
) -> syn::Result<proc_macro2::TokenStream> {
    if let Some((_, path, _)) = &item.trait_ {
        return Err(syn::Error::new_spanned(
            path,
            "#[js_entity] goes on an inherent impl block, not a trait impl",
        ));
    }
    let model = &args.model;
    if let Some(owner) = &args.metadata {
        item.items.extend(metadata_methods(owner));
    }
    let model_ident = &model
        .segments
        .last()
        .ok_or_else(|| syn::Error::new_spanned(model, "expected a model name"))?
        .ident;
    let js_name = args
        .js_name
        .unwrap_or_else(|| LitStr::new(&model_ident.to_string(), model_ident.span()));
    let fields_macro = Ident::new(
        &format!("__cohesion_fields_{model_ident}"),
        model_ident.span(),
    );
    Ok(quote! {
        ::cohesion_core::#fields_macro! {
            cohesion_macros::__js_entity_expand { model(#model) js(#js_name) #item }
        }
    })
}
