//! Checks `#[derive(Model)]` against a real model,
//! through a backend-free `Row`.

use std::collections::HashMap;

use cohesion_core::value::{Row, RowError};
use cohesion_core::{Model, Value, Workspace};

struct MapRow(HashMap<&'static str, Value>);

impl Row for MapRow {
    fn get(&self, column: &str) -> Result<Value, RowError> {
        self.0
            .get(column)
            .cloned()
            .ok_or_else(|| RowError::MissingColumn(column.to_string()))
    }
}

#[test]
fn workspace_round_trips_through_a_row() {
    let row = MapRow(HashMap::from([
        ("id", Value::Integer(7)),
        ("title", Value::Text("Home".to_string())),
        ("emoji", Value::Null),
        ("sort_order", Value::Integer(2)),
        ("deleted", Value::Integer(0)),
    ]));
    let workspace = Workspace::from_row(&row).expect("row has every column");
    assert_eq!(workspace.id(), 7);
    assert_eq!(workspace.title, "Home");
    assert_eq!(workspace.emoji, None);
    assert_eq!(workspace.sort_order, 2);
    assert!(!workspace.deleted);
}
