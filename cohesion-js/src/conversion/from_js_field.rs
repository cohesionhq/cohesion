use std::collections::HashMap;

use cohesion_core::model::NewContactMethod;
use cohesion_core::value::ValueError;
use cohesion_core::{DayFilter, DayRange, TaskAssignee, TaskSize};
use rquickjs::{Array, Ctx, Exception, FromJs, Object, Value};

use super::throw_value_error;

/// A field of a `#[derive(JsOptions)]` struct, read from a script's value.
/// Every field type needs one; a missing impl is a compile error naming it.
pub trait FromJsField: Sized {
    fn from_js_field<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> rquickjs::Result<Self>;
}

from_js_field_via_from_js!(i64, f64, bool, String);

/// `undefined` (a property left out) and `null` are `None`.
impl<T: FromJsField> FromJsField for Option<T> {
    fn from_js_field<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> rquickjs::Result<Self> {
        if value.is_undefined() || value.is_null() {
            Ok(None)
        } else {
            T::from_js_field(ctx, value).map(Some)
        }
    }
}

impl<T: FromJsField> FromJsField for Vec<T> {
    fn from_js_field<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> rquickjs::Result<Self> {
        Array::from_js(ctx, value)?
            .iter::<Value>()
            .map(|element| T::from_js_field(ctx, element?))
            .collect()
    }
}

/// A plain object's own enumerable properties; declared to scripts as
/// `Record<string, T>`.
impl<T: FromJsField> FromJsField for HashMap<String, T> {
    fn from_js_field<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> rquickjs::Result<Self> {
        Object::from_js(ctx, value)?
            .props::<String, Value>()
            .map(|entry| {
                let (key, value) = entry?;
                Ok((key, T::from_js_field(ctx, value)?))
            })
            .collect()
    }
}

/// Any JSON-serializable value, converted with the engine's `JSON.stringify`
/// so it matches what a script would send; declared to scripts as `unknown`.
impl FromJsField for serde_json::Value {
    fn from_js_field<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> rquickjs::Result<Self> {
        let Some(text) = ctx.json_stringify(value)? else {
            return Err(Exception::throw_type(ctx, "value is not JSON-serializable"));
        };
        serde_json::from_str(&text.to_string()?)
            .map_err(|error| Exception::throw_type(ctx, &error.to_string()))
    }
}

/// A whole number from 1 to 5; anything else throws a `TypeError`.
impl FromJsField for TaskSize {
    fn from_js_field<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> rquickjs::Result<Self> {
        let number = f64::from_js(ctx, value)?;
        let whole = (number.fract() == 0.0 && (0.0..=255.0).contains(&number))
            .then_some(number as u8)
            .ok_or_else(|| ValueError::Invalid(format!("a task size is 1 to 5, not {number}")));
        whole
            .and_then(TaskSize::try_from)
            .map_err(|error| throw_value_error(ctx, error))
    }
}

/// `{ kind: "user" }`, `{ kind: "agent", agentId }` or
/// `{ kind: "contact", contactId }`; any other `kind` throws a `TypeError`.
impl FromJsField for TaskAssignee {
    fn from_js_field<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> rquickjs::Result<Self> {
        let assignee = Object::from_js(ctx, value)?;
        match String::from_js(ctx, assignee.get("kind")?)?.as_str() {
            "user" => Ok(Self::User),
            "agent" => Ok(Self::Agent {
                agent_id: i64::from_js(ctx, assignee.get("agentId")?)?,
            }),
            "contact" => Ok(Self::Contact {
                contact_id: i64::from_js(ctx, assignee.get("contactId")?)?,
            }),
            other => Err(Exception::throw_type(
                ctx,
                &format!(
                    "an assignee's kind is \"user\", \"agent\" or \"contact\", not \"{other}\""
                ),
            )),
        }
    }
}

/// A day (`"2026-10-20"`), an array of days, or a `{ before, after }` range.
impl FromJsField for DayFilter {
    fn from_js_field<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> rquickjs::Result<Self> {
        if value.is_string() {
            String::from_js(ctx, value).map(Self::Day)
        } else if value.is_array() {
            Vec::from_js_field(ctx, value).map(Self::Days)
        } else {
            let range = Object::from_js(ctx, value)?;
            Ok(Self::Range(DayRange {
                before: Option::from_js_field(ctx, range.get("before")?)?,
                after: Option::from_js_field(ctx, range.get("after")?)?,
            }))
        }
    }
}

/// A `{ kind, value }` object; an unknown `kind` throws a `TypeError`.
impl FromJsField for NewContactMethod {
    fn from_js_field<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> rquickjs::Result<Self> {
        let method = Object::from_js(ctx, value)?;
        Ok(Self {
            kind: FromJsField::from_js_field(ctx, method.get("kind")?)?,
            value: String::from_js(ctx, method.get("value")?)?,
        })
    }
}
