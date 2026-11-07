//! Typed references to model columns in derive attributes, such as
//! `CatalogModel::model_name`, and the `coalesce(...)` lists built from them.
//!
//! `#[derive(Model)]` gives every stored column a typed constant
//! (`Model::FIELD`, a `Column`); a reference expands to its qualified name,
//! so a misspelled or non-column field is a compile error rather than bad
//! SQL at run time.

use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Expr, ExprLit, ExprPath, Ident, Lit, LitStr, Path, Token};

/// The `Column` constant `#[derive(Model)]` emits for the stored column of
/// field `field`: the field's name in upper case.
pub(crate) fn column_const(field: &Ident) -> Ident {
    format_ident!("{}", field.to_string().to_uppercase())
}

/// `Model::field`: the column a model stores for `field`, qualified with
/// the model's table.
pub(crate) struct ColumnRef {
    pub(crate) model: Path,
    pub(crate) field: Ident,
}

impl ColumnRef {
    /// Reads `path` as a column reference when it is `Model::field`: two or
    /// more segments, the last a snake_case field name.
    fn from_path(path: &Path) -> Option<Self> {
        if path.segments.len() < 2 {
            return None;
        }
        let field = path.segments.last()?.ident.clone();
        if !field
            .to_string()
            .starts_with(|c: char| c.is_ascii_lowercase())
        {
            return None;
        }
        let mut model = path.clone();
        model.segments.pop();
        model.segments.pop_punct();
        Some(Self { model, field })
    }

    /// Refuses a reference to a model that isn't one of `allowed`: the
    /// model being derived and those it joins.
    pub(crate) fn check_model(&self, allowed: &[String]) -> syn::Result<()> {
        let model = self.model.to_token_stream().to_string();
        if allowed.contains(&model) {
            Ok(())
        } else {
            Err(syn::Error::new_spanned(
                &self.model,
                format!("`{model}` is not this model or one it joins; join it first"),
            ))
        }
    }

    /// An expression of the column's qualified name, a `&'static str`.
    pub(crate) fn sql(&self) -> TokenStream {
        let model = &self.model;
        let constant = column_const(&self.field);
        quote! { <#model>::#constant.qualified() }
    }
}

/// One SQL term of an `expr` or a `coalesce` list: a column reference, or
/// a SQL expression given as a string literal or a path to a `&str`
/// constant.
pub(crate) enum Term {
    Column(ColumnRef),
    Sql(Expr),
}

impl Term {
    pub(crate) fn check_model(&self, allowed: &[String]) -> syn::Result<()> {
        match self {
            Self::Column(column) => column.check_model(allowed),
            Self::Sql(_) => Ok(()),
        }
    }

    /// An expression of the term's SQL, a `&str`.
    pub(crate) fn sql(&self) -> TokenStream {
        match self {
            Self::Column(column) => column.sql(),
            Self::Sql(expr) => expr.to_token_stream(),
        }
    }
}

impl Parse for Term {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(LitStr) {
            let lit: LitStr = input.parse()?;
            return Ok(Self::Sql(Expr::Lit(ExprLit {
                attrs: Vec::new(),
                lit: Lit::Str(lit),
            })));
        }
        let path: Path = input.parse()?;
        Ok(match ColumnRef::from_path(&path) {
            Some(column) => Self::Column(column),
            None => Self::Sql(Expr::Path(ExprPath {
                attrs: Vec::new(),
                qself: None,
                path,
            })),
        })
    }
}

/// Parses the `(a, b, ...)` of a `coalesce(a, b, ...)`.
pub(crate) fn parse_coalesce(input: ParseStream) -> syn::Result<Vec<Term>> {
    let content;
    syn::parenthesized!(content in input);
    let terms: Punctuated<Term, Token![,]> = Punctuated::parse_terminated(&content)?;
    if terms.len() < 2 {
        return Err(content.error("`coalesce` needs at least two terms"));
    }
    Ok(terms.into_iter().collect())
}

/// An expression of `COALESCE(<terms>)`, a `String`.
pub(crate) fn coalesce_sql(terms: &[Term]) -> TokenStream {
    let terms = terms.iter().map(Term::sql);
    quote! { format!("COALESCE({})", [#(#terms),*].join(", ")) }
}

/// Parses a `from = ...` value: `Model::field`, or a model alone.
pub(crate) enum FromTarget {
    Column(ColumnRef),
    Model(Path),
}

impl Parse for FromTarget {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let path: Path = input.parse()?;
        Ok(match ColumnRef::from_path(&path) {
            Some(column) => Self::Column(column),
            None => Self::Model(path),
        })
    }
}
