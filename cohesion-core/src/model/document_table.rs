//! Tables: a document folder with properties is a table, and each document
//! in it is a row. A folder's [`DocumentFolderProperty`]s are its columns,
//! in order, each of one [`PropertyDataType`], and a document holds at most
//! one [`DocumentValue`] per property of its folder. A value is stored in
//! every representation its type honestly gives (its text always, a number
//! when it is one), so filtering and sorting happen in SQL on the column its
//! type names. A missing value is empty: everything is nullable. A row's
//! title is its document's. A choice property's values pick from its
//! [`PropertyOption`]s, which only a choice property has.

use cohesion_macros::{Model, TextEnum, TsWire};
use serde::{Deserialize, Serialize};

use crate::model::IntelligenceAgentColor;
use crate::stored_model::{Column, ColumnGroup};
use crate::value::{IntoValue, Json, Row, RowError, Value, read_column};

/// What kind of value a property holds; stored as text.
///
/// - `"text"`: any text.
/// - `"integer"`: a whole number.
/// - `"decimal"`: a number with a fractional part.
/// - `"currency"`: an amount of money, a decimal number.
/// - `"checkbox"`: checked or not.
/// - `"date"`: a day, as `"YYYY-MM-DD"`.
/// - `"choice"`: one of the property's options. Never a formula's.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
pub enum PropertyDataType {
    Text,
    Integer,
    Decimal,
    Currency,
    Checkbox,
    Date,
    Choice,
}

impl PropertyDataType {
    /// Whether values of this type compare as greater or less than each other
    /// (numbers and days), not only as equal.
    pub fn is_ordered(self) -> bool {
        match self {
            Self::Integer | Self::Decimal | Self::Currency | Self::Date => true,
            Self::Text | Self::Checkbox | Self::Choice => false,
        }
    }
}

/// A property's type as it is read, with what only one type has: a choice
/// property's options. Flattened into its property on the wire, so
/// `data_type` names the type and a choice property also carries `options`:
/// `{"data_type": "text"}`, `{"data_type": "choice", "options": [...]}`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire)]
#[serde(tag = "data_type", rename_all = "camelCase")]
pub enum PropertyType {
    Text,
    Integer,
    Decimal,
    Currency,
    Checkbox,
    Date,
    Choice {
        /// The options its values pick from, in their order.
        options: Vec<PropertyOption>,
    },
}

impl PropertyType {
    /// The type alone, without its options.
    pub fn data_type(&self) -> PropertyDataType {
        match self {
            Self::Text => PropertyDataType::Text,
            Self::Integer => PropertyDataType::Integer,
            Self::Decimal => PropertyDataType::Decimal,
            Self::Currency => PropertyDataType::Currency,
            Self::Checkbox => PropertyDataType::Checkbox,
            Self::Date => PropertyDataType::Date,
            Self::Choice { .. } => PropertyDataType::Choice,
        }
    }

    /// A choice property's options in their order; none for any other type.
    pub fn options(&self) -> &[PropertyOption] {
        match self {
            Self::Choice { options } => options,
            _ => &[],
        }
    }
}

/// The SQL reading a property's options as a JSON array, in their order.
const OPTIONS_SQL: &str = "(SELECT json_group_array(json_object('id', o.id, \
    'property_id', o.property_id, 'name', o.name, 'color', o.color, \
    'sort_order', o.sort_order)) \
    FROM (SELECT * FROM document_property_options o \
    WHERE o.property_id = document_folder_properties.id ORDER BY o.sort_order, o.id) o)";

/// A property's type as its `data_type` column, and a choice property's
/// options from `document_property_options`, read alongside it.
impl ColumnGroup for PropertyType {
    const COLUMNS: &'static [&'static str] = &["data_type"];
    const READS: &'static [(&'static str, &'static str)] = &[("choice_options", OPTIONS_SQL)];

    fn from_row(row: &dyn Row) -> Result<Self, RowError> {
        let data_type: PropertyDataType = read_column(row, "data_type")?;
        Ok(match data_type {
            PropertyDataType::Text => Self::Text,
            PropertyDataType::Integer => Self::Integer,
            PropertyDataType::Decimal => Self::Decimal,
            PropertyDataType::Currency => Self::Currency,
            PropertyDataType::Checkbox => Self::Checkbox,
            PropertyDataType::Date => Self::Date,
            PropertyDataType::Choice => Self::Choice {
                options: read_column::<Json<Vec<PropertyOption>>>(row, "choice_options")?.0,
            },
        })
    }

    fn into_values(self) -> Vec<Value> {
        vec![self.data_type().into_value()]
    }
}

/// A column of a document folder's table: a name and the type of value it
/// holds. A folder with at least one is a table; the Inbox never has one.
#[derive(Clone, Debug, Serialize, Deserialize, Model, TsWire)]
#[model(table = "document_folder_properties", backup = Included)]
pub struct DocumentFolderProperty {
    pub id: i64,
    pub folder_id: i64,
    pub name: String,
    /// The type of the property's values, `data_type` on the wire, with a
    /// choice property's options; a formula's outputs must be of it.
    #[serde(flatten)]
    #[model(group, reads)]
    pub kind: PropertyType,
    /// Where the property sits among its folder's; lower comes first.
    pub sort_order: i64,
    /// What computes the property's values, or `null` for a property whose
    /// values are set by hand.
    #[model(group)]
    pub formula: Option<PropertyFormula>,
}

impl DocumentFolderProperty {
    /// The `data_type` column, which [`kind`](Self::kind) is read from: what
    /// a write sets. Options are written with their own commands.
    pub const DATA_TYPE: Column<Self, PropertyDataType> =
        Column::new("data_type", "document_folder_properties.data_type");

    /// The type of the property's values.
    pub fn data_type(&self) -> PropertyDataType {
        self.kind.data_type()
    }

    /// The option `option_id` of a choice property, if it has one by that id.
    pub fn option(&self, option_id: i64) -> Option<&PropertyOption> {
        self.kind
            .options()
            .iter()
            .find(|option| option.id == option_id)
    }
}

/// One of the options a choice property's values pick from: a name, unique
/// ignoring case among its property's, a color, and its place among them.
/// Only a choice property has options; they go when it stops being one. A
/// value names its option by id, so renaming one changes no value, and
/// deleting one empties the values that held it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Model, TsWire)]
#[model(table = "document_property_options", backup = Included)]
pub struct PropertyOption {
    pub id: i64,
    pub property_id: i64,
    pub name: String,
    /// The color it is drawn in, or `null` for none, drawn like a tag.
    pub color: Option<IntelligenceAgentColor>,
    /// Where the option sits among its property's; lower comes first.
    pub sort_order: i64,
}

/// A property whose value in each row is computed by a JavaScript function
/// of the row, such as `(row) => row.Price * row.Quantity`. The row maps
/// each property that isn't a formula, by name, to its value (the
/// document's title and contents aren't in it): a string for
/// text and dates (`"YYYY-MM-DD"`), a number for whole numbers, decimals and
/// currency, a boolean for a checkbox, and `null` when empty. The function
/// returns a value of the property's type (the same shapes), or `null` or
/// `undefined` for an empty cell. Its outputs are stored like any value, so
/// rows filter and sort on them; the daemon computes them again whenever
/// the row's values, the folder's properties or the source change, and
/// every hour as well when its [`FormulaUpdateMode`] is `continuously`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TsWire)]
pub struct PropertyFormula {
    /// The function, in JavaScript or TypeScript.
    pub source: String,
    /// When its outputs are computed again.
    pub update_mode: FormulaUpdateMode,
    /// Why the last computation failed for a row (the function threw, ran
    /// too long or returned a value of another type), leaving that cell
    /// empty, or `null` once every row computes.
    pub error: Option<String>,
}

/// When a formula's outputs are computed again; stored as text.
///
/// - `"whenChanged"`: whenever the row's values, the folder's properties or
///   the source change.
/// - `"continuously"`: as `whenChanged`, and every hour as well, so a
///   formula that reads the clock (days until a date) stays current.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
#[text_enum(rename_all = "camelCase")]
pub enum FormulaUpdateMode {
    WhenChanged,
    Continuously,
}

/// A property's formula as three columns, `formula_source`,
/// `formula_update_mode` and `formula_error`; the table's checks keep a
/// mode and an error from outliving the source, and a source from going
/// without a mode.
impl ColumnGroup for Option<PropertyFormula> {
    const COLUMNS: &'static [&'static str] =
        &["formula_source", "formula_update_mode", "formula_error"];

    fn from_row(row: &dyn Row) -> Result<Self, RowError> {
        let source: Option<String> = read_column(row, "formula_source")?;
        let update_mode: Option<FormulaUpdateMode> = read_column(row, "formula_update_mode")?;
        let error: Option<String> = read_column(row, "formula_error")?;
        Ok(match (source, update_mode) {
            (Some(source), Some(update_mode)) => Some(PropertyFormula {
                source,
                update_mode,
                error,
            }),
            _ => None,
        })
    }

    fn into_values(self) -> Vec<Value> {
        match self {
            Some(formula) => vec![
                formula.source.into_value(),
                formula.update_mode.into_value(),
                formula.error.into_value(),
            ],
            None => vec![Value::Null, Value::Null, Value::Null],
        }
    }
}

/// One document's value for one property of its folder, as stored:
/// `text_value` always (empty for a choice), `integer_value` and
/// `real_value` where the value's type gives them ([`PropertyValue::integer`],
/// [`PropertyValue::real`]), and a choice's `option_id`, which the schema
/// holds to an option of the same property. A document has no value for a property it
/// holds nothing in, and none at all outside a table: its values go when it
/// leaves the folder.
#[derive(Clone, Debug, Model)]
#[model(table = "document_values", backup = Included)]
pub struct DocumentValue {
    pub id: i64,
    pub folder_id: i64,
    pub document_id: i64,
    pub property_id: i64,
    pub text_value: String,
    pub integer_value: Option<i64>,
    pub real_value: Option<f64>,
    pub option_id: Option<i64>,
}

impl DocumentValue {
    /// The value as a property of `data_type` reads it, or `None` when what
    /// is stored doesn't give that type.
    pub fn value(&self, data_type: PropertyDataType) -> Option<PropertyValue> {
        Some(match data_type {
            PropertyDataType::Text => PropertyValue::Text(self.text_value.clone()),
            PropertyDataType::Integer => PropertyValue::Integer(self.integer_value?),
            PropertyDataType::Decimal => PropertyValue::Decimal(self.real_value?),
            PropertyDataType::Currency => PropertyValue::Currency(self.real_value?),
            PropertyDataType::Checkbox => PropertyValue::Checkbox(self.integer_value? != 0),
            PropertyDataType::Date => PropertyValue::Date(self.text_value.clone()),
            PropertyDataType::Choice => PropertyValue::Choice(self.option_id?),
        })
    }
}

/// A value in a table, tagged with its type, which must be the type of
/// the property it is written to.
///
/// - `{"type": "text", "value": "Ada"}`; empty text is no value.
/// - `{"type": "integer", "value": 42}`
/// - `{"type": "decimal", "value": 3.5}`
/// - `{"type": "currency", "value": 12.99}`
/// - `{"type": "checkbox", "value": true}`
/// - `{"type": "date", "value": "2026-11-09"}`
/// - `{"type": "choice", "value": 7}`: the id of one of the property's
///   options.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum PropertyValue {
    Text(String),
    Integer(i64),
    Decimal(f64),
    Currency(f64),
    Checkbox(bool),
    Date(String),
    Choice(i64),
}

impl PropertyValue {
    /// The type of property this value fits.
    pub fn data_type(&self) -> PropertyDataType {
        match self {
            Self::Text(_) => PropertyDataType::Text,
            Self::Integer(_) => PropertyDataType::Integer,
            Self::Decimal(_) => PropertyDataType::Decimal,
            Self::Currency(_) => PropertyDataType::Currency,
            Self::Checkbox(_) => PropertyDataType::Checkbox,
            Self::Date(_) => PropertyDataType::Date,
            Self::Choice(_) => PropertyDataType::Choice,
        }
    }

    /// The value as it may be stored: `Ok(None)` for empty text, which is no
    /// value, and an error saying what is wrong for a number that isn't
    /// finite or a date that isn't a real `YYYY-MM-DD` day.
    pub fn checked(self) -> Result<Option<Self>, &'static str> {
        match &self {
            Self::Text(text) if text.is_empty() => Ok(None),
            Self::Decimal(number) | Self::Currency(number) if !number.is_finite() => {
                Err("A number must be finite.")
            }
            Self::Date(day) if !is_day(day) => Err("A date must be a real day, as YYYY-MM-DD."),
            _ => Ok(Some(self)),
        }
    }

    /// The value as text: what is stored in `text_value`, and what
    /// [`PropertyValue::parse`] reads back. A choice's text is its option's
    /// name, which the value doesn't hold, so it stores none.
    pub fn text(&self) -> String {
        match self {
            Self::Text(text) | Self::Date(text) => text.clone(),
            Self::Integer(number) => number.to_string(),
            Self::Decimal(number) | Self::Currency(number) => number.to_string(),
            Self::Checkbox(checked) => checked.to_string(),
            Self::Choice(_) => String::new(),
        }
    }

    /// The value as a whole number, where its type is one: an integer, or a
    /// checkbox as 1 or 0.
    pub fn integer(&self) -> Option<i64> {
        match self {
            Self::Integer(number) => Some(*number),
            Self::Checkbox(checked) => Some(i64::from(*checked)),
            Self::Text(_)
            | Self::Decimal(_)
            | Self::Currency(_)
            | Self::Date(_)
            | Self::Choice(_) => None,
        }
    }

    /// The option a choice names.
    pub fn option_id(&self) -> Option<i64> {
        match self {
            Self::Choice(option_id) => Some(*option_id),
            _ => None,
        }
    }

    /// The value as a number, where its type is one: a decimal, an amount,
    /// or an integer (exact up to 2^53).
    pub fn real(&self) -> Option<f64> {
        match self {
            Self::Decimal(number) | Self::Currency(number) => Some(*number),
            Self::Integer(number) => Some(*number as f64),
            Self::Text(_) | Self::Checkbox(_) | Self::Date(_) | Self::Choice(_) => None,
        }
    }

    /// `text`, a value's [`PropertyValue::text`], read as a value of
    /// `data_type`, or `None` when it isn't one: how a value survives its
    /// property changing type. `"42"` is an integer, a decimal, an amount or
    /// text; `"3.5"` no integer; `"true"` only a checkbox or text. No text is
    /// a choice by itself: a property becoming one makes an option of each
    /// distinct text.
    pub fn parse(data_type: PropertyDataType, text: &str) -> Option<Self> {
        let number = || text.parse::<f64>().ok().filter(|number| number.is_finite());
        let value = match data_type {
            PropertyDataType::Text => Self::Text(text.to_string()),
            PropertyDataType::Integer => Self::Integer(text.parse().ok()?),
            PropertyDataType::Decimal => Self::Decimal(number()?),
            PropertyDataType::Currency => Self::Currency(number()?),
            PropertyDataType::Checkbox => Self::Checkbox(text.parse().ok()?),
            PropertyDataType::Date => Self::Date(text.to_string()),
            PropertyDataType::Choice => return None,
        };
        value.checked().ok().flatten()
    }
}

/// Whether `text` is a real day written `YYYY-MM-DD`, as SQLite's `date()`
/// reads one.
fn is_day(text: &str) -> bool {
    let bytes = text.as_bytes();
    let digits = |range: std::ops::Range<usize>| {
        bytes[range.clone()]
            .iter()
            .all(u8::is_ascii_digit)
            .then(|| {
                text[range]
                    .parse::<u32>()
                    .expect("ASCII digits are a number")
            })
    };
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let (Some(year), Some(month), Some(day)) = (digits(0..4), digits(5..7), digits(8..10)) else {
        return false;
    };
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day)
}

/// One document's value for one property, on the wire.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TsWire)]
pub struct DocumentCell {
    pub property_id: i64,
    pub value: PropertyValue,
}

/// A document in a table, as a row: its title and one cell per property it
/// holds a value in, in the properties' order; a property with no cell is
/// empty.
#[derive(Clone, Debug, Serialize, Deserialize, TsWire)]
pub struct TableRow {
    pub document_id: i64,
    pub folder_id: i64,
    pub title: String,
    pub cells: Vec<DocumentCell>,
}

#[cfg(test)]
mod tests;
