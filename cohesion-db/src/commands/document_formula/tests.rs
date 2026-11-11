use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{
    DocumentFolderProperty, FormulaUpdateMode, PropertyDataType, PropertyValue,
};
use cohesion_core::protocol::{TableColumn, TableCondition, TableFilter, TableSort};

use crate::commands::document_folder_property::tests::{Fixture, definition};
use crate::test_support::recording_context;
use crate::{
    CommandError, DatabaseEventKind, DeleteDocumentCommand, DeleteDocumentFolderPropertyCommand,
    FormulaOutputs, FormulaProperty, ListDocumentFolderPropertiesCommand,
    ListStaleTableRowsCommand, ListTableRowsCommand, ListTablesWithStaleFormulasCommand,
    MarkContinuousFormulasStaleCommand, MoveDocumentCommand, RenameDocumentFolderPropertyCommand,
    SetDocumentFolderPropertyFormulaCommand, StaleTableRow, StaleTableRows,
    StoreTableFormulaOutputsCommand,
};

use FormulaUpdateMode::{Continuously, WhenChanged};

use PropertyDataType as T;
use PropertyValue as V;

/// What a formula reads of a row: its non-formula values by name.
type Inputs = [(String, Option<V>)];

impl Fixture {
    fn stale(&self) -> StaleTableRows {
        self.db
            .send_blocking(ListStaleTableRowsCommand {
                folder_id: self.folder.id,
            })
            .unwrap()
    }

    /// The ids of the documents whose outputs are out of date.
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
        stale: StaleTableRows,
        compute: impl Fn(&FormulaProperty, &Inputs) -> Result<Option<V>, String>,
    ) {
        let formulas = stale
            .formulas
            .into_iter()
            .map(|formula| FormulaOutputs {
                outputs: stale
                    .rows
                    .iter()
                    .map(|row: &StaleTableRow| compute(&formula, &row.inputs))
                    .collect(),
                formula,
            })
            .collect();
        self.db
            .send_blocking(StoreTableFormulaOutputsCommand {
                folder_id: self.folder.id,
                rows: stale.rows.iter().map(|row| row.staleness).collect(),
                formulas,
            })
            .unwrap();
    }

    /// Reads the outdated rows and stores what `compute` gives for them.
    fn pass(&self, compute: impl Fn(&FormulaProperty, &Inputs) -> Result<Option<V>, String>) {
        self.store(self.stale(), compute);
    }

    fn set_formula(
        &self,
        property: &DocumentFolderProperty,
        formula: Option<&str>,
    ) -> DocumentFolderProperty {
        self.set_formula_as(property, formula, WhenChanged)
    }

    fn set_formula_as(
        &self,
        property: &DocumentFolderProperty,
        formula: Option<&str>,
        update_mode: FormulaUpdateMode,
    ) -> DocumentFolderProperty {
        self.db
            .send_blocking(SetDocumentFolderPropertyFormulaCommand {
                workspace_id: self.ws,
                folder_id: self.folder.id,
                property_id: property.id,
                formula: formula.map(|source| definition(source, update_mode)),
            })
            .unwrap()
    }

    /// The table's invalidations raised by `change`.
    fn invalidations(&self, change: impl FnOnce()) -> Vec<Invalidation> {
        let mut batches = self.db.subscribe();
        change();
        let mut keys = Vec::new();
        while let Ok(batch) = batches.try_recv() {
            keys.extend(batch.keys.iter().cloned());
        }
        keys.retain(|key| *key == Invalidation::DocumentTable(self.folder.id));
        keys
    }

    /// The property as it is now.
    fn reread(&self, property: &DocumentFolderProperty) -> DocumentFolderProperty {
        self.db
            .send_blocking(ListDocumentFolderPropertiesCommand {
                workspace_id: self.ws,
                folder_id: self.folder.id,
            })
            .unwrap()
            .into_iter()
            .find(|p| p.id == property.id)
            .unwrap()
    }

    fn error(&self, property: &DocumentFolderProperty) -> Option<String> {
        self.reread(property).formula.unwrap().error
    }

    /// The value document `row_id` holds for `property`.
    fn value(&self, row_id: i64, property: &DocumentFolderProperty) -> Option<V> {
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

/// A table of three documents with `Pages` 30, 10 and 20, and a formula
/// `Double` of it, computed.
fn pages() -> (
    Fixture,
    DocumentFolderProperty,
    DocumentFolderProperty,
    Vec<i64>,
) {
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
        f.db.send_blocking(ListTableRowsCommand {
            workspace_id: f.ws,
            folder_id: f.folder.id,
            filters: vec![TableFilter {
                column: TableColumn::Property {
                    property_id: double.id,
                },
                condition: TableCondition::GreaterThan {
                    value: V::Integer(30),
                },
            }],
            sort: Some(TableSort {
                column: TableColumn::Property {
                    property_id: double.id,
                },
                descending: false,
            }),
        })
        .unwrap();
    let ids: Vec<i64> = listed.iter().map(|row| row.document_id).collect();
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
fn a_document_moved_in_is_computed_and_one_moved_out_is_not() {
    let (f, _, double, rows) = pages();
    let note = f.document_in(None, "Note");
    let move_to = |document_id, folder_id| {
        f.db.send_blocking(MoveDocumentCommand {
            workspace_id: f.ws,
            document_id,
            folder_id,
        })
        .unwrap();
    };
    move_to(note, Some(f.folder.id));
    assert_eq!(f.stale_ids(), vec![note]);
    // Leaving before its pass, it is no longer marked, and its output isn't
    // stored.
    let read = f.stale();
    move_to(note, None);
    f.store(read, |_, _| Ok(Some(V::Integer(1))));
    assert!(f.stale_ids().is_empty());
    assert!(
        f.stored(&double)
            .iter()
            .all(|(document, ..)| rows.contains(document))
    );
    assert!(
        f.db.send_blocking(ListTablesWithStaleFormulasCommand)
            .unwrap()
            .is_empty()
    );
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
            f.db.send_blocking(RenameDocumentFolderPropertyCommand {
                workspace_id: f.ws,
                folder_id: f.folder.id,
                property_id: pages.id,
                name: "Leaves".to_string(),
            })
            .unwrap();
        },
        &|| {
            f.db.send_blocking(DeleteDocumentFolderPropertyCommand {
                workspace_id: f.ws,
                folder_id: f.folder.id,
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
fn a_table_with_no_formula_marks_nothing_and_raises_nothing() {
    let (db, events) = recording_context();
    let f = Fixture::on(db);
    let pages = f.property("Pages", T::Integer);
    let row = f.row();
    f.set(row, &pages, Some(V::Integer(3))).unwrap();
    let raised = |events: Vec<DatabaseEventKind>| {
        events
            .iter()
            .filter(|e| matches!(e, DatabaseEventKind::TableFormulasStale { .. }))
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
fn outputs_of_a_document_moved_away_since_are_dropped() {
    let (f, pages, double, rows) = pages();
    f.set(rows[0], &pages, Some(V::Integer(1))).unwrap();
    let read = f.stale();
    f.db.send_blocking(MoveDocumentCommand {
        workspace_id: f.ws,
        document_id: rows[0],
        folder_id: None,
    })
    .unwrap();
    f.store(read, doubled);
    assert!(f.stale_ids().is_empty());
    assert!(
        f.stored(&double)
            .iter()
            .all(|(document, ..)| *document != rows[0])
    );
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
    let refused = f.create_property("Double", T::Integer, Some(definition("  ", WhenChanged)));
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
fn the_tables_left_outdated_are_listed() {
    let (f, pages, _, rows) = pages();
    assert!(
        f.db.send_blocking(ListTablesWithStaleFormulasCommand)
            .unwrap()
            .is_empty()
    );
    f.set(rows[0], &pages, Some(V::Integer(1))).unwrap();
    assert_eq!(
        f.db.send_blocking(ListTablesWithStaleFormulasCommand)
            .unwrap(),
        vec![f.folder.id]
    );
}

#[test]
fn a_formulas_update_mode_round_trips_and_a_hand_set_property_has_none() {
    let f = Fixture::new();
    let plain = f.property("Due", T::Date);
    assert!(plain.formula.is_none());
    let left = f
        .create_property(
            "Days left",
            T::Integer,
            Some(definition("(row) => 1", Continuously)),
        )
        .unwrap();
    assert_eq!(left.formula.as_ref().unwrap().update_mode, Continuously);
    assert_eq!(f.reread(&left).formula.unwrap().update_mode, Continuously);
    let doubled = f.formula("Double", T::Integer, "(row) => 2");
    assert_eq!(f.reread(&doubled).formula.unwrap().update_mode, WhenChanged);
    // Set by hand again, it has no mode left.
    assert!(f.set_formula(&left, None).formula.is_none());
    assert!(f.reread(&left).formula.is_none());
}

#[test]
fn changing_only_the_update_mode_keeps_outputs_and_error_and_computes_nothing() {
    let (f, _, double, rows) = pages();
    f.set_formula(&double, Some("(row) => row.Pages * 3"));
    f.pass(|formula, inputs| match number(inputs, "Pages") {
        Some(10) => Err("TypeError: no".to_string()),
        _ => doubled(formula, inputs),
    });
    assert!(f.stale_ids().is_empty());
    let mut changed = None;
    let invalidated = f.invalidations(|| {
        changed = Some(f.set_formula_as(&double, Some("(row) => row.Pages * 3"), Continuously));
    });
    let formula = changed.unwrap().formula.unwrap();
    assert_eq!(formula.update_mode, Continuously);
    assert_eq!(formula.error, Some("TypeError: no".to_string()));
    assert_eq!(f.value(rows[0], &double), Some(V::Integer(60)));
    assert!(f.stale_ids().is_empty());
    // The property changed, so its table is refetched.
    assert_eq!(invalidated, [Invalidation::DocumentTable(f.folder.id)]);

    // A new source with the mode computes again, and keeps the mode.
    let changed = f.set_formula_as(&double, Some("(row) => 1"), Continuously);
    assert_eq!(changed.formula.unwrap().update_mode, Continuously);
    assert_eq!(f.stale_ids(), rows);
}

#[test]
fn the_hourly_run_marks_only_live_documents_of_folders_with_a_continuous_formula() {
    let (db, events) = recording_context();
    let f = Fixture::on(db);
    let pages = f.property("Pages", T::Integer);
    let left = f
        .create_property(
            "Days left",
            T::Integer,
            Some(definition("(row) => 1", Continuously)),
        )
        .unwrap();
    let rows: Vec<i64> = (0..3).map(|_| f.row()).collect();
    f.set(rows[0], &pages, Some(V::Integer(1))).unwrap();
    f.db.send_blocking(DeleteDocumentCommand {
        workspace_id: f.ws,
        document_id: rows[2],
    })
    .unwrap();
    f.pass(|_, _| Ok(Some(V::Integer(1))));
    // Another table whose formula is computed only on change, and a plain
    // folder.
    let other = f.new_folder("Other");
    f.db.send_blocking(crate::CreateDocumentFolderPropertyCommand {
        workspace_id: f.ws,
        folder_id: other.id,
        name: "Double".to_string(),
        data_type: T::Integer,
        formula: Some(definition("(row) => 2", WhenChanged)),
        options: Vec::new(),
    })
    .unwrap();
    f.document_in(Some(other.id), "Elsewhere");
    f.new_folder("Plain");
    let stale_other = || {
        f.db.send_blocking(ListStaleTableRowsCommand {
            folder_id: other.id,
        })
        .unwrap()
        .rows
        .len()
    };
    let before_other = stale_other();
    events.take();

    let marked =
        f.db.send_blocking(MarkContinuousFormulasStaleCommand)
            .unwrap();
    assert_eq!(marked, vec![f.folder.id]);
    assert_eq!(f.stale_ids(), vec![rows[0], rows[1]]);
    assert_eq!(stale_other(), before_other);
    assert_eq!(
        events.take(),
        vec![DatabaseEventKind::TableFormulasStale {
            folder_id: f.folder.id
        }]
    );

    // Once the formula is computed only on change, the run passes it by.
    f.pass(|_, _| Ok(Some(V::Integer(1))));
    f.set_formula_as(&left, Some("(row) => 1"), WhenChanged);
    assert!(
        f.db.send_blocking(MarkContinuousFormulasStaleCommand)
            .unwrap()
            .is_empty()
    );
    assert!(f.stale_ids().is_empty());
}

#[test]
fn a_pass_that_changes_nothing_invalidates_nothing() {
    let (f, pages, double, rows) = pages();
    f.set_formula_as(&double, Some("(row) => row.Pages * 2"), Continuously);
    f.db.send_blocking(MarkContinuousFormulasStaleCommand)
        .unwrap();
    assert_eq!(f.stale_ids(), rows);
    // The same outputs: nothing is refetched, and the marks clear.
    assert!(f.invalidations(|| f.pass(doubled)).is_empty());
    assert!(f.stale_ids().is_empty());

    // A changed output invalidates the table.
    f.set(rows[0], &pages, Some(V::Integer(1))).unwrap();
    assert_eq!(
        f.invalidations(|| f.pass(doubled)),
        [Invalidation::DocumentTable(f.folder.id)]
    );
    assert_eq!(f.value(rows[0], &double), Some(V::Integer(2)));
    // So does an error appearing, with the cells it empties.
    f.db.send_blocking(MarkContinuousFormulasStaleCommand)
        .unwrap();
    assert_eq!(
        f.invalidations(|| f.pass(|_, _| Err("RangeError".to_string()))),
        [Invalidation::DocumentTable(f.folder.id)]
    );
    assert_eq!(f.error(&double), Some("RangeError".to_string()));
}
