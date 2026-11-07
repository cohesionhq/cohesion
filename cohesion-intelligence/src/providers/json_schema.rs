use std::collections::BTreeMap;

use serde::Serialize;

use crate::tool_definition::{Field, FieldKind, Tool};

/// A tool's parameters as JSON Schema, shared by every backend.
#[derive(Serialize)]
pub(super) struct JsonSchemaObject<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    properties: BTreeMap<&'a str, JsonSchemaProperty<'a>>,
    required: Vec<&'a str>,
}
impl<'a> From<&'a Tool> for JsonSchemaObject<'a> {
    fn from(tool: &'a Tool) -> Self {
        JsonSchemaObject {
            kind: "object",
            properties: tool
                .fields
                .iter()
                .map(|field| (field.name, JsonSchemaProperty::from(field)))
                .collect(),
            required: tool
                .fields
                .iter()
                .filter(|field| field.required)
                .map(|field| field.name)
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct JsonSchemaProperty<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    description: &'a str,
    /// An array's element type.
    #[serde(skip_serializing_if = "Option::is_none")]
    items: Option<JsonSchemaItems>,
}
impl<'a> From<&'a Field> for JsonSchemaProperty<'a> {
    fn from(field: &'a Field) -> Self {
        let (kind, items) = match field.kind {
            FieldKind::String => ("string", None),
            FieldKind::Integer => ("integer", None),
            FieldKind::StringList => ("array", Some(JsonSchemaItems { kind: "string" })),
            FieldKind::IntegerList => ("array", Some(JsonSchemaItems { kind: "integer" })),
            FieldKind::Boolean => ("boolean", None),
        };
        JsonSchemaProperty {
            kind,
            description: field.description,
            items,
        }
    }
}

#[derive(Serialize)]
struct JsonSchemaItems {
    #[serde(rename = "type")]
    kind: &'static str,
}
