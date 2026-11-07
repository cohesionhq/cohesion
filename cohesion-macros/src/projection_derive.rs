//! `#[derive(Projection)]`: a struct read from some of a model's columns,
//! columns of models it joins, or SQL expressions over them.

use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{DeriveInput, Type, TypePath};

use crate::column_ref::{ColumnRef, FromTarget, Term, coalesce_sql, parse_coalesce};
use crate::derive_input::named_fields;
use crate::joins::{JoinClause, Joins};

/// Where a field's column comes from.
enum Source {
    /// A column of the model the projection reads.
    Base,
    /// The column of this joined model.
    Joined(Type),
    /// A column of the model or one it joins, named by a field of another
    /// name (`#[projection(from = Model::field)]`).
    Column(ColumnRef),
    /// The model's column, or this joined model's where that is `NULL`.
    OrFrom(Type),
    /// A SQL expression: a string literal, a path to a `&str` constant, or
    /// a column `Model::field`.
    Expr(Term),
    /// The first of these terms that isn't `NULL` (`coalesce(...)`).
    Coalesce(Vec<Term>),
    /// The columns of the model's `ColumnGroup` field of this name
    /// (`#[projection(group)]`).
    Group,
}

pub(crate) fn projection(input: DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let mut of: Option<Type> = None;
    let mut joins = Joins::default();
    for attr in input
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("projection"))
    {
        let mut clause = JoinClause::default();
        attr.parse_nested_meta(|meta| {
            if clause.take(&meta)? {
            } else if meta.path.is_ident("of") {
                of = Some(meta.value()?.parse()?);
            } else {
                return Err(meta.error(
                    "expected `of = Model`, or `join = Model, on = \"...\"` \
                     (`left_join` for a left join)",
                ));
            }
            Ok(())
        })?;
        joins.push(clause, attr)?;
    }
    let Some(of) = of else {
        return Err(syn::Error::new_spanned(
            name,
            "#[derive(Projection)] needs #[projection(of = Model)], naming the model it reads",
        ));
    };

    // What a column reference (`Model::field`) may name: the model read
    // and the models joined.
    let mut referable = vec![of.to_token_stream().to_string()];
    referable.extend(joins.models());

    let mut selected = Vec::new();
    let mut reads = Vec::new();
    let mut idents = Vec::new();
    let mut checks = Vec::new();
    for (index, field) in named_fields(&input, "Projection")?.enumerate() {
        let ident = field.ident.clone().expect("named fields have identifiers");
        let mut source = Source::Base;
        let mut json = false;
        for attr in field
            .attrs
            .iter()
            .filter(|a| a.path().is_ident("projection"))
        {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("expr") {
                    let term: Term = meta.value()?.parse()?;
                    term.check_model(&referable)?;
                    source = Source::Expr(term);
                } else if meta.path.is_ident("coalesce") {
                    let terms = parse_coalesce(meta.input)?;
                    for term in &terms {
                        term.check_model(&referable)?;
                    }
                    source = Source::Coalesce(terms);
                } else if meta.path.is_ident("from") || meta.path.is_ident("or_from") {
                    let or = meta.path.is_ident("or_from");
                    let model = match meta.value()?.parse()? {
                        FromTarget::Column(column) if !or => {
                            column.check_model(&referable)?;
                            source = Source::Column(column);
                            return Ok(());
                        }
                        FromTarget::Column(column) => {
                            return Err(syn::Error::new_spanned(
                                &column.field,
                                "`or_from` names a model; the field is this one's",
                            ));
                        }
                        FromTarget::Model(path) => Type::Path(TypePath { qself: None, path }),
                    };
                    if !joins.contains(&model) {
                        return Err(meta.error("`from` must name a model the projection joins"));
                    }
                    source = if or {
                        Source::OrFrom(model)
                    } else {
                        Source::Joined(model)
                    };
                } else if meta.path.is_ident("json") {
                    json = true;
                } else if meta.path.is_ident("group") {
                    source = Source::Group;
                } else {
                    return Err(meta.error(
                        "expected `expr = ...`, `coalesce(...)`, `from = Model`, \
                         `from = Model::field`, `or_from = Model`, `json` or `group`",
                    ));
                }
                Ok(())
            })?;
        }
        let column = ident.to_string();
        let ty = &field.ty;
        // A column must be a field of its model, of the same type: checked
        // by reading it from a value of that model.
        let check = format_ident!("column_{index}_is_a_field");
        match &source {
            Source::Base => {
                checks.push(quote! {
                    #[allow(dead_code)]
                    fn #check(model: &#of) { let _: &#ty = &model.#ident; }
                });
                selected.push(quote! {
                    format!(
                        "{}.{}",
                        <#of as ::cohesion_core::stored_model::Model>::TABLE,
                        #column,
                    )
                });
            }
            Source::Joined(model) => {
                // A left join may find no row, so its column is read as an
                // `Option` of the model's type: only an inner join's column
                // has the model's type exactly.
                let left = joins.is_left(model);
                checks.push(if left {
                    quote! {
                        #[allow(dead_code)]
                        fn #check(model: &#model) { let _ = &model.#ident; }
                    }
                } else {
                    quote! {
                        #[allow(dead_code)]
                        fn #check(model: &#model) { let _: &#ty = &model.#ident; }
                    }
                });
                selected.push(quote! {
                    format!(
                        "{table}.{column} AS {column}",
                        table = <#model as ::cohesion_core::stored_model::Model>::TABLE,
                        column = #column,
                    )
                });
            }
            Source::OrFrom(model) => {
                checks.push(quote! {
                    #[allow(dead_code)]
                    fn #check(model: &#of, joined: &#model) {
                        let _: &#ty = &model.#ident;
                        let _: &#ty = &joined.#ident;
                    }
                });
                selected.push(quote! {
                    format!(
                        "COALESCE({base}.{column}, {joined}.{column}) AS {column}",
                        base = <#of as ::cohesion_core::stored_model::Model>::TABLE,
                        joined = <#model as ::cohesion_core::stored_model::Model>::TABLE,
                        column = #column,
                    )
                });
            }
            Source::Column(reference) => {
                // As for `from = Model`: a left-joined column may be `NULL`.
                let model = &reference.model;
                let field = &reference.field;
                let left = joins.is_left(model);
                checks.push(if left {
                    quote! {
                        #[allow(dead_code)]
                        fn #check(model: &#model) { let _ = &model.#field; }
                    }
                } else {
                    quote! {
                        #[allow(dead_code)]
                        fn #check(model: &#model) { let _: &#ty = &model.#field; }
                    }
                });
                let sql = reference.sql();
                selected.push(quote! { format!("{} AS {}", #sql, #column) });
            }
            Source::Expr(term) => {
                let sql = term.sql();
                selected.push(quote! { format!("{} AS {}", #sql, #column) });
            }
            Source::Coalesce(terms) => {
                let sql = coalesce_sql(terms);
                selected.push(quote! { format!("{} AS {}", #sql, #column) });
            }
            Source::Group => {
                checks.push(quote! {
                    #[allow(dead_code)]
                    fn #check(model: &#of) { let _: &#ty = &model.#ident; }
                });
                selected.push(quote! {
                    <#ty as ::cohesion_core::stored_model::ColumnGroup>::COLUMNS
                        .iter()
                        .map(|column| format!(
                            "{}.{}",
                            <#of as ::cohesion_core::stored_model::Model>::TABLE,
                            column,
                        ))
                        .collect::<::std::vec::Vec<_>>()
                        .join(", ")
                });
            }
        }
        reads.push(if matches!(source, Source::Group) {
            quote! { <#ty as ::cohesion_core::stored_model::ColumnGroup>::from_row(row)? }
        } else if json {
            quote! {
                ::cohesion_core::value::read_column::<::cohesion_core::value::Json<#ty>>(
                    row, #column,
                )?.0
            }
        } else {
            quote! { ::cohesion_core::value::read_column(row, #column)? }
        });
        idents.push(ident);
    }

    let join_sql = joins.push_sql();

    Ok(quote! {
        const _: () = {
            #(#checks)*
        };

        impl ::cohesion_core::stored_model::Named for #name {
            const NOUN: &'static str = <#of as ::cohesion_core::stored_model::Named>::NOUN;
        }

        impl ::cohesion_core::stored_model::Projection for #name {
            type Of = #of;

            fn columns() -> ::std::string::String {
                [#(#selected),*].join(", ")
            }

            fn joins() -> ::std::string::String {
                let mut sql = ::std::string::String::new();
                #(#join_sql)*
                sql
            }

            fn from_row(
                row: &dyn ::cohesion_core::value::Row,
            ) -> ::core::result::Result<Self, ::cohesion_core::value::RowError> {
                ::core::result::Result::Ok(Self {
                    #(#idents: #reads,)*
                })
            }
        }
    })
}
