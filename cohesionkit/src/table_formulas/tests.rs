use cohesion_core::model::{
    DocumentFolderProperty, FormulaUpdateMode, PropertyDataType, PropertyType, PropertyValue,
};
use cohesion_core::protocol::{FormulaDefinition, TableColumn, TableCondition, TableFilter};
use cohesion_db::{
    Container, CreateDocumentCommand, CreateDocumentFolderCommand,
    CreateDocumentFolderPropertyCommand, CreateWorkspaceCommand, DatabaseContext,
    DatabaseEventKind, DatabaseEventListener, DatabaseEventTrace, GetTableRowCommand,
    ListDocumentFolderPropertiesCommand, ListTableRowsCommand, ListTablesWithStaleFormulasCommand,
    Secrets, SetDocumentFolderPropertyFormulaCommand, SetDocumentValueCommand, WorkTracker,
};
use cohesion_js::ScriptEngine;

use super::{FormulaQueue, evaluate_formulas, mark_continuous_formulas};
use crate::hooks::HookManager;

use PropertyDataType as T;
use PropertyValue as V;

/// A table of orders with a price and a quantity, on an engine of its
/// own; nothing runs a pass but the test.
struct Orders {
    db: DatabaseContext,
    engine: ScriptEngine,
    runtime: tokio::runtime::Runtime,
    ws: i64,
    folder_id: i64,
    price: DocumentFolderProperty,
    quantity: DocumentFolderProperty,
}

impl Orders {
    fn new() -> Self {
        let mut container = Container::in_memory().unwrap();
        container.migrate().unwrap();
        let db = DatabaseContext::new(container, Secrets::new(), ());
        let ws = db
            .send_blocking(CreateWorkspaceCommand {
                title: "Shop".to_string(),
                emoji: None,
            })
            .unwrap()
            .id;
        let folder_id = db
            .send_blocking(CreateDocumentFolderCommand {
                workspace_id: ws,
                title: "Orders".to_string(),
                parent_id: None,
            })
            .unwrap()
            .id;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let engine = runtime.block_on(async { ScriptEngine::new(db.read_write()) });
        let mut orders = Self {
            engine,
            runtime,
            ws,
            folder_id,
            price: dummy(),
            quantity: dummy(),
            db,
        };
        orders.price = orders.property("Price", T::Currency, None);
        orders.quantity = orders.property("Quantity", T::Integer, None);
        orders
    }

    fn property(&self, name: &str, data_type: T, formula: Option<&str>) -> DocumentFolderProperty {
        self.property_as(name, data_type, formula, FormulaUpdateMode::WhenChanged)
    }

    fn property_as(
        &self,
        name: &str,
        data_type: T,
        formula: Option<&str>,
        update_mode: FormulaUpdateMode,
    ) -> DocumentFolderProperty {
        self.db
            .send_blocking(CreateDocumentFolderPropertyCommand {
                workspace_id: self.ws,
                folder_id: self.folder_id,
                name: name.to_string(),
                data_type,
                formula: formula.map(|source| FormulaDefinition {
                    source: source.to_string(),
                    update_mode,
                }),
                options: Vec::new(),
            })
            .unwrap()
    }

    /// An order of `quantity` at `price`.
    fn order(&self, price: f64, quantity: Option<i64>) -> i64 {
        let row = self
            .db
            .send_blocking(CreateDocumentCommand {
                workspace_id: self.ws,
                title: "Order".to_string(),
                contents: String::new(),
                folder_id: Some(self.folder_id),
            })
            .unwrap()
            .id;
        self.set(row, &self.price, Some(V::Currency(price)));
        self.set(row, &self.quantity, quantity.map(V::Integer));
        row
    }

    fn set(&self, row_id: i64, property: &DocumentFolderProperty, value: Option<V>) {
        self.db
            .send_blocking(SetDocumentValueCommand {
                workspace_id: self.ws,
                folder_id: self.folder_id,
                document_id: row_id,
                property_id: property.id,
                value,
            })
            .unwrap();
    }

    fn pass(&self) {
        self.runtime
            .block_on(evaluate_formulas(&self.db, &self.engine, self.folder_id))
            .unwrap();
    }

    fn value(&self, row_id: i64, property: &DocumentFolderProperty) -> Option<V> {
        self.db
            .send_blocking(GetTableRowCommand {
                workspace_id: self.ws,
                folder_id: self.folder_id,
                document_id: row_id,
            })
            .unwrap()
            .cells
            .into_iter()
            .find(|cell| cell.property_id == property.id)
            .map(|cell| cell.value)
    }

    fn error(&self, property: &DocumentFolderProperty) -> Option<String> {
        self.db
            .send_blocking(ListDocumentFolderPropertiesCommand {
                workspace_id: self.ws,
                folder_id: self.folder_id,
            })
            .unwrap()
            .into_iter()
            .find(|p| p.id == property.id)
            .and_then(|p| p.formula)
            .and_then(|formula| formula.error)
    }
}

fn dummy() -> DocumentFolderProperty {
    DocumentFolderProperty {
        id: 0,
        folder_id: 0,
        name: String::new(),
        kind: PropertyType::Text,
        sort_order: 0,
        formula: None,
    }
}

#[test]
fn a_pass_computes_every_formula_and_stores_what_filters_read() {
    let o = Orders::new();
    let total = o.property(
        "Total",
        T::Currency,
        Some("(row) => row.Quantity === null ? null : row.Price * row.Quantity"),
    );
    let cheap = o.order(2.5, Some(2));
    let dear = o.order(10.0, Some(3));
    let unknown = o.order(1.0, None);
    o.pass();
    assert_eq!(o.value(cheap, &total), Some(V::Currency(5.0)));
    assert_eq!(o.value(dear, &total), Some(V::Currency(30.0)));
    assert_eq!(o.value(unknown, &total), None);
    assert_eq!(o.error(&total), None);
    let over_ten =
        o.db.send_blocking(ListTableRowsCommand {
            workspace_id: o.ws,
            folder_id: o.folder_id,
            filters: vec![TableFilter {
                column: TableColumn::Property {
                    property_id: total.id,
                },
                condition: TableCondition::GreaterThan {
                    value: V::Currency(10.0),
                },
            }],
            sort: None,
        })
        .unwrap();
    assert_eq!(
        over_ten
            .iter()
            .map(|row| row.document_id)
            .collect::<Vec<_>>(),
        vec![dear]
    );
    assert!(
        o.db.send_blocking(ListTablesWithStaleFormulasCommand)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_value_change_and_a_source_change_are_computed_again() {
    let o = Orders::new();
    let total = o.property(
        "Total",
        T::Currency,
        Some("(row) => row.Price * row.Quantity"),
    );
    let row = o.order(2.0, Some(2));
    o.pass();
    assert_eq!(o.value(row, &total), Some(V::Currency(4.0)));

    o.set(row, &o.quantity, Some(V::Integer(5)));
    o.pass();
    assert_eq!(o.value(row, &total), Some(V::Currency(10.0)));

    // The compiled function of the old source isn't served again.
    o.db.send_blocking(SetDocumentFolderPropertyFormulaCommand {
        workspace_id: o.ws,
        folder_id: o.folder_id,
        property_id: total.id,
        formula: Some(FormulaDefinition {
            source: "(row) => row.Price + row.Quantity".to_string(),
            update_mode: FormulaUpdateMode::WhenChanged,
        }),
    })
    .unwrap();
    o.pass();
    assert_eq!(o.value(row, &total), Some(V::Currency(7.0)));
}

#[test]
fn a_throwing_or_mistyped_formula_leaves_empty_cells_and_an_error_alone() {
    let o = Orders::new();
    let throws = o.property(
        "Throws",
        T::Integer,
        Some("(row) => { if (row.Quantity === 1) throw new Error('one'); return row.Quantity }"),
    );
    let mistyped = o.property("Mistyped", T::Integer, Some("(row) => 'many'"));
    let fine = o.property("Fine", T::Integer, Some("(row) => row.Quantity * 10"));
    let one = o.order(1.0, Some(1));
    let two = o.order(1.0, Some(2));
    o.pass();
    assert_eq!(o.value(one, &throws), None);
    assert_eq!(o.value(two, &throws), Some(V::Integer(2)));
    assert!(o.error(&throws).unwrap().contains("one"));
    assert_eq!(o.value(one, &mistyped), None);
    assert!(o.error(&mistyped).is_some());
    assert_eq!(o.value(one, &fine), Some(V::Integer(10)));
    assert_eq!(o.error(&fine), None);
}

#[test]
fn the_hourly_run_computes_a_continuous_formula_that_reads_the_clock_again() {
    let o = Orders::new();
    let now = o.property_as(
        "Now",
        T::Integer,
        Some("(row) => Date.now()"),
        FormulaUpdateMode::Continuously,
    );
    let once = o.property("Once", T::Integer, Some("(row) => Date.now()"));
    let row = o.order(1.0, Some(1));
    o.pass();
    let (first, first_once) = (o.value(row, &now), o.value(row, &once));
    assert!(first.is_some());
    std::thread::sleep(std::time::Duration::from_millis(5));

    o.runtime.block_on(mark_continuous_formulas(&o.db));
    assert_eq!(
        o.db.send_blocking(ListTablesWithStaleFormulasCommand)
            .unwrap(),
        vec![o.folder_id]
    );
    o.pass();
    // Every formula of the row is computed in the pass.
    assert_ne!(o.value(row, &now), first);
    assert_ne!(o.value(row, &once), first_once);
}

#[test]
fn a_formula_cannot_reach_the_database_or_the_network() {
    let o = Orders::new();
    let reach = o.property(
        "Reach",
        T::Text,
        Some("(row) => [typeof cohesion, typeof network, typeof fetch, typeof storage].join()"),
    );
    let row = o.order(1.0, Some(1));
    o.pass();
    assert_eq!(
        o.value(row, &reach),
        Some(V::Text("undefined,undefined,undefined,undefined".into()))
    );
}

#[test]
fn the_hook_manager_hands_stale_formulas_to_the_queue_not_the_hooks() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let _entered = runtime.enter();
    let work = WorkTracker::default();
    let (queue, mut runner) = FormulaQueue::new(work.clone());
    let (hooks, _receiver) = HookManager::new(work.clone(), queue);
    DatabaseEventListener::send(
        &hooks,
        DatabaseEventTrace::default().event(DatabaseEventKind::TableFormulasStale { folder_id: 7 }),
    );
    // The only work queued is the pass, no hook.
    let pending: Vec<String> = work.pending().into_iter().map(|w| w.label).collect();
    assert_eq!(pending, vec!["formulas of document folder 7".to_string()]);
    assert_eq!(runner.0.try_recv().unwrap().0, 7);
}
