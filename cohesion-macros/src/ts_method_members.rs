//! Reads the members of an rquickjs `methods` block for a TypeScript
//! declaration. Where the written type says too little (a `Value<'js>`
//! argument, say), `#[ts(...)]` overrides it; those attributes are read and
//! removed here, so rquickjs never sees them.

use syn::{Attribute, Expr, ExprLit, FnArg, ImplItem, Lit, LitStr, Pat, ReturnType};

use crate::identifier_case::js_field_name;
use crate::ts_declaration::{Members, Method, Prop, doc_text};
use crate::ts_type_mapping::{param, ts_type};

/// `#[ts(type = "...")]` on an argument, `#[ts(returns = "...")]`,
/// `#[ts(generics = "...")]` or `#[ts(skip)]` on a method.
#[derive(Default)]
struct TsAttr {
    ty: Option<String>,
    returns: Option<String>,
    generics: Option<String>,
    skip: bool,
}

/// The parts of rquickjs's `#[qjs(...)]` that change what scripts see.
#[derive(Default)]
struct QjsAttr {
    get: bool,
    set: bool,
    skip: bool,
    is_static: bool,
    constructor: bool,
    rename: Option<String>,
    /// Renamed to something that is not a string, such as a well-known symbol.
    symbol: bool,
}

/// Reads the members of an rquickjs `methods` block on top of `props` (an
/// entity's columns), removing every `#[ts(...)]` attribute as it goes.
pub fn members(items: &mut [ImplItem], props: Vec<Prop>) -> syn::Result<Members> {
    let mut members = Members {
        props,
        methods: Vec::new(),
    };
    for item in items {
        let ImplItem::Fn(function) = item else {
            continue;
        };
        let ts = take_ts_attr(&mut function.attrs)?;
        if ts.ty.is_some() {
            return Err(syn::Error::new_spanned(
                &function.sig.ident,
                "`#[ts(type = ...)]` goes on an argument; use `#[ts(returns = ...)]` on a method",
            ));
        }
        let mut params = Vec::new();
        for (index, input) in function.sig.inputs.iter_mut().enumerate() {
            let FnArg::Typed(arg) = input else {
                continue;
            };
            let arg_ts = take_ts_attr(&mut arg.attrs)?;
            if arg_ts.returns.is_some() || arg_ts.generics.is_some() || arg_ts.skip {
                return Err(syn::Error::new_spanned(
                    &arg.pat,
                    "an argument only takes `#[ts(type = \"...\")]`",
                ));
            }
            let name = match &*arg.pat {
                Pat::Ident(pat) => js_field_name(pat.ident.to_string().trim_start_matches('_')),
                _ => format!("arg{index}"),
            };
            if let Some(param) = param(&name, &arg.ty, arg_ts.ty) {
                params.push(param);
            }
        }

        let qjs = read_qjs(&function.attrs)?;
        if ts.skip || qjs.skip || qjs.constructor || qjs.symbol {
            continue;
        }
        let name = qjs.rename.unwrap_or_else(|| function.sig.ident.to_string());
        let docs = doc_text(&function.attrs);
        let returns = ts.returns.unwrap_or_else(|| match &function.sig.output {
            ReturnType::Default => "void".to_string(),
            ReturnType::Type(_, ty) => ts_type(ty),
        });

        if qjs.get {
            match members.props.iter_mut().find(|p| p.name == name) {
                Some(prop) => {
                    prop.ty = returns;
                    if prop.docs.is_empty() {
                        prop.docs = docs;
                    }
                }
                None => members.props.push(Prop {
                    name,
                    ty: returns,
                    writable: false,
                    docs,
                }),
            }
        } else if qjs.set {
            // The value's type, without `name: ` (or `name?: `).
            let ty = params
                .first()
                .and_then(|p| p.split_once(": ").map(|(_, ty)| ty.to_string()))
                .unwrap_or_else(|| "unknown".to_string());
            match members.props.iter_mut().find(|p| p.name == name) {
                Some(prop) => {
                    prop.writable = true;
                    // The column's docs say what the value is; the setter's
                    // say what assigning it does, so keep both.
                    if prop.docs.is_empty() {
                        prop.docs = docs;
                    } else if !docs.is_empty() {
                        prop.docs = format!("{}\n\n{docs}", prop.docs);
                    }
                }
                None => members.props.push(Prop {
                    name,
                    ty,
                    writable: true,
                    docs,
                }),
            }
        } else {
            members.methods.push(Method {
                name,
                generics: ts.generics.unwrap_or_default(),
                params,
                returns,
                docs,
                is_static: qjs.is_static,
            });
        }
    }
    Ok(members)
}

/// Reads and removes `#[ts(...)]`.
fn take_ts_attr(attrs: &mut Vec<Attribute>) -> syn::Result<TsAttr> {
    let mut ts = TsAttr::default();
    let mut kept = Vec::with_capacity(attrs.len());
    for attr in attrs.drain(..) {
        if !attr.path().is_ident("ts") {
            kept.push(attr);
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("type") {
                ts.ty = Some(meta.value()?.parse::<LitStr>()?.value());
            } else if meta.path.is_ident("returns") {
                ts.returns = Some(meta.value()?.parse::<LitStr>()?.value());
            } else if meta.path.is_ident("generics") {
                ts.generics = Some(meta.value()?.parse::<LitStr>()?.value());
            } else if meta.path.is_ident("skip") {
                ts.skip = true;
            } else {
                return Err(meta.error(
                    "expected `type = \"...\"`, `returns = \"...\"`, `generics = \"...\"` or `skip`",
                ));
            }
            Ok(())
        })?;
    }
    *attrs = kept;
    Ok(ts)
}

fn read_qjs(attrs: &[Attribute]) -> syn::Result<QjsAttr> {
    let mut qjs = QjsAttr::default();
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("qjs")) {
        attr.parse_nested_meta(|meta| {
            let key = meta
                .path
                .get_ident()
                .map(ToString::to_string)
                .unwrap_or_default();
            match key.as_str() {
                "get" => qjs.get = true,
                "set" => qjs.set = true,
                "skip" => qjs.skip = true,
                "static" => qjs.is_static = true,
                "constructor" => qjs.constructor = true,
                "rename" => match meta.value()?.parse::<Expr>()? {
                    Expr::Lit(ExprLit {
                        lit: Lit::Str(name),
                        ..
                    }) => qjs.rename = Some(name.value()),
                    _ => qjs.symbol = true,
                },
                // Flags that do not change the declaration, such as `enumerable`.
                _ => {
                    if meta.input.peek(syn::Token![=]) {
                        meta.value()?.parse::<Expr>()?;
                    }
                }
            }
            Ok(())
        })?;
    }
    Ok(qjs)
}
