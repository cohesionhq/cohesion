//! Databases: tables of typed values in a workspace. A [`Database`] has
//! ordered [`DatabaseProperty`]s (its columns, each of one
//! [`DatabaseDataType`]) and [`DatabaseRow`]s, and a row holds at most one
//! [`DatabaseRowValue`] per property. A value is stored in every
//! representation its type honestly gives (its text always, a number when it
//! is one), so filtering and sorting happen in SQL on the column its type
//! names. A missing value is empty: everything is nullable.

use cohesion_macros::{Model, TextEnum, TsWire};
use serde::{Deserialize, Serialize};

use crate::stored_model::ColumnGroup;
use crate::value::{IntoValue, Row, RowError, Value, read_column};

/// A table of typed values in a workspace.
#[derive(Clone, Debug, Serialize, Deserialize, Model, TsWire)]
#[model(table = "databases", backup = Included)]
pub struct Database {
    pub id: i64,
    pub workspace_id: i64,
    pub title: String,
}

/// What kind of value a database property holds; stored as text.
///
/// - `"text"`: any text.
/// - `"integer"`: a whole number.
/// - `"decimal"`: a number with a fractional part.
/// - `"currency"`: an amount of money, a decimal number.
/// - `"checkbox"`: checked or not.
/// - `"date"`: a day, as `"YYYY-MM-DD"`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
pub enum DatabaseDataType {
    Text,
    Integer,
    Decimal,
    Currency,
    Checkbox,
    Date,
}

impl DatabaseDataType {
    /// Whether values of this type compare as greater or less than each other
    /// (numbers and days), not only as equal.
    pub fn is_ordered(self) -> bool {
        match self {
            Self::Integer | Self::Decimal | Self::Currency | Self::Date => true,
            Self::Text | Self::Checkbox => false,
        }
    }
}

/// A column of a database: a name and the type of value it holds.
#[derive(Clone, Debug, Serialize, Deserialize, Model, TsWire)]
#[model(table = "database_properties", backup = Included)]
pub struct DatabaseProperty {
    pub id: i64,
    pub database_id: i64,
    pub name: String,
    /// The type of the property's values; a formula's outputs must be of it.
    pub data_type: DatabaseDataType,
    /// Where the property sits among its database's; lower comes first.
    pub sort_order: i64,
    /// What computes the property's values, or `null` for a property whose
    /// values are set by hand.
    #[model(group)]
    pub formula: Option<DatabaseFormula>,
}

/// A property whose value in each row is computed by a JavaScript function
/// of the row, such as `(row) => row.Price * row.Quantity`. The row maps
/// each property that isn't a formula, by name, to its value: a string for
/// text and dates (`"YYYY-MM-DD"`), a number for whole numbers, decimals and
/// currency, a boolean for a checkbox, and `null` when empty. The function
/// returns a value of the property's type (the same shapes), or `null` or
/// `undefined` for an empty cell. Its outputs are stored like any value, so
/// rows filter and sort on them; the daemon computes them again whenever
/// the row's values, the database's properties or the source change.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TsWire)]
pub struct DatabaseFormula {
    /// The function, in JavaScript or TypeScript.
    pub source: String,
    /// Why the last computation failed for a row (the function threw, ran
    /// too long or returned a value of another type), leaving that cell
    /// empty, or `null` once every row computes.
    pub error: Option<String>,
}

/// A property's formula as two columns, `formula_source` and
/// `formula_error`; the table's check keeps an error from outliving its
/// source.
impl ColumnGroup for Option<DatabaseFormula> {
    const COLUMNS: &'static [&'static str] = &["formula_source", "formula_error"];

    fn from_row(row: &dyn Row) -> Result<Self, RowError> {
        let source: Option<String> = read_column(row, "formula_source")?;
        let error: Option<String> = read_column(row, "formula_error")?;
        Ok(source.map(|source| DatabaseFormula { source, error }))
    }

    fn into_values(self) -> Vec<Value> {
        let (source, error) = match self {
            Some(DatabaseFormula { source, error }) => (Some(source), error),
            None => (None, None),
        };
        vec![source.into_value(), error.into_value()]
    }
}

/// An entry of a database, which holds its values; it may have a document
/// of its own.
#[derive(Clone, Debug, Serialize, Deserialize, Model, TsWire)]
#[model(table = "database_rows", backup = Included)]
pub struct DatabaseRow {
    pub id: i64,
    pub database_id: i64,
    /// The row's own document, of type `"databaseRow"`, or `null` when it has
    /// none. It never shows in the document list and goes with the row.
    pub document_id: Option<i64>,
}

/// One row's value for one property, as stored: `text_value` always, and
/// `integer_value` and `real_value` where the value's type gives them
/// ([`DatabaseValue::integer`], [`DatabaseValue::real`]). A row has no value
/// for a property it holds nothing in.
#[derive(Clone, Debug, Model)]
#[model(table = "database_row_values", backup = Included)]
pub struct DatabaseRowValue {
    pub id: i64,
    pub database_id: i64,
    pub row_id: i64,
    pub property_id: i64,
    pub text_value: String,
    pub integer_value: Option<i64>,
    pub real_value: Option<f64>,
}

impl DatabaseRowValue {
    /// The value as a property of `data_type` reads it, or `None` when what
    /// is stored doesn't give that type.
    pub fn value(&self, data_type: DatabaseDataType) -> Option<DatabaseValue> {
        Some(match data_type {
            DatabaseDataType::Text => DatabaseValue::Text(self.text_value.clone()),
            DatabaseDataType::Integer => DatabaseValue::Integer(self.integer_value?),
            DatabaseDataType::Decimal => DatabaseValue::Decimal(self.real_value?),
            DatabaseDataType::Currency => DatabaseValue::Currency(self.real_value?),
            DatabaseDataType::Checkbox => DatabaseValue::Checkbox(self.integer_value? != 0),
            DatabaseDataType::Date => DatabaseValue::Date(self.text_value.clone()),
        })
    }
}

/// A value in a database, tagged with its type, which must be the type of
/// the property it is written to.
///
/// - `{"type": "text", "value": "Ada"}`; empty text is no value.
/// - `{"type": "integer", "value": 42}`
/// - `{"type": "decimal", "value": 3.5}`
/// - `{"type": "currency", "value": 12.99}`
/// - `{"type": "checkbox", "value": true}`
/// - `{"type": "date", "value": "2026-11-09"}`
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum DatabaseValue {
    Text(String),
    Integer(i64),
    Decimal(f64),
    Currency(f64),
    Checkbox(bool),
    Date(String),
}

impl DatabaseValue {
    /// The type of property this value fits.
    pub fn data_type(&self) -> DatabaseDataType {
        match self {
            Self::Text(_) => DatabaseDataType::Text,
            Self::Integer(_) => DatabaseDataType::Integer,
            Self::Decimal(_) => DatabaseDataType::Decimal,
            Self::Currency(_) => DatabaseDataType::Currency,
            Self::Checkbox(_) => DatabaseDataType::Checkbox,
            Self::Date(_) => DatabaseDataType::Date,
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

    /// The value as text, which every type gives: what is stored in
    /// `text_value`, and what [`DatabaseValue::parse`] reads back.
    pub fn text(&self) -> String {
        match self {
            Self::Text(text) | Self::Date(text) => text.clone(),
            Self::Integer(number) => number.to_string(),
            Self::Decimal(number) | Self::Currency(number) => number.to_string(),
            Self::Checkbox(checked) => checked.to_string(),
        }
    }

    /// The value as a whole number, where its type is one: an integer, or a
    /// checkbox as 1 or 0.
    pub fn integer(&self) -> Option<i64> {
        match self {
            Self::Integer(number) => Some(*number),
            Self::Checkbox(checked) => Some(i64::from(*checked)),
            Self::Text(_) | Self::Decimal(_) | Self::Currency(_) | Self::Date(_) => None,
        }
    }

    /// The value as a number, where its type is one: a decimal, an amount,
    /// or an integer (exact up to 2^53).
    pub fn real(&self) -> Option<f64> {
        match self {
            Self::Decimal(number) | Self::Currency(number) => Some(*number),
            Self::Integer(number) => Some(*number as f64),
            Self::Text(_) | Self::Checkbox(_) | Self::Date(_) => None,
        }
    }

    /// `text`, a value's [`DatabaseValue::text`], read as a value of
    /// `data_type`, or `None` when it isn't one: how a value survives its
    /// property changing type. `"42"` is an integer, a decimal, an amount or
    /// text; `"3.5"` no integer; `"true"` only a checkbox or text.
    pub fn parse(data_type: DatabaseDataType, text: &str) -> Option<Self> {
        let number = || text.parse::<f64>().ok().filter(|number| number.is_finite());
        let value = match data_type {
            DatabaseDataType::Text => Self::Text(text.to_string()),
            DatabaseDataType::Integer => Self::Integer(text.parse().ok()?),
            DatabaseDataType::Decimal => Self::Decimal(number()?),
            DatabaseDataType::Currency => Self::Currency(number()?),
            DatabaseDataType::Checkbox => Self::Checkbox(text.parse().ok()?),
            DatabaseDataType::Date => Self::Date(text.to_string()),
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

/// One row's value for one property, on the wire.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, TsWire)]
pub struct DatabaseCell {
    pub property_id: i64,
    pub value: DatabaseValue,
}

/// A database row with its values: one cell per property it holds a value
/// in, in the properties' order; a property with no cell is empty.
#[derive(Clone, Debug, Serialize, Deserialize, TsWire)]
pub struct DatabaseRowEntry {
    pub id: i64,
    pub database_id: i64,
    /// The row's own document, or `null` when it has none.
    pub document_id: Option<i64>,
    pub cells: Vec<DatabaseCell>,
}

impl DatabaseRowEntry {
    /// What names the row where it shows on its own, as in an embed, since a
    /// row has no title: its value for the first text property of
    /// `properties`, its database's in their order, or `"Untitled"` when that
    /// is empty or the database has no text property.
    pub fn label(&self, properties: &[DatabaseProperty]) -> String {
        properties
            .iter()
            .find(|property| property.data_type == DatabaseDataType::Text)
            .and_then(|property| {
                self.cells
                    .iter()
                    .find(|cell| cell.property_id == property.id)
            })
            .map(|cell| cell.value.text())
            .unwrap_or_else(|| "Untitled".to_string())
    }
}

#[cfg(test)]
mod tests;
