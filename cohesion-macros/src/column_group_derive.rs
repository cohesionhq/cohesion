//! `#[derive(ColumnGroup)]`: an enum stored as a group of nullable columns.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Ident, Type};

/// One variant: its name and the (column, index into the group, type) of
/// each field.
struct Variant {
    ident: Ident,
    fields: Vec<(Ident, usize, Type)>,
}

pub(crate) fn column_group(input: DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let Data::Enum(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            name,
            "#[derive(ColumnGroup)] needs an enum",
        ));
    };

    // Every column once, in order of first appearance.
    let mut columns: Vec<String> = Vec::new();
    let mut variants = Vec::new();
    for variant in &data.variants {
        let mut fields = Vec::new();
        match &variant.fields {
            Fields::Unit => {}
            Fields::Named(named) => {
                for field in &named.named {
                    let ident = field.ident.clone().expect("named fields have identifiers");
                    let column = ident.to_string();
                    let index = match columns.iter().position(|c| *c == column) {
                        Some(index) => index,
                        None => {
                            columns.push(column);
                            columns.len() - 1
                        }
                    };
                    fields.push((ident, index, field.ty.clone()));
                }
            }
            Fields::Unnamed(_) => {
                return Err(syn::Error::new_spanned(
                    variant,
                    "a ColumnGroup variant's fields are named: each is a column",
                ));
            }
        }
        variants.push(Variant {
            ident: variant.ident.clone(),
            fields,
        });
    }
    let count = columns.len();

    let candidates = variants.iter().map(|variant| {
        let ident = &variant.ident;
        let mine: Vec<usize> = variant.fields.iter().map(|(_, index, _)| *index).collect();
        let required: Vec<usize> = variant
            .fields
            .iter()
            .filter(|(_, _, ty)| crate::ts_type_mapping::option_inner(ty).is_none())
            .map(|(_, index, _)| *index)
            .collect();
        let names = variant.fields.iter().map(|(ident, ..)| ident);
        let reads = variant.fields.iter().map(|(ident, index, _)| {
            let column = ident.to_string();
            quote! {
                ::cohesion_core::value::FromValue::from_value(values[#index].clone())
                    .map_err(|error| ::cohesion_core::value::RowError::Value {
                        column: #column.to_string(),
                        error,
                    })?
            }
        });
        quote! {
            let mine: &[usize] = &[#(#mine),*];
            let required: &[usize] = &[#(#required),*];
            let fits = required.iter().all(|index| !is_null(&values[*index]))
                && (0..#count)
                    .filter(|index| !mine.contains(index))
                    .all(|index| is_null(&values[index]));
            if fits {
                matches += 1;
                found = Some(Self::#ident { #(#names: #reads,)* });
            }
        }
    });

    let into_values = variants.iter().map(|variant| {
        let ident = &variant.ident;
        let names: Vec<&Ident> = variant.fields.iter().map(|(ident, ..)| ident).collect();
        let sets = variant.fields.iter().map(|(ident, index, _)| {
            quote! { values[#index] = ::cohesion_core::value::IntoValue::into_value(#ident); }
        });
        quote! {
            Self::#ident { #(#names),* } => { #(#sets)* }
        }
    });

    let column_reads = columns.iter().map(|column| quote! { row.get(#column)? });

    Ok(quote! {
        impl ::cohesion_core::stored_model::ColumnGroup for #name {
            const COLUMNS: &'static [&'static str] = &[#(#columns),*];

            fn from_row(
                row: &dyn ::cohesion_core::value::Row,
            ) -> ::core::result::Result<Self, ::cohesion_core::value::RowError> {
                fn is_null(value: &::cohesion_core::value::Value) -> bool {
                    matches!(value, ::cohesion_core::value::Value::Null)
                }
                let values: [::cohesion_core::value::Value; #count] = [#(#column_reads),*];
                let mut found = ::core::option::Option::None;
                let mut matches = 0usize;
                #({ #candidates })*
                match found {
                    ::core::option::Option::Some(found) if matches == 1 => {
                        ::core::result::Result::Ok(found)
                    }
                    _ => ::core::result::Result::Err(::cohesion_core::value::RowError::Value {
                        column: <Self as ::cohesion_core::stored_model::ColumnGroup>::COLUMNS
                            .join(", "),
                        error: ::cohesion_core::value::ValueError::Invalid(format!(
                            "{matches} of {}'s variants fit these columns, not one",
                            stringify!(#name),
                        )),
                    }),
                }
            }

            fn into_values(self) -> ::std::vec::Vec<::cohesion_core::value::Value> {
                let mut values = ::std::vec![::cohesion_core::value::Value::Null; #count];
                match self {
                    #(#into_values)*
                }
                values
            }
        }
    })
}
