//! `#[derive(TextEnum)]`: a fieldless enum parsed and scripted as text, and
//! stored as text or, with `store = "integer"`, as its discriminant.

use quote::quote;
use syn::{Data, DeriveInput, Fields, Ident, LitStr};

use crate::identifier_case::camel_case;
use crate::text_enum_rename_rule::RenameRule;
use crate::ts_declaration::{Detail, doc_text};

pub(crate) fn text_enum(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let Data::Enum(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            name,
            "#[derive(TextEnum)] only supports enums",
        ));
    };

    let mut default: Option<Ident> = None;
    let mut rename_all = RenameRule::Kebab;
    let mut store_integer = false;
    for attr in input
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("text_enum"))
    {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("default") {
                default = Some(meta.value()?.parse()?);
                Ok(())
            } else if meta.path.is_ident("rename_all") {
                let literal: LitStr = meta.value()?.parse()?;
                rename_all = RenameRule::parse(&literal.value()).ok_or_else(|| {
                    syn::Error::new_spanned(
                        &literal,
                        format!(
                            "unknown rename_all case; expected one of {}",
                            RenameRule::NAMES
                        ),
                    )
                })?;
                Ok(())
            } else if meta.path.is_ident("store") {
                let literal: LitStr = meta.value()?.parse()?;
                store_integer = match literal.value().as_str() {
                    "text" => false,
                    "integer" => true,
                    _ => {
                        return Err(syn::Error::new_spanned(
                            &literal,
                            "unknown store; expected \"text\" or \"integer\"",
                        ));
                    }
                };
                Ok(())
            } else {
                Err(meta.error(
                    "expected `default = Variant`, `rename_all = \"case\"` or `store = \"integer\"`",
                ))
            }
        })?;
    }

    let mut variants = Vec::new();
    let mut texts = Vec::new();
    let mut camels = Vec::new();
    for variant in &data.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(syn::Error::new_spanned(
                variant,
                "#[derive(TextEnum)] variants must not have fields",
            ));
        }
        let text = match (rename_all, text_enum_raw(variant)?) {
            (RenameRule::Custom, Some(raw)) => {
                if texts.contains(&raw.value()) {
                    return Err(syn::Error::new_spanned(
                        &raw,
                        "another variant already has this text",
                    ));
                }
                raw.value()
            }
            (RenameRule::Custom, None) => {
                return Err(syn::Error::new_spanned(
                    variant,
                    "`rename_all = \"custom\"` needs `#[text_enum(\"a:b\")]` on every variant",
                ));
            }
            (_, Some(raw)) => {
                return Err(syn::Error::new_spanned(
                    raw,
                    "variant text needs `#[text_enum(rename_all = \"custom\")]` on the enum",
                ));
            }
            (rule, None) => rule.apply(&variant.ident.to_string()),
        };
        variants.push(&variant.ident);
        texts.push(text);
        camels.push(camel_case(&variant.ident.to_string()));
    }
    // Single-word variants spell the same both ways; an or-pattern repeating
    // the literal would be an unreachable pattern.
    let patterns: Vec<_> = texts
        .iter()
        .zip(&camels)
        .map(|(text, camel)| {
            if text == camel {
                quote! { #text }
            } else {
                quote! { #text | #camel }
            }
        })
        .collect();
    if let Some(default) = &default
        && !variants.contains(&default)
    {
        return Err(syn::Error::new_spanned(
            default,
            format!("`{default}` is not a variant of `{name}`"),
        ));
    }

    let doc = doc_text(&input.attrs);
    let union = texts
        .iter()
        .map(|text| format!("{text:?}"))
        .collect::<Vec<_>>()
        .join(" | ");
    let ts_type = |detail: Detail| format!("{}type {name} = {union};", detail.jsdoc(&doc, ""));
    let (ts, ts_brief) = (ts_type(Detail::Full), ts_type(Detail::Brief));
    let cases: String = texts
        .iter()
        .zip(&camels)
        .map(|(text, camel)| format!("  readonly {camel}: {text:?};\n"))
        .collect();
    let ts_global = |detail: Detail| {
        format!(
            "{}declare const {name}: {{\n{cases}}};",
            detail.jsdoc(&doc, "")
        )
    };
    let (ts_global, ts_global_brief) = (ts_global(Detail::Full), ts_global(Detail::Brief));

    // Custom text is the only spelling, so serde must use it too; a serde
    // derive would write the variant name.
    let serde_impls = matches!(rename_all, RenameRule::Custom).then(|| {
        quote! {
            impl ::cohesion_core::__serde::Serialize for #name {
                fn serialize<S: ::cohesion_core::__serde::Serializer>(
                    &self,
                    serializer: S,
                ) -> ::core::result::Result<S::Ok, S::Error> {
                    serializer.serialize_str(self.as_str())
                }
            }

            impl<'de> ::cohesion_core::__serde::Deserialize<'de> for #name {
                fn deserialize<D: ::cohesion_core::__serde::Deserializer<'de>>(
                    deserializer: D,
                ) -> ::core::result::Result<Self, D::Error> {
                    let text: ::std::string::String =
                        ::cohesion_core::__serde::Deserialize::deserialize(deserializer)?;
                    text.parse()
                        .map_err(<D::Error as ::cohesion_core::__serde::de::Error>::custom)
                }
            }
        }
    });

    let forced_fallback = {
        let message = format!("unknown {name}: {{other}}");
        quote! {
            other => ::core::result::Result::Err(
                ::cohesion_core::value::ValueError::Invalid(::std::format!(#message))
            ),
        }
    };

    let fallback = match &default {
        Some(default) => {
            let message = format!("unknown {name} {{:?}}; using {default}");
            quote! {
                other => {
                    ::cohesion_core::log::warn(
                        ::cohesion_core::log::Module::Database,
                        ::std::format_args!(#message, other),
                    );
                    ::core::result::Result::Ok(Self::#default)
                }
            }
        }
        None => forced_fallback.clone(),
    };

    let value_impls = if store_integer {
        let fallback = match &default {
            Some(default) => {
                let message = format!("unknown {name} {{}}; using {default}");
                quote! {
                    ::cohesion_core::log::warn(
                        ::cohesion_core::log::Module::Database,
                        ::std::format_args!(#message, number),
                    );
                    ::core::result::Result::Ok(Self::#default)
                }
            }
            None => {
                let message = format!("unknown {name}: {{number}}");
                quote! {
                    ::core::result::Result::Err(
                        ::cohesion_core::value::ValueError::Invalid(::std::format!(#message))
                    )
                }
            }
        };
        quote! {
            impl ::cohesion_core::value::IntoValue for #name {
                fn into_value(self) -> ::cohesion_core::value::Value {
                    ::cohesion_core::value::Value::Integer(self as i64)
                }
            }

            impl ::cohesion_core::value::FromValue for #name {
                fn from_value(
                    value: ::cohesion_core::value::Value,
                ) -> ::core::result::Result<Self, ::cohesion_core::value::ValueError> {
                    match value {
                        ::cohesion_core::value::Value::Integer(number) => {
                            #(if number == Self::#variants as i64 {
                                return ::core::result::Result::Ok(Self::#variants);
                            })*
                            #fallback
                        }
                        other => ::core::result::Result::Err(
                            ::cohesion_core::value::ValueError::wrong_type("integer", &other)
                        ),
                    }
                }
            }
        }
    } else {
        quote! {
            impl ::cohesion_core::value::IntoValue for #name {
                fn into_value(self) -> ::cohesion_core::value::Value {
                    ::cohesion_core::value::Value::Text(self.as_str().to_string())
                }
            }

            impl ::cohesion_core::value::FromValue for #name {
                fn from_value(
                    value: ::cohesion_core::value::Value,
                ) -> ::core::result::Result<Self, ::cohesion_core::value::ValueError> {
                    match value {
                        ::cohesion_core::value::Value::Text(text) => text.parse(),
                        other => ::core::result::Result::Err(
                            ::cohesion_core::value::ValueError::wrong_type("text", &other)
                        ),
                    }
                }
            }
        }
    };

    Ok(quote! {
        #serde_impls

        impl #name {
            pub const fn as_str(&self) -> &'static str {
                match self {
                    #(Self::#variants => #texts,)*
                }
            }

            /// Every variant, in declaration order.
            pub const ALL: &'static [Self] = &[#(Self::#variants,)*];

            /// Every variant's text, in declaration order.
            pub const CASES: &'static [&'static str] = &[#(#texts,)*];

            /// Each variant's camelCase name, usable as a JS property, with
            /// its text.
            pub const JS_CASES: &'static [(&'static str, &'static str)] =
                &[#((#camels, #texts),)*];

            /// TypeScript for the frozen object of cases that `mount_enum!`
            /// defines as a global; `TS` declares only the type.
            pub const TS_GLOBAL: &'static str = #ts_global;

            /// `TS_GLOBAL` with only its doc comment's first paragraph.
            pub const TS_GLOBAL_BRIEF: &'static str = #ts_global_brief;
        }

        impl ::cohesion_core::ts_documentation::TsDocumentation for #name {
            const TS: &'static str = #ts;
            const TS_BRIEF: &'static str = #ts_brief;
        }

        #value_impls

        impl ::core::fmt::Display for #name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl ::core::str::FromStr for #name {
            type Err = ::cohesion_core::value::ValueError;

            fn from_str(text: &str) -> ::core::result::Result<Self, Self::Err> {
                match text {
                    #(#patterns => ::core::result::Result::Ok(Self::#variants),)*
                    #fallback
                }
            }
        }

        impl #name {
            /// Like `FromStr`, but unknown text is always an error, even
            /// when the enum has a `default`.
            pub fn from_str_strict(text: &str) -> ::core::result::Result<Self, ::cohesion_core::value::ValueError> {
                match text {
                    #(#patterns => ::core::result::Result::Ok(Self::#variants),)*
                    #forced_fallback
                }
            }
        }
    })
}

/// A variant's `#[text_enum("...")]`.
fn text_enum_raw(variant: &syn::Variant) -> syn::Result<Option<LitStr>> {
    let mut raw = None;
    for attr in variant
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("text_enum"))
    {
        raw = Some(attr.parse_args::<LitStr>()?);
    }
    Ok(raw)
}
