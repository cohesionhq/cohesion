use rquickjs::{Ctx, Value};

use super::from_js_field::FromJsField;

/// A single value or an array of them, as an options field; declared to
/// scripts as `T | T[]`.
pub enum OneOrMore<T> {
    One(T),
    Many(Vec<T>),
}

impl<T> OneOrMore<T> {
    pub fn into_vec(self) -> Vec<T> {
        match self {
            Self::One(one) => vec![one],
            Self::Many(many) => many,
        }
    }
}

impl<T: FromJsField> FromJsField for OneOrMore<T> {
    fn from_js_field<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> rquickjs::Result<Self> {
        if value.is_array() {
            Vec::<T>::from_js_field(ctx, value).map(Self::Many)
        } else {
            T::from_js_field(ctx, value).map(Self::One)
        }
    }
}
