use cohesion_core::model::{
    Database, DatabaseCell, DatabaseDataType, DatabaseProperty, DatabaseRowValue, DatabaseValue,
};

use crate::test_support::context;
use crate::{
    CommandContext, CommandError, CreateDatabaseCommand, CreateDatabasePropertyCommand,
    CreateDatabaseRowCommand, CreateDatabaseRowDocumentCommand, CreateWorkspaceCommand,
    DatabaseCommand, DatabaseCommandResult, DatabaseContext, DeleteDatabaseCommand,
    DeleteDatabasePropertyCommand, Filtered, GetDatabaseRowCommand, GetDocumentCommand,
    ListDatabasePropertiesCommand, ListDatabasesCommand, MoveDatabasePropertyCommand, Read,
    ReadableDatabase, SetDatabasePropertyTypeCommand, SetDatabaseValueCommand,
};

/// Every stored value of one property, as `(row, text, integer, real)`, by row.
struct StoredValues(i64);
impl DatabaseCommand for StoredValues {
    type Response = Vec<(i64, String, Option<i64>, Option<f64>)>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        let values: Vec<DatabaseRowValue> = ctx
            .db
            .select()
            .filter_eq(DatabaseRowValue::PROPERTY_ID, self.0)
            .order("row_id")
            .all()?;
        Ok(values
            .into_iter()
            .map(|v| (v.row_id, v.text_value, v.integer_value, v.real_value))
            .collect())
    }
}

pub(crate) struct Fixture {
    pub db: DatabaseContext,
    pub ws: i64,
    pub database: Database,
}

impl Fixture {
    pub fn new() -> Self {
        let db = context();
        let ws = db
            .send_blocking(CreateWorkspaceCommand {
                title: "Home".to_string(),
                emoji: None,
            })
            .unwrap()
            .id;
        let database = db
            .send_blocking(CreateDatabaseCommand {
                workspace_id: ws,
                title: "Books".to_string(),
            })
            .unwrap();
        Self { db, ws, database }
    }

    pub fn property(&self, name: &str, data_type: DatabaseDataType) -> DatabaseProperty {
        self.db
            .send_blocking(CreateDatabasePropertyCommand {
                workspace_id: self.ws,
                database_id: self.database.id,
                name: name.to_string(),
                data_type,
            })
            .unwrap()
    }

    pub fn row(&self) -> i64 {
        self.db
            .send_blocking(CreateDatabaseRowCommand {
                workspace_id: self.ws,
                database_id: self.database.id,
            })
            .unwrap()
            .id
    }

    pub fn set(
        &self,
        row_id: i64,
        property: &DatabaseProperty,
        value: Option<DatabaseValue>,
    ) -> Result<Vec<DatabaseCell>, CommandError> {
        self.db
            .send_blocking(SetDatabaseValueCommand {
                workspace_id: self.ws,
                database_id: self.database.id,
                row_id,
                property_id: property.id,
                value,
            })
            .map(|entry| entry.cells)
    }

    pub fn cells(&self, row_id: i64) -> Vec<DatabaseCell> {
        self.db
            .send_blocking(GetDatabaseRowCommand {
                workspace_id: self.ws,
                database_id: self.database.id,
                row_id,
            })
            .unwrap()
            .cells
    }

    fn retype(&self, property: &DatabaseProperty, data_type: DatabaseDataType) {
        self.db
            .send_blocking(SetDatabasePropertyTypeCommand {
                workspace_id: self.ws,
                database_id: self.database.id,
                property_id: property.id,
                data_type,
            })
            .unwrap();
    }

    fn stored(&self, property: &DatabaseProperty) -> Vec<(i64, String, Option<i64>, Option<f64>)> {
        self.db.send_blocking(StoredValues(property.id)).unwrap()
    }
}

fn cell(property: &DatabaseProperty, value: DatabaseValue) -> DatabaseCell {
    DatabaseCell {
        property_id: property.id,
        value,
    }
}

#[test]
fn a_value_is_stored_in_every_representation_its_type_gives() {
    use DatabaseDataType::*;
    let f = Fixture::new();
    let cases = [
        (Text, DatabaseValue::Text("Dune".into()), "Dune", None, None),
        (
            Integer,
            DatabaseValue::Integer(412),
            "412",
            Some(412),
            Some(412.0),
        ),
        (Decimal, DatabaseValue::Decimal(4.5), "4.5", None, Some(4.5)),
        (
            Currency,
            DatabaseValue::Currency(9.99),
            "9.99",
            None,
            Some(9.99),
        ),
        (
            Checkbox,
            DatabaseValue::Checkbox(true),
            "true",
            Some(1),
            None,
        ),
        (
            Date,
            DatabaseValue::Date("1965-08-01".into()),
            "1965-08-01",
            None,
            None,
        ),
    ];
    let row = f.row();
    for (data_type, value, text, integer, real) in cases {
        let property = f.property(&format!("{data_type:?}"), data_type);
        let cells = f.set(row, &property, Some(value.clone())).unwrap();
        assert!(cells.contains(&cell(&property, value)));
        assert_eq!(
            f.stored(&property),
            [(row, text.to_string(), integer, real)]
        );
    }
}

#[test]
fn a_value_must_fit_its_property_and_everything_may_be_empty() {
    let f = Fixture::new();
    let pages = f.property("Pages", DatabaseDataType::Integer);
    let title = f.property("Title", DatabaseDataType::Text);
    let read = f.property("Read on", DatabaseDataType::Date);
    let row = f.row();
    // Text can't stand in for a number, nor a number for text.
    for (property, value) in [
        (&pages, DatabaseValue::Text("412".into())),
        (&pages, DatabaseValue::Decimal(412.0)),
        (&title, DatabaseValue::Integer(1)),
        (&read, DatabaseValue::Date("1965-02-30".into())),
    ] {
        assert!(matches!(
            f.set(row, property, Some(value)),
            Err(CommandError::Validation(_))
        ));
    }
    assert!(f.cells(row).is_empty());

    f.set(row, &pages, Some(DatabaseValue::Integer(412)))
        .unwrap();
    f.set(row, &title, Some(DatabaseValue::Text("Dune".into())))
        .unwrap();
    // Empty text is no value, and absent empties a value.
    f.set(row, &title, Some(DatabaseValue::Text(String::new())))
        .unwrap();
    f.set(row, &pages, None).unwrap();
    assert!(f.cells(row).is_empty());
    assert!(f.stored(&pages).is_empty());
}

#[test]
fn a_property_changing_type_keeps_the_values_whose_text_is_one() {
    let f = Fixture::new();
    let property = f.property("Pages", DatabaseDataType::Text);
    let rows: Vec<i64> = (0..4).map(|_| f.row()).collect();
    for (row, text) in rows.iter().zip(["412", "4.5", "many", "0"]) {
        f.set(*row, &property, Some(DatabaseValue::Text(text.into())))
            .unwrap();
    }

    f.retype(&property, DatabaseDataType::Integer);
    assert_eq!(
        f.stored(&property),
        [
            (rows[0], "412".to_string(), Some(412), Some(412.0)),
            (rows[3], "0".to_string(), Some(0), Some(0.0)),
        ]
    );

    f.retype(&property, DatabaseDataType::Decimal);
    assert_eq!(
        f.stored(&property),
        [
            (rows[0], "412".to_string(), None, Some(412.0)),
            (rows[3], "0".to_string(), None, Some(0.0)),
        ]
    );
    assert_eq!(
        f.cells(rows[0]),
        [cell(&property, DatabaseValue::Decimal(412.0))]
    );

    // A checkbox reads only `true` and `false`; text keeps everything.
    f.retype(&property, DatabaseDataType::Checkbox);
    assert!(f.stored(&property).is_empty());
    let done = f.property("Done", DatabaseDataType::Checkbox);
    f.set(rows[1], &done, Some(DatabaseValue::Checkbox(false)))
        .unwrap();
    f.retype(&done, DatabaseDataType::Text);
    assert_eq!(
        f.stored(&done),
        [(rows[1], "false".to_string(), None, None)]
    );
}

#[test]
fn deleting_a_property_takes_its_values_and_properties_keep_their_order() {
    let f = Fixture::new();
    let title = f.property("Title", DatabaseDataType::Text);
    let pages = f.property("Pages", DatabaseDataType::Integer);
    let read = f.property("Read on", DatabaseDataType::Date);
    let names = |f: &Fixture| -> Vec<String> {
        f.db.send_blocking(ListDatabasePropertiesCommand {
            workspace_id: f.ws,
            database_id: f.database.id,
        })
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect()
    };
    assert_eq!(names(&f), ["Title", "Pages", "Read on"]);
    f.db.send_blocking(MoveDatabasePropertyCommand {
        workspace_id: f.ws,
        database_id: f.database.id,
        property_id: read.id,
        before_id: Some(title.id),
    })
    .unwrap();
    assert_eq!(names(&f), ["Read on", "Title", "Pages"]);

    let row = f.row();
    f.set(row, &pages, Some(DatabaseValue::Integer(412)))
        .unwrap();
    f.set(row, &title, Some(DatabaseValue::Text("Dune".into())))
        .unwrap();
    f.db.send_blocking(DeleteDatabasePropertyCommand {
        workspace_id: f.ws,
        database_id: f.database.id,
        property_id: pages.id,
    })
    .unwrap();
    assert!(f.stored(&pages).is_empty());
    assert_eq!(
        f.cells(row),
        [cell(&title, DatabaseValue::Text("Dune".into()))]
    );
    assert_eq!(names(&f), ["Read on", "Title"]);
}

#[test]
fn a_database_is_its_workspaces_and_deleting_it_takes_its_rows_documents() {
    let f = Fixture::new();
    let other =
        f.db.send_blocking(CreateWorkspaceCommand {
            title: "Work".to_string(),
            emoji: None,
        })
        .unwrap()
        .id;
    // Another workspace can't reach it.
    assert!(matches!(
        f.db.send_blocking(ListDatabasePropertiesCommand {
            workspace_id: other,
            database_id: f.database.id,
        }),
        Err(CommandError::NotFound(_))
    ));
    let row = f.row();
    let document =
        f.db.send_blocking(CreateDatabaseRowDocumentCommand {
            workspace_id: f.ws,
            database_id: f.database.id,
            row_id: row,
        })
        .unwrap();
    f.db.send_blocking(DeleteDatabaseCommand {
        workspace_id: f.ws,
        database_id: f.database.id,
    })
    .unwrap();
    assert!(
        f.db.send_blocking(ListDatabasesCommand { workspace_id: f.ws })
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        f.db.send_blocking(GetDocumentCommand {
            workspace_id: f.ws,
            document_id: document.id,
        }),
        Err(CommandError::NotFound(_))
    ));
}
