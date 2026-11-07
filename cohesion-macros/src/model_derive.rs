//! `#[derive(Model)]`: the storage description of a table-backed struct.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Ident, LitStr, Type};

use crate::column_ref::{Term, coalesce_sql, column_const, parse_coalesce};
use crate::derive_input::named_fields;
use crate::joins::{JoinClause, Joins};
use crate::text_enum_rename_rule::RenameRule;

/// How a field is stored.
enum Storage {
    /// One column of the field's type.
    Plain,
    /// One column of JSON text (`#[model(json)]`).
    Json,
    /// Several nullable columns held by a `ColumnGroup` (`#[model(group)]`).
    Group,
}

/// How a field is read, which may differ from how it is stored: its column
/// is always stored, and written, as is.
enum Read {
    /// From the column itself.
    Column,
    /// From a SQL expression instead (`#[model(expr = ...)]`): a string
    /// literal, a path to a `&str` constant, or a column `Model::field`.
    Expr(Term),
    /// From the first of these terms that isn't `NULL`
    /// (`#[model(coalesce(Self::name, Client::client_name))]`).
    Coalesce(Vec<Term>),
    /// From the column, or from this joined model's where that is `NULL`
    /// (`#[model(or_from = Model)]`).
    OrFrom(Type),
}

struct Column {
    ident: Ident,
    ty: Type,
    storage: Storage,
    read_from: Read,
}

impl Column {
    fn name(&self) -> String {
        self.ident.to_string()
    }

    /// Statements adding this field's columns to `columns`.
    fn push_columns(&self) -> TokenStream {
        let name = self.name();
        match self.storage {
            Storage::Plain | Storage::Json => quote! { columns.push(#name); },
            Storage::Group => {
                let ty = &self.ty;
                quote! {
                    columns.extend_from_slice(
                        <#ty as ::cohesion_core::stored_model::ColumnGroup>::COLUMNS,
                    );
                }
            }
        }
    }

    /// Statements adding this field's `SELECT` items to `items`.
    fn push_select_items(&self) -> TokenStream {
        let name = self.name();
        let ty = &self.ty;
        let table = quote! { <Self as ::cohesion_core::stored_model::Model>::TABLE };
        match (&self.storage, &self.read_from) {
            (Storage::Group, _) => quote! {
                items.extend(
                    <#ty as ::cohesion_core::stored_model::ColumnGroup>::COLUMNS
                        .iter()
                        .map(|column| format!("{}.{}", #table, column)),
                );
            },
            (_, Read::Column) => quote! { items.push(format!("{}.{}", #table, #name)); },
            (_, Read::Expr(term)) => {
                let sql = term.sql();
                quote! { items.push(format!("{} AS {}", #sql, #name)); }
            }
            (_, Read::Coalesce(terms)) => {
                let sql = coalesce_sql(terms);
                quote! { items.push(format!("{} AS {}", #sql, #name)); }
            }
            (_, Read::OrFrom(model)) => quote! {
                items.push(format!(
                    "COALESCE({base}.{column}, {joined}.{column}) AS {column}",
                    base = #table,
                    joined = <#model as ::cohesion_core::stored_model::Model>::TABLE,
                    column = #name,
                ));
            },
        }
    }

    /// The expression reading this field from `row`.
    fn read(&self) -> TokenStream {
        let name = self.name();
        let ty = &self.ty;
        match self.storage {
            Storage::Plain => quote! { ::cohesion_core::value::read_column(row, #name)? },
            Storage::Json => quote! {
                ::cohesion_core::value::read_column::<::cohesion_core::value::Json<#ty>>(
                    row, #name,
                )?.0
            },
            Storage::Group => quote! {
                <#ty as ::cohesion_core::stored_model::ColumnGroup>::from_row(row)?
            },
        }
    }
}

pub(crate) fn model(input: DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let mut table: Option<LitStr> = None;
    let mut backup: Option<Ident> = None;
    let mut joins = Joins::default();
    for attr in input.attrs.iter().filter(|a| a.path().is_ident("model")) {
        let mut clause = JoinClause::default();
        attr.parse_nested_meta(|meta| {
            if clause.take(&meta)? {
            } else if meta.path.is_ident("table") {
                table = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("backup") {
                backup = Some(meta.value()?.parse()?);
            } else {
                return Err(meta.error(
                    "expected `table = \"...\"`, `backup = Included` (or `LeftOut`), or \
                     `join = Model, on = \"...\"` (`left_join` for a left join)",
                ));
            }
            Ok(())
        })?;
        joins.push(clause, attr)?;
    }
    let Some(table) = table else {
        return Err(syn::Error::new_spanned(
            name,
            "#[derive(Model)] needs #[model(table = \"...\", backup = ...)]",
        ));
    };
    let Some(backup) = backup else {
        return Err(syn::Error::new_spanned(
            name,
            "#[derive(Model)] needs `backup = Included` or `backup = LeftOut`: \
             say whether the table's rows go in a backup",
        ));
    };
    // `TaskFolder` is "task folder" in a client's messages.
    let model_name = RenameRule::LowerSpace.apply(&name.to_string());

    // What a column reference (`Model::field`) may name: this model, by
    // `Self` or its name, and the models it joins.
    let mut referable = vec!["Self".to_string(), name.to_string()];
    referable.extend(joins.models());

    let fields = named_fields(&input, "Model")?;
    let mut id: Option<Column> = None;
    let mut columns = Vec::new();
    let mut skipped = Vec::new();
    let mut checks = Vec::new();
    let mut soft_delete = false;
    for (index, field) in fields.enumerate() {
        let ident = field.ident.clone().expect("named fields have identifiers");
        let mut skip = false;
        let mut storage = Storage::Plain;
        let mut read_from = Read::Column;
        for attr in field.attrs.iter().filter(|a| a.path().is_ident("model")) {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("skip") {
                    skip = true;
                } else if meta.path.is_ident("json") {
                    storage = Storage::Json;
                } else if meta.path.is_ident("group") {
                    storage = Storage::Group;
                } else if meta.path.is_ident("expr") {
                    let term: Term = meta.value()?.parse()?;
                    term.check_model(&referable)?;
                    read_from = Read::Expr(term);
                } else if meta.path.is_ident("coalesce") {
                    let terms = parse_coalesce(meta.input)?;
                    for term in &terms {
                        term.check_model(&referable)?;
                    }
                    read_from = Read::Coalesce(terms);
                } else if meta.path.is_ident("or_from") {
                    let model: Type = meta.value()?.parse()?;
                    if !joins.contains(&model) {
                        return Err(meta.error("`or_from` must name a model this one joins"));
                    }
                    read_from = Read::OrFrom(model);
                } else {
                    return Err(meta.error(
                        "expected `skip`, `json`, `group`, `expr = ...`, `coalesce(...)` \
                         or `or_from = Model`",
                    ));
                }
                Ok(())
            })?;
        }
        if !matches!(read_from, Read::Column) && (skip || matches!(storage, Storage::Group)) {
            return Err(syn::Error::new_spanned(
                &ident,
                "`expr`, `coalesce` and `or_from` change how one stored column is read: a `skip` or \
                 `group` field has no single column",
            ));
        }
        if let Read::OrFrom(joined) = &read_from {
            // The joined model must have a field of this name and type.
            let check = format_ident!("column_{index}_is_a_field_of_the_joined_model");
            let ty = &field.ty;
            checks.push(quote! {
                #[allow(dead_code)]
                fn #check(joined: &#joined) { let _: &#ty = &joined.#ident; }
            });
        }
        let column = Column {
            ident,
            ty: field.ty.clone(),
            storage,
            read_from,
        };
        if column.name() == "id" {
            if skip
                || !matches!(column.storage, Storage::Plain)
                || !matches!(column.read_from, Read::Column)
                || !is_ident_type(&field.ty, "i64")
            {
                return Err(syn::Error::new_spanned(
                    &field.ty,
                    "a Model's `id` field must be a column of type `i64`",
                ));
            }
            id = Some(column);
        } else if skip {
            skipped.push(column.ident);
        } else {
            soft_delete |= is_ident_type(&field.ty, "bool") && column.name() == "deleted";
            columns.push(column);
        }
    }
    let Some(id) = id else {
        return Err(syn::Error::new_spanned(
            name,
            "#[derive(Model)] needs a field `id: i64`",
        ));
    };
    // `id` always comes first, whatever the declaration order.
    columns.insert(0, id);

    // `RETURNING *` reads a row of the table alone: only a model that joins
    // nothing and reads every field from its column can be read from one.
    let single_table =
        joins.is_empty() && columns.iter().all(|c| matches!(c.read_from, Read::Column));
    let single_table = single_table.then(|| {
        quote! { impl ::cohesion_core::stored_model::SingleTable for #name {} }
    });
    // A group's columns are named after its variants' fields, so the
    // compiler checks that none of them is also another column of the model.
    let plain_names: Vec<String> = columns
        .iter()
        .filter(|c| !matches!(c.storage, Storage::Group))
        .map(Column::name)
        .collect();
    let groups: Vec<&Type> = columns
        .iter()
        .filter(|c| matches!(c.storage, Storage::Group))
        .map(|c| &c.ty)
        .collect();
    for (index, group) in groups.iter().enumerate() {
        let others = groups[index + 1..].iter().map(|other| {
            quote! {
                ::cohesion_core::stored_model::assert_columns_disjoint(
                    <#group as ::cohesion_core::stored_model::ColumnGroup>::COLUMNS,
                    <#other as ::cohesion_core::stored_model::ColumnGroup>::COLUMNS,
                );
            }
        });
        checks.push(quote! {
            const _: () = {
                ::cohesion_core::stored_model::assert_columns_disjoint(
                    <#group as ::cohesion_core::stored_model::ColumnGroup>::COLUMNS,
                    &[#(#plain_names),*],
                );
                #(#others)*
            };
        });
    }
    let push_select_items = columns.iter().map(Column::push_select_items);
    let join_sql = joins.push_sql();
    let push_columns = columns.iter().map(Column::push_columns);
    // A typed constant per stored column, for queries; `Model::field` in
    // another derive's attribute names a column through it too. A group's
    // columns have none.
    let table_name = table.value();
    let column_consts = columns
        .iter()
        .filter(|c| !matches!(c.storage, Storage::Group))
        .map(|c| {
            let constant = column_const(&c.ident);
            let column = c.name();
            let qualified = format!("{table_name}.{column}");
            let ty = &c.ty;
            // A field read through a fallback or an expression is stored in a
            // column that may be `NULL`, whatever the field's own type.
            let value = match (&c.storage, &c.read_from) {
                (Storage::Json, _) => quote! { ::cohesion_core::value::Json<#ty> },
                (_, Read::Coalesce(_) | Read::Expr(_)) if !is_option(ty) => {
                    quote! { ::std::option::Option<#ty> }
                }
                _ => quote! { #ty },
            };
            let doc = format!("The `{column}` column.");
            quote! {
                #[doc = #doc]
                pub const #constant: ::cohesion_core::stored_model::Column<Self, #value> =
                    ::cohesion_core::stored_model::Column::new(#column, #qualified);
            }
        });
    let idents: Vec<&Ident> = columns.iter().map(|c| &c.ident).collect();
    let reads = columns.iter().map(Column::read);
    let soft_delete = soft_delete.then(|| {
        quote! { impl ::cohesion_core::stored_model::SoftDelete for #name {} }
    });

    Ok(quote! {
        const _: () = {
            #(#checks)*
        };

        #soft_delete
        #single_table

        impl #name {
            #(#column_consts)*
        }

        impl ::cohesion_core::stored_model::Named for #name {
            const NOUN: &'static str = #model_name;
        }

        impl ::cohesion_core::stored_model::Model for #name {
            const TABLE: &'static str = #table;
            const BACKUP: ::cohesion_core::stored_model::BackupClass =
                ::cohesion_core::stored_model::BackupClass::#backup;

            fn columns() -> ::std::vec::Vec<&'static str> {
                let mut columns = ::std::vec::Vec::new();
                #(#push_columns)*
                columns
            }

            fn select_list() -> ::std::string::String {
                let mut items: ::std::vec::Vec<::std::string::String> = ::std::vec::Vec::new();
                #(#push_select_items)*
                items.join(", ")
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
                    #(#skipped: ::core::default::Default::default(),)*
                })
            }

            fn id(&self) -> i64 {
                self.id
            }
        }
    })
}

/// Whether `ty` is written as the bare identifier `ident`, such as `i64`.
fn is_ident_type(ty: &Type, ident: &str) -> bool {
    matches!(ty, Type::Path(path) if path.qself.is_none() && path.path.is_ident(ident))
}

/// Whether `ty` is written `Option<...>`.
fn is_option(ty: &Type) -> bool {
    matches!(ty, Type::Path(path) if path.path.segments.last().is_some_and(|s| s.ident == "Option"))
}
