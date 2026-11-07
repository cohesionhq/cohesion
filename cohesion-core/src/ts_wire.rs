//! TypeScript declarations of the wire protocol, generated from the types
//! themselves so clients (the web UI) never mirror them by hand.
//! `cargo run -p cohesion-core --example ts_client` prints the module.

use std::collections::{BTreeMap, HashMap};

use crate::invalidation::InvalidationBatch;
use crate::protocol::{Command, ErrorCode, PROTOCOL_VERSION, ServerError, Vocabulary};

/// `export` declarations by type name.
pub type Declarations = BTreeMap<&'static str, String>;

/// A type as its JSON looks on the wire. Implemented by `#[derive(TsWire)]`
/// for protocol types and models, and here for primitives and containers.
pub trait TsWire {
    /// The type where it is used, such as `Task[]` or `number | null`.
    fn ts_type() -> String;

    /// Adds this type's declaration, and those of the types it refers to,
    /// to `out`. Primitives and containers have none of their own.
    fn declare(_out: &mut Declarations) {}
}

macro_rules! primitive {
    ($ts:literal: $($ty:ty),*) => {
        $(impl TsWire for $ty {
            fn ts_type() -> String {
                $ts.to_string()
            }
        })*
    };
}

primitive!("number": i32, i64, u32, u64, usize, f64);
primitive!("string": String);
primitive!("boolean": bool);

/// Any JSON a script or client chose.
impl TsWire for serde_json::Value {
    fn ts_type() -> String {
        "unknown".to_string()
    }
}

impl<T: TsWire> TsWire for Option<T> {
    fn ts_type() -> String {
        format!("{} | null", T::ts_type())
    }

    fn declare(out: &mut Declarations) {
        T::declare(out);
    }
}

impl<T: TsWire> TsWire for Box<T> {
    fn ts_type() -> String {
        T::ts_type()
    }

    fn declare(out: &mut Declarations) {
        T::declare(out);
    }
}

impl<T: TsWire> TsWire for Vec<T> {
    fn ts_type() -> String {
        let element = T::ts_type();
        if element.contains(' ') {
            format!("({element})[]")
        } else {
            format!("{element}[]")
        }
    }

    fn declare(out: &mut Declarations) {
        T::declare(out);
    }
}

impl<V: TsWire> TsWire for HashMap<String, V> {
    fn ts_type() -> String {
        format!("Record<string, {}>", V::ts_type())
    }

    fn declare(out: &mut Declarations) {
        V::declare(out);
    }
}

impl<V: TsWire> TsWire for BTreeMap<String, V> {
    fn ts_type() -> String {
        format!("Record<string, {}>", V::ts_type())
    }

    fn declare(out: &mut Declarations) {
        V::declare(out);
    }
}

/// The TypeScript module clients import: every type a command or frame
/// carries, then `Requests` and `Replies`, which map each request `type` to
/// its `args` and its reply's `data`.
pub fn client_module() -> String {
    let mut out = Declarations::new();
    let maps = Command::declare_ts(&mut out);
    ServerError::declare(&mut out);
    ErrorCode::declare(&mut out);
    InvalidationBatch::declare(&mut out);
    Vocabulary::declare(&mut out);
    let mut module =
        String::from("// Generated from cohesion-core by `npm run generate`; do not edit.\n\n");
    module += &format!("export const PROTOCOL_VERSION = {PROTOCOL_VERSION};\n\n");
    module += "/** Marks a request that carries no `args` key on the wire. */\n";
    module += "export type NoArgs = undefined;\n\n";
    for declaration in out.values() {
        module += declaration;
        module += "\n\n";
    }
    module + &maps
}
