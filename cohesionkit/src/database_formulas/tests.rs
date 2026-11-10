use cohesion_core::model::{DatabaseDataType, DatabaseProperty, DatabaseValue};
use cohesion_core::protocol::{DatabaseCondition, DatabaseFilter};
use cohesion_db::{
    Container, CreateDatabaseCommand, CreateDatabasePropertyCommand, CreateDatabaseRowCommand,
    CreateWorkspaceCommand, DatabaseContext, DatabaseEventKind, DatabaseEventListener,
    DatabaseEventTrace, GetDatabaseRowCommand, ListDatabasePropertiesCommand,
    ListDatabaseRowsCommand, ListDatabasesWithStaleFormulasCommand, Secrets,
    SetDatabasePropertyFormulaCommand, SetDatabaseValueCommand, WorkTracker,
};
use cohesion_js::ScriptEngine;

use super::{FormulaQueue, evaluate_formulas};
use crate::hooks::HookManager;

use DatabaseDataType as T;
use DatabaseValue as V;

/// A database of orders with a price and a quantity, on an engine of its
/// own; nothing runs a pass but the test.
struct Orders {
    db: DatabaseContext,
    engine: ScriptEngine,
    runtime: tokio::runtime::Runtime,
    ws: i64,
    database_id: i64,
    price: DatabaseProperty,
    quantity: DatabaseProperty,
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
        let database_id = db
            .send_blocking(CreateDatabaseCommand {
                workspace_id: ws,
                title: "Orders".to_string(),
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
            database_id,
            price: dummy(),
            quantity: dummy(),
            db,
        };
        orders.price = orders.property("Price", T::Currency, None);
        orders.quantity = orders.property("Quantity", T::Integer, None);
        orders
    }

    fn property(&self, name: &str, data_type: T, formula: Option<&str>) -> DatabaseProperty {
        self.db
            .send_blocking(CreateDatabasePropertyCommand {
                workspace_id: self.ws,
                database_id: self.database_id,
                name: name.to_string(),
                data_type,
                formula: formula.map(str::to_string),
            })
            .unwrap()
    }

    /// An order of `quantity` at `price`.
    fn order(&self, price: f64, quantity: Option<i64>) -> i64 {
        let row = self
            .db
            .send_blocking(CreateDatabaseRowCommand {
                workspace_id: self.ws,
                database_id: self.database_id,
            })
            .unwrap()
            .id;
        self.set(row, &self.price, Some(V::Currency(price)));
        self.set(row, &self.quantity, quantity.map(V::Integer));
        row
    }

    fn set(&self, row_id: i64, property: &DatabaseProperty, value: Option<V>) {
        self.db
            .send_blocking(SetDatabaseValueCommand {
                workspace_id: self.ws,
                database_id: self.database_id,
                row_id,
                property_id: property.id,
                value,
            })
            .unwrap();
    }

    fn pass(&self) {
        self.runtime
            .block_on(evaluate_formulas(&self.db, &self.engine, self.database_id))
            .unwrap();
    }

    fn value(&self, row_id: i64, property: &DatabaseProperty) -> Option<V> {
        self.db
            .send_blocking(GetDatabaseRowCommand {
                workspace_id: self.ws,
                database_id: self.database_id,
                row_id,
            })
            .unwrap()
            .cells
            .into_iter()
            .find(|cell| cell.property_id == property.id)
            .map(|cell| cell.value)
    }

    fn error(&self, property: &DatabaseProperty) -> Option<String> {
        self.db
            .send_blocking(ListDatabasePropertiesCommand {
                workspace_id: self.ws,
                database_id: self.database_id,
            })
            .unwrap()
            .into_iter()
            .find(|p| p.id == property.id)
            .and_then(|p| p.formula)
            .and_then(|formula| formula.error)
    }
}

fn dummy() -> DatabaseProperty {
    DatabaseProperty {
        id: 0,
        database_id: 0,
        name: String::new(),
        data_type: T::Text,
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
        o.db.send_blocking(ListDatabaseRowsCommand {
            workspace_id: o.ws,
            database_id: o.database_id,
            filters: vec![DatabaseFilter {
                property_id: total.id,
                condition: DatabaseCondition::GreaterThan {
                    value: V::Currency(10.0),
                },
            }],
            sort: None,
        })
        .unwrap();
    assert_eq!(
        over_ten.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![dear]
    );
    assert!(
        o.db.send_blocking(ListDatabasesWithStaleFormulasCommand)
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
    o.db.send_blocking(SetDatabasePropertyFormulaCommand {
        workspace_id: o.ws,
        database_id: o.database_id,
        property_id: total.id,
        formula: Some("(row) => row.Price + row.Quantity".to_string()),
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
        DatabaseEventTrace::default()
            .event(DatabaseEventKind::DatabaseFormulasStale { database_id: 7 }),
    );
    // The only work queued is the pass, no hook.
    let pending: Vec<String> = work.pending().into_iter().map(|w| w.label).collect();
    assert_eq!(pending, vec!["formulas of database 7".to_string()]);
    assert_eq!(runner.0.try_recv().unwrap().0, 7);
}
