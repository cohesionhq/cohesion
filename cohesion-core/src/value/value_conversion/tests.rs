use super::*;
use crate::TextEnum;

/// Falls back to `Unknown` on unrecognized text.
#[derive(Debug, PartialEq, TextEnum)]
#[text_enum(default = Unknown)]
enum WithDefault {
    Known,
    Unknown,
}

#[test]
fn text_enum_reads_known_text() {
    let value = WithDefault::from_value(Value::Text("known".into()));
    assert_eq!(value.unwrap(), WithDefault::Known);
    assert_eq!(WithDefault::Known.into_value(), Value::Text("known".into()));
}

#[test]
fn text_enum_unknown_text_falls_back_to_the_default() {
    let value = WithDefault::from_value(Value::Text("archived".into()));
    assert_eq!(value.unwrap(), WithDefault::Unknown);
}
