use cohesion_core::model::{DatabaseDataType, DatabaseProperty, DatabaseValue};
use cohesion_core::protocol::{DatabaseCondition, DatabaseFilter, DatabaseSort};

use crate::commands::database::tests::Fixture;
use crate::test_support::recording_context;
use crate::{
    CommandError, DatabaseEventKind, DeleteDatabasePropertyCommand, DeleteDatabaseRowCommand,
    FormulaOutputs, FormulaProperty, ListDatabasePropertiesCommand, ListDatabaseRowsCommand,
    ListDatabasesWithStaleFormulasCommand, ListStaleDatabaseRowsCommand,
    RenameDatabasePropertyCommand, SetDatabasePropertyFormulaCommand, StaleDatabaseRow,
    StaleDatabaseRows, StoreDatabaseFormulaOutputsCommand,
};

use DatabaseDataType as T;
use DatabaseValue as V;

/// What a formula reads of a row: its non-formula values by name.
type Inputs = [(String, Option<V>)];

impl Fixture {
    fn stale(&self) -> StaleDatabaseRows {
        self.db
            .send_blocking(ListStaleDatabaseRowsCommand {
                database_id: self.database.id,
            })
            .unwrap()
    }

    /// The ids of the rows whose outputs are out of date.
    fn stale_ids(&self) -> Vec<i64> {
        self.stale()
            .rows
            .iter()
            .map(|row| row.staleness.id)
            .collect()
    }

    /// Stores what `compute` gives every formula for `stale`'s rows, as the
    /// daemon's pass would.
    fn store(
        &self,
        stale: StaleDatabaseRows,
        compute: impl Fn(&FormulaProperty, &Inputs) -> Result<Option<V>, String>,
    ) {
        let formulas = stale
            .formulas
            .into_iter()
            .map(|formula| FormulaOutputs {
                outputs: stale
                    .rows
                    .iter()
                    .map(|row: &StaleDatabaseRow| compute(&formula, &row.inputs))
                    .collect(),
                formula,
            })
            .collect();
        self.db
            .send_blocking(StoreDatabaseFormulaOutputsCommand {
                database_id: self.database.id,
                rows: stale.rows.iter().map(|row| row.staleness).collect(),
                formulas,
            })
            .unwrap();
    }

    /// Reads the outdated rows and stores what `compute` gives for them.
    fn pass(&self, compute: impl Fn(&FormulaProperty, &Inputs) -> Result<Option<V>, String>) {
        self.store(self.stale(), compute);
    }

    fn set_formula(&self, property: &DatabaseProperty, formula: Option<&str>) -> DatabaseProperty {
        self.db
            .send_blocking(SetDatabasePropertyFormulaCommand {
                workspace_id: self.ws,
                database_id: self.database.id,
                property_id: property.id,
                formula: formula.map(str::to_string),
            })
            .unwrap()
    }

    /// The property as it is now.
    fn reread(&self, property: &DatabaseProperty) -> DatabaseProperty {
        self.db
            .send_blocking(ListDatabasePropertiesCommand {
                workspace_id: self.ws,
                database_id: self.database.id,
            })
            .unwrap()
            .into_iter()
            .find(|p| p.id == property.id)
            .unwrap()
    }

    fn error(&self, property: &DatabaseProperty) -> Option<String> {
        self.reread(property).formula.unwrap().error
    }

    /// The value row `row_id` holds for `property`.
    fn value(&self, row_id: i64, property: &DatabaseProperty) -> Option<V> {
        self.cells(row_id)
            .into_iter()
            .find(|cell| cell.property_id == property.id)
            .map(|cell| cell.value)
    }
}

/// The number named `name` in `inputs`.
fn number(inputs: &Inputs, name: &str) -> Option<i64> {
    inputs
        .iter()
        .find(|(n, _)| n == name)
        .and_then(|(_, v)| match v {
            Some(V::Integer(number)) => Some(*number),
            _ => None,
        })
}

/// `Pages * 2`, as a stand-in for a formula's function.
fn doubled(_: &FormulaProperty, inputs: &Inputs) -> Result<Option<V>, String> {
    Ok(number(inputs, "Pages").map(|pages| V::Integer(pages * 2)))
}

/// A database of three rows with `Pages` 30, 10 and 20, and a formula
/// `Double` of it, computed.
fn pages() -> (Fixture, DatabaseProperty, DatabaseProperty, Vec<i64>) {
    let f = Fixture::new();
    let pages = f.property("Pages", T::Integer);
    let double = f.formula("Double", T::Integer, "(row) => row.Pages * 2");
    let rows: Vec<i64> = (0..3).map(|_| f.row()).collect();
    for (row, value) in rows.iter().zip([30, 10, 20]) {
        f.set(*row, &pages, Some(V::Integer(value))).unwrap();
    }
    f.pass(doubled);
    (f, pages, double, rows)
}

#[test]
fn a_formulas_outputs_are_stored_and_filter_and_sort_in_sql() {
    let (f, _, double, rows) = pages();
    assert!(f.stale_ids().is_empty());
    assert_eq!(f.value(rows[0], &double), Some(V::Integer(60)));
    let listed =
        f.db.send_blocking(ListDatabaseRowsCommand {
            workspace_id: f.ws,
            database_id: f.database.id,
            filters: vec![DatabaseFilter {
                property_id: double.id,
                condition: DatabaseCondition::GreaterThan {
                    value: V::Integer(30),
                },
            }],
            sort: Some(DatabaseSort {
                property_id: double.id,
                descending: false,
            }),
        })
        .unwrap();
    let ids: Vec<i64> = listed.iter().map(|row| row.id).collect();
    assert_eq!(ids, vec![rows[2], rows[0]]);
}

#[test]
fn a_formulas_values_cant_be_set() {
    let (f, _, double, rows) = pages();
    let refused = f.set(rows[0], &double, Some(V::Integer(1)));
    assert!(matches!(refused, Err(CommandError::Validation(_))));
    assert_eq!(f.value(rows[0], &double), Some(V::Integer(60)));
}

#[test]
fn a_value_set_outdates_its_row_alone() {
    let (f, pages, double, rows) = pages();
    f.set(rows[1], &pages, Some(V::Integer(11))).unwrap();
    assert_eq!(f.stale_ids(), vec![rows[1]]);
    f.pass(doubled);
    assert_eq!(f.value(rows[1], &double), Some(V::Integer(22)));
    assert!(f.stale_ids().is_empty());
}

#[test]
fn a_new_row_is_computed() {
    let (f, _, _, _) = pages();
    let row = f.row();
    assert_eq!(f.stale_ids(), vec![row]);
}

#[test]
fn changing_what_formulas_read_outdates_every_row() {
    let (f, pages, double, rows) = pages();
    let changes: [&dyn Fn(); 5] = [
        &|| {
            f.set_formula(&double, Some("(row) => row.Pages * 3"));
        },
        &|| {
            f.property("Notes", T::Text);
        },
        &|| f.retype(&pages, T::Decimal),
        &|| {
            f.db.send_blocking(RenameDatabasePropertyCommand {
                workspace_id: f.ws,
                database_id: f.database.id,
                property_id: pages.id,
                name: "Leaves".to_string(),
            })
            .unwrap();
        },
        &|| {
            f.db.send_blocking(DeleteDatabasePropertyCommand {
                workspace_id: f.ws,
                database_id: f.database.id,
                property_id: pages.id,
            })
            .unwrap();
        },
    ];
    for change in changes {
        change();
        assert_eq!(f.stale_ids(), rows);
        f.pass(|_, _| Ok(None));
        assert!(f.stale_ids().is_empty());
    }
}

#[test]
fn a_database_with_no_formula_marks_nothing_and_raises_nothing() {
    let (db, events) = recording_context();
    let f = Fixture::on(db);
    let pages = f.property("Pages", T::Integer);
    let row = f.row();
    f.set(row, &pages, Some(V::Integer(3))).unwrap();
    let raised = |events: Vec<DatabaseEventKind>| {
        events
            .iter()
            .filter(|e| matches!(e, DatabaseEventKind::DatabaseFormulasStale { .. }))
            .count()
    };
    assert_eq!(raised(events.take()), 0);
    assert!(f.stale_ids().is_empty());

    f.formula("Double", T::Integer, "(row) => row.Pages * 2");
    assert_eq!(raised(events.take()), 1);
    assert_eq!(f.stale_ids(), vec![row]);
}

#[test]
fn a_change_during_a_pass_keeps_its_row_outdated() {
    let (f, pages, double, rows) = pages();
    f.set(rows[0], &pages, Some(V::Integer(1))).unwrap();
    let read = f.stale();
    // The value changes again after the pass read the row.
    f.set(rows[0], &pages, Some(V::Integer(5))).unwrap();
    f.store(read, doubled);
    assert_eq!(f.value(rows[0], &double), Some(V::Integer(2)));
    assert_eq!(f.stale_ids(), vec![rows[0]]);
    f.pass(doubled);
    assert_eq!(f.value(rows[0], &double), Some(V::Integer(10)));
}

#[test]
fn outputs_of_a_source_changed_since_are_dropped() {
    let (f, _, double, rows) = pages();
    f.set_formula(&double, Some("(row) => row.Pages * 3"));
    let read = f.stale();
    f.set_formula(&double, Some("(row) => row.Pages * 4"));
    f.store(read, |_, _| Ok(Some(V::Integer(-1))));
    assert_eq!(f.value(rows[0], &double), Some(V::Integer(60)));
    assert_eq!(f.stale_ids(), rows);
}

#[test]
fn outputs_of_a_row_deleted_since_are_dropped() {
    let (f, pages, _, rows) = pages();
    f.set(rows[0], &pages, Some(V::Integer(1))).unwrap();
    let read = f.stale();
    f.db.send_blocking(DeleteDatabaseRowCommand {
        workspace_id: f.ws,
        database_id: f.database.id,
        row_id: rows[0],
    })
    .unwrap();
    f.store(read, doubled);
    assert!(f.stale_ids().is_empty());
}

#[test]
fn a_failing_row_leaves_its_cell_empty_and_the_error_on_the_formula() {
    let (f, pages, double, rows) = pages();
    f.set_formula(&double, Some("(row) => row.Pages * 3"));
    f.pass(|formula, inputs| match number(inputs, "Pages") {
        Some(10) => Err("TypeError: no".to_string()),
        _ => doubled(formula, inputs),
    });
    assert_eq!(f.value(rows[1], &double), None);
    assert_eq!(f.value(rows[0], &double), Some(V::Integer(60)));
    assert_eq!(f.value(rows[2], &double), Some(V::Integer(40)));
    assert_eq!(f.error(&double), Some("TypeError: no".to_string()));

    // A pass over another row alone keeps the error, and outdates every
    // row so the next pass finds whether one still fails.
    f.set(rows[0], &pages, Some(V::Integer(31))).unwrap();
    f.pass(doubled);
    assert_eq!(f.error(&double), Some("TypeError: no".to_string()));
    assert_eq!(f.stale_ids(), rows);

    f.pass(doubled);
    assert_eq!(f.error(&double), None);
    assert_eq!(f.value(rows[1], &double), Some(V::Integer(20)));
    assert!(f.stale_ids().is_empty());
}

#[test]
fn a_new_source_clears_the_old_ones_error() {
    let (f, _, double, _) = pages();
    f.set_formula(&double, Some("(row) => {"));
    f.pass(|_, _| Err("SyntaxError".to_string()));
    assert_eq!(f.error(&double), Some("SyntaxError".to_string()));
    let changed = f.set_formula(&double, Some("(row) => row.Pages * 2"));
    assert_eq!(changed.formula.unwrap().error, None);
}

#[test]
fn becoming_a_formula_drops_hand_set_values_and_leaving_keeps_outputs() {
    let (f, pages, double, rows) = pages();
    let plain = f.set_formula(&double, None);
    assert!(plain.formula.is_none());
    assert_eq!(f.value(rows[0], &double), Some(V::Integer(60)));
    f.set(rows[0], &double, Some(V::Integer(7))).unwrap();

    f.set_formula(&pages, Some("(row) => 1"));
    assert_eq!(f.value(rows[0], &pages), None);
    assert_eq!(f.stale_ids(), rows);
}

#[test]
fn an_empty_formula_is_refused() {
    let f = Fixture::new();
    let refused = f.create_property("Double", T::Integer, Some("  ".to_string()));
    assert!(matches!(refused, Err(CommandError::Validation(_))));
}

#[test]
fn formulas_read_only_properties_that_arent_formulas_by_name() {
    let f = Fixture::new();
    let title = f.property("Title", T::Text);
    f.formula("Shout", T::Text, "(row) => row.Title");
    // Of two properties with one name, the first created is read.
    f.property("Title", T::Integer);
    let row = f.row();
    f.set(row, &title, Some(V::Text("Dune".to_string())))
        .unwrap();
    let stale = f.stale();
    assert_eq!(
        stale.rows[0].inputs,
        vec![("Title".to_string(), Some(V::Text("Dune".to_string())))]
    );
}

#[test]
fn the_databases_left_outdated_are_listed() {
    let (f, pages, _, rows) = pages();
    assert!(
        f.db.send_blocking(ListDatabasesWithStaleFormulasCommand)
            .unwrap()
            .is_empty()
    );
    f.set(rows[0], &pages, Some(V::Integer(1))).unwrap();
    assert_eq!(
        f.db.send_blocking(ListDatabasesWithStaleFormulasCommand)
            .unwrap(),
        vec![f.database.id]
    );
}
