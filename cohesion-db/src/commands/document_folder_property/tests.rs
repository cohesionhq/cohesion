use cohesion_core::model::{
    DocumentCell, DocumentFolder, DocumentFolderProperty, DocumentValue, FormulaUpdateMode,
    PropertyDataType, PropertyValue,
};
use cohesion_core::protocol::FormulaDefinition;

use crate::test_support::context;
use crate::{
    CommandContext, CommandError, CreateDocumentCommand, CreateDocumentFolderCommand,
    CreateDocumentFolderPropertyCommand, CreateWorkspaceCommand, DatabaseCommand,
    DatabaseCommandResult, DatabaseContext, DeleteDocumentFolderCommand,
    DeleteDocumentFolderPropertyCommand, Filtered, GetDocumentCommand, GetTableRowCommand,
    ListDocumentFolderPropertiesCommand, ListDocumentFoldersCommand,
    MoveDocumentFolderPropertyCommand, Read, ReadableDatabase,
    SetDocumentFolderPropertyTypeCommand, SetDocumentValueCommand,
};

/// Every stored value of one property, as `(document, text, integer, real)`,
/// by document.
struct StoredValues(i64);
impl DatabaseCommand for StoredValues {
    type Response = Vec<(i64, String, Option<i64>, Option<f64>)>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        let values: Vec<DocumentValue> = ctx
            .db
            .select()
            .filter_eq(DocumentValue::PROPERTY_ID, self.0)
            .order("document_id")
            .all()?;
        Ok(values
            .into_iter()
            .map(|v| (v.document_id, v.text_value, v.integer_value, v.real_value))
            .collect())
    }
}

/// A workspace with a folder, "Books", to make a table of.
pub(crate) struct Fixture {
    pub db: DatabaseContext,
    pub ws: i64,
    pub folder: DocumentFolder,
}

impl Fixture {
    pub fn new() -> Self {
        Self::on(context())
    }

    /// A fixture whose database is `db`.
    pub fn on(db: DatabaseContext) -> Self {
        let ws = db
            .send_blocking(CreateWorkspaceCommand {
                title: "Home".to_string(),
                emoji: None,
            })
            .unwrap()
            .id;
        let mut f = Self {
            db,
            ws,
            folder: DocumentFolder {
                id: 0,
                workspace_id: ws,
                parent_id: None,
                title: String::new(),
                sort_order: 0,
                deleted: false,
                is_table: false,
            },
        };
        f.folder = f.new_folder("Books");
        f
    }

    /// Another top-level folder of the workspace.
    pub fn new_folder(&self, title: &str) -> DocumentFolder {
        self.db
            .send_blocking(CreateDocumentFolderCommand {
                workspace_id: self.ws,
                title: title.to_string(),
                parent_id: None,
            })
            .unwrap()
    }

    /// The fixture's folder as listed now.
    pub fn folder_now(&self, folder_id: i64) -> DocumentFolder {
        self.db
            .send_blocking(ListDocumentFoldersCommand {
                workspace_id: self.ws,
            })
            .unwrap()
            .into_iter()
            .find(|folder| folder.id == folder_id)
            .unwrap()
    }

    pub fn property(&self, name: &str, data_type: PropertyDataType) -> DocumentFolderProperty {
        self.create_property(name, data_type, None).unwrap()
    }

    /// A formula property computing `source` when what it reads changes.
    pub fn formula(
        &self,
        name: &str,
        data_type: PropertyDataType,
        source: &str,
    ) -> DocumentFolderProperty {
        self.create_property(
            name,
            data_type,
            Some(definition(source, FormulaUpdateMode::WhenChanged)),
        )
        .unwrap()
    }

    pub fn create_property(
        &self,
        name: &str,
        data_type: PropertyDataType,
        formula: Option<FormulaDefinition>,
    ) -> Result<DocumentFolderProperty, CommandError> {
        self.db.send_blocking(CreateDocumentFolderPropertyCommand {
            workspace_id: self.ws,
            folder_id: self.folder.id,
            name: name.to_string(),
            data_type,
            formula,
            options: Vec::new(),
        })
    }

    /// A new document in the fixture's folder: a row of its table.
    pub fn row(&self) -> i64 {
        self.document_in(Some(self.folder.id), "Untitled")
    }

    /// A new document titled `title` in `folder_id`, or the Inbox.
    pub fn document_in(&self, folder_id: Option<i64>, title: &str) -> i64 {
        self.db
            .send_blocking(CreateDocumentCommand {
                workspace_id: self.ws,
                title: title.to_string(),
                contents: String::new(),
                folder_id,
            })
            .unwrap()
            .id
    }

    pub fn set(
        &self,
        document_id: i64,
        property: &DocumentFolderProperty,
        value: Option<PropertyValue>,
    ) -> Result<Vec<DocumentCell>, CommandError> {
        self.db
            .send_blocking(SetDocumentValueCommand {
                workspace_id: self.ws,
                folder_id: property.folder_id,
                document_id,
                property_id: property.id,
                value,
            })
            .map(|row| row.cells)
    }

    pub fn cells(&self, document_id: i64) -> Vec<DocumentCell> {
        self.db
            .send_blocking(GetTableRowCommand {
                workspace_id: self.ws,
                folder_id: self.folder.id,
                document_id,
            })
            .unwrap()
            .cells
    }

    pub fn retype(&self, property: &DocumentFolderProperty, data_type: PropertyDataType) {
        self.db
            .send_blocking(SetDocumentFolderPropertyTypeCommand {
                workspace_id: self.ws,
                folder_id: self.folder.id,
                property_id: property.id,
                data_type,
            })
            .unwrap();
    }

    pub fn stored(
        &self,
        property: &DocumentFolderProperty,
    ) -> Vec<(i64, String, Option<i64>, Option<f64>)> {
        self.db.send_blocking(StoredValues(property.id)).unwrap()
    }
}
/// A formula as a request defines it.
pub fn definition(source: &str, update_mode: FormulaUpdateMode) -> FormulaDefinition {
    FormulaDefinition {
        source: source.to_string(),
        update_mode,
    }
}

fn cell(property: &DocumentFolderProperty, value: PropertyValue) -> DocumentCell {
    DocumentCell {
        property_id: property.id,
        value,
    }
}

#[test]
fn a_value_is_stored_in_every_representation_its_type_gives() {
    use PropertyDataType::*;
    let f = Fixture::new();
    let cases = [
        (Text, PropertyValue::Text("Dune".into()), "Dune", None, None),
        (
            Integer,
            PropertyValue::Integer(412),
            "412",
            Some(412),
            Some(412.0),
        ),
        (Decimal, PropertyValue::Decimal(4.5), "4.5", None, Some(4.5)),
        (
            Currency,
            PropertyValue::Currency(9.99),
            "9.99",
            None,
            Some(9.99),
        ),
        (
            Checkbox,
            PropertyValue::Checkbox(true),
            "true",
            Some(1),
            None,
        ),
        (
            Date,
            PropertyValue::Date("1965-08-01".into()),
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
    let pages = f.property("Pages", PropertyDataType::Integer);
    let title = f.property("Title", PropertyDataType::Text);
    let read = f.property("Read on", PropertyDataType::Date);
    let row = f.row();
    // Text can't stand in for a number, nor a number for text.
    for (property, value) in [
        (&pages, PropertyValue::Text("412".into())),
        (&pages, PropertyValue::Decimal(412.0)),
        (&title, PropertyValue::Integer(1)),
        (&read, PropertyValue::Date("1965-02-30".into())),
    ] {
        assert!(matches!(
            f.set(row, property, Some(value)),
            Err(CommandError::Validation(_))
        ));
    }
    assert!(f.cells(row).is_empty());

    f.set(row, &pages, Some(PropertyValue::Integer(412)))
        .unwrap();
    f.set(row, &title, Some(PropertyValue::Text("Dune".into())))
        .unwrap();
    // Empty text is no value, and absent empties a value.
    f.set(row, &title, Some(PropertyValue::Text(String::new())))
        .unwrap();
    f.set(row, &pages, None).unwrap();
    assert!(f.cells(row).is_empty());
    assert!(f.stored(&pages).is_empty());
}

#[test]
fn a_property_changing_type_keeps_the_values_whose_text_is_one() {
    let f = Fixture::new();
    let property = f.property("Pages", PropertyDataType::Text);
    let rows: Vec<i64> = (0..4).map(|_| f.row()).collect();
    for (row, text) in rows.iter().zip(["412", "4.5", "many", "0"]) {
        f.set(*row, &property, Some(PropertyValue::Text(text.into())))
            .unwrap();
    }

    f.retype(&property, PropertyDataType::Integer);
    assert_eq!(
        f.stored(&property),
        [
            (rows[0], "412".to_string(), Some(412), Some(412.0)),
            (rows[3], "0".to_string(), Some(0), Some(0.0)),
        ]
    );

    f.retype(&property, PropertyDataType::Decimal);
    assert_eq!(
        f.stored(&property),
        [
            (rows[0], "412".to_string(), None, Some(412.0)),
            (rows[3], "0".to_string(), None, Some(0.0)),
        ]
    );
    assert_eq!(
        f.cells(rows[0]),
        [cell(&property, PropertyValue::Decimal(412.0))]
    );

    // A checkbox reads only `true` and `false`; text keeps everything.
    f.retype(&property, PropertyDataType::Checkbox);
    assert!(f.stored(&property).is_empty());
    let done = f.property("Done", PropertyDataType::Checkbox);
    f.set(rows[1], &done, Some(PropertyValue::Checkbox(false)))
        .unwrap();
    f.retype(&done, PropertyDataType::Text);
    assert_eq!(
        f.stored(&done),
        [(rows[1], "false".to_string(), None, None)]
    );
}

#[test]
fn deleting_a_property_takes_its_values_and_properties_keep_their_order() {
    let f = Fixture::new();
    let title = f.property("Title", PropertyDataType::Text);
    let pages = f.property("Pages", PropertyDataType::Integer);
    let read = f.property("Read on", PropertyDataType::Date);
    let names = |f: &Fixture| -> Vec<String> {
        f.db.send_blocking(ListDocumentFolderPropertiesCommand {
            workspace_id: f.ws,
            folder_id: f.folder.id,
        })
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect()
    };
    assert_eq!(names(&f), ["Title", "Pages", "Read on"]);
    f.db.send_blocking(MoveDocumentFolderPropertyCommand {
        workspace_id: f.ws,
        folder_id: f.folder.id,
        property_id: read.id,
        before_id: Some(title.id),
    })
    .unwrap();
    assert_eq!(names(&f), ["Read on", "Title", "Pages"]);

    let row = f.row();
    f.set(row, &pages, Some(PropertyValue::Integer(412)))
        .unwrap();
    f.set(row, &title, Some(PropertyValue::Text("Dune".into())))
        .unwrap();
    f.db.send_blocking(DeleteDocumentFolderPropertyCommand {
        workspace_id: f.ws,
        folder_id: f.folder.id,
        property_id: pages.id,
    })
    .unwrap();
    assert!(f.stored(&pages).is_empty());
    assert_eq!(
        f.cells(row),
        [cell(&title, PropertyValue::Text("Dune".into()))]
    );
    assert_eq!(names(&f), ["Read on", "Title"]);
}

#[test]
fn a_folder_is_a_table_while_it_has_a_property_and_is_its_workspaces() {
    let f = Fixture::new();
    assert!(!f.folder_now(f.folder.id).is_table);
    let title = f.property("Title", PropertyDataType::Text);
    let pages = f.property("Pages", PropertyDataType::Integer);
    assert!(f.folder_now(f.folder.id).is_table);
    let row = f.row();
    f.set(row, &pages, Some(PropertyValue::Integer(412)))
        .unwrap();

    // Another workspace can't reach it.
    let other =
        f.db.send_blocking(CreateWorkspaceCommand {
            title: "Work".to_string(),
            emoji: None,
        })
        .unwrap()
        .id;
    assert!(matches!(
        f.db.send_blocking(ListDocumentFolderPropertiesCommand {
            workspace_id: other,
            folder_id: f.folder.id,
        }),
        Err(CommandError::NotFound(_))
    ));

    // Deleting its last property makes it a plain folder again; its
    // documents stay.
    for property in [&title, &pages] {
        f.db.send_blocking(DeleteDocumentFolderPropertyCommand {
            workspace_id: f.ws,
            folder_id: f.folder.id,
            property_id: property.id,
        })
        .unwrap();
    }
    assert!(!f.folder_now(f.folder.id).is_table);
    assert!(f.stored(&pages).is_empty());
    assert!(f.cells(row).is_empty());
    let document =
        f.db.send_blocking(GetDocumentCommand {
            workspace_id: f.ws,
            document_id: row,
        })
        .unwrap();
    assert_eq!(document.folder_id, Some(f.folder.id));
}

#[test]
fn deleting_a_table_folder_takes_its_properties_and_values_and_keeps_its_documents() {
    let f = Fixture::new();
    let pages = f.property("Pages", PropertyDataType::Integer);
    let row = f.row();
    f.set(row, &pages, Some(PropertyValue::Integer(412)))
        .unwrap();
    f.db.send_blocking(DeleteDocumentFolderCommand {
        workspace_id: f.ws,
        folder_id: f.folder.id,
    })
    .unwrap();
    assert!(f.stored(&pages).is_empty());
    let document =
        f.db.send_blocking(GetDocumentCommand {
            workspace_id: f.ws,
            document_id: row,
        })
        .unwrap();
    assert_eq!(document.folder_id, None);
}
