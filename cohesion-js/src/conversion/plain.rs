use rquickjs::{Ctx, Value};

/// `value` as plain data, as serde serializes it: what the connector
/// functions receive and return, and what `cohesion.vocabulary()` returns.
pub(crate) fn plain<'js, T: serde::Serialize>(
    ctx: &Ctx<'js>,
    value: &T,
) -> rquickjs::Result<Value<'js>> {
    ctx.json_parse(serde_json::to_string(value).expect("a serde value serializes"))
}
