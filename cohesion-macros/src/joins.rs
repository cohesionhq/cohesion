//! The `join = Model, on = "..."` clauses `#[model(...)]` and
//! `#[projection(...)]` share.

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::meta::ParseNestedMeta;
use syn::{Attribute, LitStr, Type};

/// A `join = Model, on = "..."` or `left_join = Model, on = "..."`.
struct Join {
    left: bool,
    model: Type,
    on: LitStr,
}

/// The joins a derive's container attributes declare.
#[derive(Default)]
pub(crate) struct Joins(Vec<Join>);

/// The `join`/`left_join` and `on` keys of one `#[...(...)]` attribute,
/// which make a join only together.
#[derive(Default)]
pub(crate) struct JoinClause {
    join: Option<(bool, Type)>,
    on: Option<LitStr>,
}

impl JoinClause {
    /// Reads `meta` if it is one of the join keys; false if it is not.
    pub(crate) fn take(&mut self, meta: &ParseNestedMeta) -> syn::Result<bool> {
        if meta.path.is_ident("join") {
            self.join = Some((false, meta.value()?.parse()?));
        } else if meta.path.is_ident("left_join") {
            self.join = Some((true, meta.value()?.parse()?));
        } else if meta.path.is_ident("on") {
            self.on = Some(meta.value()?.parse()?);
        } else {
            return Ok(false);
        }
        Ok(true)
    }
}

impl Joins {
    /// Adds the join `clause` makes, or none if it had neither key.
    pub(crate) fn push(&mut self, clause: JoinClause, attr: &Attribute) -> syn::Result<()> {
        match (clause.join, clause.on) {
            (Some((left, model)), Some(on)) => self.0.push(Join { left, model, on }),
            (None, None) => {}
            _ => {
                return Err(syn::Error::new_spanned(
                    attr,
                    "a join needs both `join = Model` (or `left_join`) and `on = \"...\"`, \
                     in the same attribute",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The joined models, as written.
    pub(crate) fn models(&self) -> impl Iterator<Item = String> + '_ {
        self.0
            .iter()
            .map(|join| join.model.to_token_stream().to_string())
    }

    /// Whether `model` is joined.
    pub(crate) fn contains(&self, model: &Type) -> bool {
        let wanted = model.to_token_stream().to_string();
        self.models().any(|joined| joined == wanted)
    }

    /// Whether `model` is joined with a left join.
    pub(crate) fn is_left(&self, model: &impl ToTokens) -> bool {
        let wanted = model.to_token_stream().to_string();
        self.0
            .iter()
            .any(|join| join.left && join.model.to_token_stream().to_string() == wanted)
    }

    /// Statements that append each join to a `sql: String`.
    pub(crate) fn push_sql(&self) -> Vec<TokenStream> {
        self.0
            .iter()
            .map(|join| {
                let model = &join.model;
                let keyword = if join.left { "LEFT JOIN" } else { "INNER JOIN" };
                let on = &join.on;
                quote! {
                    sql.push_str(&format!(
                        " {} {} ON {}",
                        #keyword,
                        <#model as ::cohesion_core::stored_model::Model>::TABLE,
                        #on,
                    ));
                }
            })
            .collect()
    }
}
