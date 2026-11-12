use cohesion_core::model::{DocumentFolderProperty, PropertyDataType, PropertyType, PropertyValue};
use cohesion_core::protocol::{
    NewPropertyOption, TableColumn, TableCondition, TableFilter, TableSort,
};

use crate::commands::document_folder_property::tests::Fixture;
use crate::{
    CommandError, CreateDocumentFolderCommand, CreateDocumentFolderPropertyCommand,
    CreateWorkspaceCommand, DeleteDocumentCommand, GetDocumentCommand, GetTableRowCommand,
    ListDocumentsCommand, ListTableRowsCommand, MoveDocumentCommand, SearchWorkspaceCommand,
    SetDocumentValueCommand,
};

/// The column of `property`.
fn col(property: &DocumentFolderProperty) -> TableColumn {
    TableColumn::Property {
        property_id: property.id,
    }
}

/// A table of five books, one document per row in this order, titled
/// Dune, emma, Middlemarch, Untitled and Untitled, with every type of
/// property; the fourth book holds no values at all, and each other leaves
/// one property empty.
struct Books {
    f: Fixture,
    title: DocumentFolderProperty,
    pages: DocumentFolderProperty,
    rating: DocumentFolderProperty,
    price: DocumentFolderProperty,
    read: DocumentFolderProperty,
    finished: DocumentFolderProperty,
    rows: Vec<i64>,
}

impl Books {
    fn new() -> Self {
        use PropertyValue as V;
        let f = Fixture::new();
        let title = f.property("Title", PropertyDataType::Text);
        let pages = f.property("Pages", PropertyDataType::Integer);
        let rating = f.property("Rating", PropertyDataType::Decimal);
        let price = f.property("Price", PropertyDataType::Currency);
        let read = f.property("Read on", PropertyDataType::Date);
        let finished = f.property("Finished", PropertyDataType::Checkbox);
        let rows: Vec<i64> = ["Dune", "emma", "Middlemarch", "Untitled", "Untitled"]
            .into_iter()
            .map(|title| f.document_in(Some(f.folder.id), title))
            .collect();
        let books: [[Option<V>; 6]; 5] = [
            [
                Some(V::Text("Dune".into())),
                Some(V::Integer(412)),
                Some(V::Decimal(4.5)),
                Some(V::Currency(9.99)),
                Some(V::Date("2024-03-01".into())),
                None,
            ],
            [
                Some(V::Text("emma".into())),
                Some(V::Integer(90)),
                None,
                Some(V::Currency(12.5)),
                Some(V::Date("2023-12-31".into())),
                Some(V::Checkbox(false)),
            ],
            [
                Some(V::Text("Middlemarch".into())),
                None,
                Some(V::Decimal(3.25)),
                Some(V::Currency(4.0)),
                Some(V::Date("2025-01-15".into())),
                Some(V::Checkbox(true)),
            ],
            [None, None, None, None, None, None],
            [
                None,
                Some(V::Integer(1000)),
                Some(V::Decimal(4.5)),
                Some(V::Currency(30.0)),
                None,
                Some(V::Checkbox(true)),
            ],
        ];
        let properties = [&title, &pages, &rating, &price, &read, &finished];
        for (row, values) in rows.iter().zip(books) {
            for (property, value) in properties.iter().zip(values) {
                if value.is_some() {
                    f.set(*row, property, value).unwrap();
                }
            }
        }
        Self {
            f,
            title,
            pages,
            rating,
            price,
            read,
            finished,
            rows,
        }
    }

    /// The books listed, as their row's place in `rows` (0 to 4).
    fn list(
        &self,
        filters: Vec<(TableColumn, TableCondition)>,
        sort: Option<(TableColumn, bool)>,
    ) -> Result<Vec<usize>, CommandError> {
        let rows = self.f.db.send_blocking(ListTableRowsCommand {
            workspace_id: self.f.ws,
            folder_id: self.f.folder.id,
            filters: filters
                .into_iter()
                .map(|(column, condition)| TableFilter { column, condition })
                .collect(),
            sort: sort.map(|(column, descending)| TableSort { column, descending }),
        })?;
        Ok(rows
            .iter()
            .map(|row| {
                self.rows
                    .iter()
                    .position(|id| *id == row.document_id)
                    .unwrap()
            })
            .collect())
    }

    fn filter(&self, property: &DocumentFolderProperty, condition: TableCondition) -> Vec<usize> {
        self.list(vec![(col(property), condition)], None).unwrap()
    }

    fn sort(&self, property: &DocumentFolderProperty, descending: bool) -> Vec<usize> {
        self.list(Vec::new(), Some((col(property), descending)))
            .unwrap()
    }
}

#[test]
fn rows_come_with_their_titles_and_values_in_the_order_they_were_created() {
    let books = Books::new();
    let rows = books
        .f
        .db
        .send_blocking(ListTableRowsCommand {
            workspace_id: books.f.ws,
            folder_id: books.f.folder.id,
            filters: Vec::new(),
            sort: None,
        })
        .unwrap();
    assert_eq!(rows.len(), 5);
    let ids: Vec<i64> = rows.iter().map(|row| row.document_id).collect();
    assert_eq!(ids, books.rows);
    assert_eq!(rows[1].title, "emma");
    assert_eq!(rows[1].folder_id, books.f.folder.id);
    // Cells in the properties' order, only where a value is held.
    let first: Vec<i64> = rows[0].cells.iter().map(|c| c.property_id).collect();
    assert_eq!(
        first,
        [
            books.title.id,
            books.pages.id,
            books.rating.id,
            books.price.id,
            books.read.id
        ]
    );
    assert!(rows[3].cells.is_empty());
}

#[test]
fn every_type_sorts_as_it_compares_with_empty_values_last_either_way() {
    let books = Books::new();
    // Text ignores case; ties keep the order rows were added.
    assert_eq!(books.sort(&books.title, false), [0, 1, 2, 3, 4]);
    assert_eq!(books.sort(&books.title, true), [2, 1, 0, 3, 4]);
    // Numbers as numbers, not as text ("1000" before "412" as text).
    assert_eq!(books.sort(&books.pages, false), [1, 0, 4, 2, 3]);
    assert_eq!(books.sort(&books.pages, true), [4, 0, 1, 2, 3]);
    assert_eq!(books.sort(&books.rating, false), [2, 0, 4, 1, 3]);
    assert_eq!(books.sort(&books.rating, true), [0, 4, 2, 1, 3]);
    assert_eq!(books.sort(&books.price, false), [2, 0, 1, 4, 3]);
    assert_eq!(books.sort(&books.read, false), [1, 0, 2, 3, 4]);
    assert_eq!(books.sort(&books.read, true), [2, 0, 1, 3, 4]);
    // Unchecked before checked; an empty checkbox is neither.
    assert_eq!(books.sort(&books.finished, false), [1, 2, 4, 0, 3]);
}

#[test]
fn every_type_filters_as_it_compares() {
    use PropertyValue as V;
    use TableCondition as C;
    let books = Books::new();
    // Text: equal and containing ignore case; an empty value contains nothing.
    let title = &books.title;
    assert_eq!(
        books.filter(
            title,
            C::Equals {
                value: V::Text("EMMA".into())
            }
        ),
        [1]
    );
    assert_eq!(
        books.filter(title, C::Contains { text: "M".into() }),
        [1, 2]
    );
    assert_eq!(
        books.filter(title, C::NotContains { text: "m".into() }),
        [0, 3, 4]
    );
    assert_eq!(
        books.filter(
            title,
            C::NotEquals {
                value: V::Text("dune".into())
            }
        ),
        [1, 2, 3, 4]
    );
    // Numbers compare as numbers; an empty value is neither greater nor less.
    let pages = &books.pages;
    assert_eq!(
        books.filter(
            pages,
            C::GreaterThan {
                value: V::Integer(100)
            }
        ),
        [0, 4]
    );
    assert_eq!(
        books.filter(
            pages,
            C::LessOrEqual {
                value: V::Integer(412)
            }
        ),
        [0, 1]
    );
    assert_eq!(
        books.filter(
            pages,
            C::Equals {
                value: V::Integer(90)
            }
        ),
        [1]
    );
    let rating = &books.rating;
    assert_eq!(
        books.filter(
            rating,
            C::GreaterOrEqual {
                value: V::Decimal(4.5)
            }
        ),
        [0, 4]
    );
    assert_eq!(
        books.filter(
            rating,
            C::LessThan {
                value: V::Decimal(4.0)
            }
        ),
        [2]
    );
    let price = &books.price;
    assert_eq!(
        books.filter(
            price,
            C::LessThan {
                value: V::Currency(10.0)
            }
        ),
        [0, 2]
    );
    // Dates compare by day.
    let read = &books.read;
    assert_eq!(
        books.filter(
            read,
            C::GreaterThan {
                value: V::Date("2024-01-01".into())
            }
        ),
        [0, 2]
    );
    assert_eq!(
        books.filter(
            read,
            C::Equals {
                value: V::Date("2023-12-31".into())
            }
        ),
        [1]
    );
    // A checkbox is checked, unchecked or empty.
    let finished = &books.finished;
    assert_eq!(
        books.filter(
            finished,
            C::Equals {
                value: V::Checkbox(true)
            }
        ),
        [2, 4]
    );
    assert_eq!(
        books.filter(
            finished,
            C::Equals {
                value: V::Checkbox(false)
            }
        ),
        [1]
    );
    assert_eq!(books.filter(finished, C::IsEmpty), [0, 3]);
    assert_eq!(books.filter(finished, C::IsNotEmpty), [1, 2, 4]);
    // Every filter holds, and the sort orders what is left.
    assert_eq!(
        books
            .list(
                vec![
                    (col(pages), C::IsNotEmpty),
                    (
                        col(finished),
                        C::NotEquals {
                            value: V::Checkbox(false)
                        }
                    )
                ],
                Some((col(&books.pages), true))
            )
            .unwrap(),
        [4, 0]
    );
}

#[test]
fn a_filter_the_property_cant_meet_is_refused() {
    use PropertyValue as V;
    use TableCondition as C;
    let books = Books::new();
    let refused = |column: TableColumn, condition: C| {
        matches!(
            books.list(vec![(column, condition)], None),
            Err(CommandError::Validation(_))
        )
    };
    assert!(refused(
        col(&books.pages),
        C::Equals {
            value: V::Text("90".into())
        }
    ));
    assert!(refused(col(&books.pages), C::Contains { text: "9".into() }));
    assert!(refused(
        col(&books.title),
        C::GreaterThan {
            value: V::Text("a".into())
        }
    ));
    assert!(refused(
        TableColumn::Title,
        C::LessThan {
            value: V::Text("a".into())
        }
    ));
    assert!(refused(
        TableColumn::Title,
        C::Equals {
            value: V::Integer(1)
        }
    ));
    assert!(refused(
        col(&books.finished),
        C::LessThan {
            value: V::Checkbox(true)
        }
    ));
    assert!(refused(
        col(&books.read),
        C::Equals {
            value: V::Date("someday".into())
        }
    ));

    // A property of another folder is not found, for a filter or a sort.
    let mut stranger = books.title.clone();
    stranger.id += 1000;
    assert!(matches!(
        books.list(vec![(col(&stranger), C::IsEmpty)], None),
        Err(CommandError::NotFound(_))
    ));
    assert!(matches!(
        books.list(Vec::new(), Some((col(&stranger), false))),
        Err(CommandError::NotFound(_))
    ));
}

#[test]
fn the_title_is_a_text_column_to_filter_and_sort_on() {
    use PropertyValue as V;
    use TableCondition as C;
    let books = Books::new();
    let title = |condition| books.list(vec![(TableColumn::Title, condition)], None);
    assert_eq!(
        title(C::Equals {
            value: V::Text("DUNE".into())
        })
        .unwrap(),
        [0]
    );
    assert_eq!(title(C::Contains { text: "M".into() }).unwrap(), [1, 2]);
    assert_eq!(
        title(C::NotContains { text: "m".into() }).unwrap(),
        [0, 3, 4]
    );
    assert_eq!(
        title(C::NotEquals {
            value: V::Text("untitled".into())
        })
        .unwrap(),
        [0, 1, 2]
    );
    assert!(title(C::IsEmpty).unwrap().is_empty());
    // Ignoring case, ties in the order they were created.
    let sort = |descending| {
        books
            .list(Vec::new(), Some((TableColumn::Title, descending)))
            .unwrap()
    };
    assert_eq!(sort(false), [0, 1, 2, 3, 4]);
    assert_eq!(sort(true), [3, 4, 2, 1, 0]);
}

fn listed(books: &Books) -> Vec<String> {
    books
        .f
        .db
        .send_blocking(ListDocumentsCommand {
            workspace_id: books.f.ws,
            document_type: None,
            journal_from: None,
            journal_to: None,
            ids: None,
            metadata: Default::default(),
            has_metadata: Vec::new(),
        })
        .unwrap()
        .into_iter()
        .map(|d| d.title)
        .collect()
}

#[test]
fn a_row_is_a_document_listed_searched_and_soft_deleted_like_any() {
    let books = Books::new();
    let (db, ws) = (&books.f.db, books.f.ws);
    let mut titles = listed(&books);
    titles.sort();
    assert_eq!(
        titles,
        ["Dune", "Middlemarch", "Untitled", "Untitled", "emma"]
    );
    let found = db
        .send_blocking(SearchWorkspaceCommand {
            workspace_id: ws,
            query: "Middlemarch".to_string(),
        })
        .unwrap();
    assert_eq!(
        found.iter().map(|hit| hit.entity_id).collect::<Vec<_>>(),
        [books.rows[2]]
    );

    // A deleted row leaves the table and keeps its values for Rewind.
    db.send_blocking(DeleteDocumentCommand {
        workspace_id: ws,
        document_id: books.rows[0],
    })
    .unwrap();
    assert_eq!(books.sort(&books.pages, false), [1, 4, 2, 3]);
    assert!(
        books
            .f
            .stored(&books.pages)
            .iter()
            .any(|(document, ..)| *document == books.rows[0])
    );
    // Its value can't be set while it is deleted.
    assert!(matches!(
        books
            .f
            .set(books.rows[0], &books.pages, Some(PropertyValue::Integer(1))),
        Err(CommandError::NotFound(_))
    ));
}

/// Moves `document_id` to `folder_id`, or the Inbox.
fn move_to(books: &Books, document_id: i64, folder_id: Option<i64>) -> Result<(), CommandError> {
    books
        .f
        .db
        .send_blocking(MoveDocumentCommand {
            workspace_id: books.f.ws,
            document_id,
            folder_id,
        })
        .map(|_| ())
}

#[test]
fn moving_a_document_out_of_a_table_or_into_another_lets_go_of_its_values() {
    use PropertyValue as V;
    let books = Books::new();
    let f = &books.f;
    let dune = books.rows[0];
    let held = |property: &DocumentFolderProperty, document: i64| {
        f.stored(property)
            .iter()
            .any(|(stored, ..)| *stored == document)
    };

    // Within its own folder nothing changes.
    move_to(&books, dune, Some(f.folder.id)).unwrap();
    assert_eq!(f.cells(dune).len(), 5);

    // Out to the Inbox: its values go, the document stays.
    move_to(&books, dune, None).unwrap();
    assert!(!held(&books.pages, dune));
    assert!(!held(&books.title, dune));
    assert_eq!(books.sort(&books.pages, false), [1, 4, 2, 3]);
    let document =
        f.db.send_blocking(GetDocumentCommand {
            workspace_id: f.ws,
            document_id: dune,
        })
        .unwrap();
    assert_eq!(
        (document.title.as_str(), document.folder_id),
        ("Dune", None)
    );

    // Back in: a row with no values, which can be given some.
    move_to(&books, dune, Some(f.folder.id)).unwrap();
    assert!(f.cells(dune).is_empty());
    f.set(dune, &books.pages, Some(V::Integer(412))).unwrap();

    // Into another table: it holds none of the old table's values, and none
    // of the new one's until they are set.
    let films = f.new_folder("Films");
    let year =
        f.db.send_blocking(crate::CreateDocumentFolderPropertyCommand {
            workspace_id: f.ws,
            folder_id: films.id,
            name: "Year".to_string(),
            data_type: PropertyDataType::Integer,
            formula: None,
            options: Vec::new(),
        })
        .unwrap();
    move_to(&books, dune, Some(films.id)).unwrap();
    assert!(!held(&books.pages, dune));
    let row =
        f.db.send_blocking(GetTableRowCommand {
            workspace_id: f.ws,
            folder_id: films.id,
            document_id: dune,
        })
        .unwrap();
    assert!(row.cells.is_empty());
    f.set(dune, &year, Some(V::Integer(1965))).unwrap();
    // A value of the old table can't be set on it now.
    assert!(matches!(
        f.set(dune, &books.pages, Some(V::Integer(1))),
        Err(CommandError::NotFound(_))
    ));

    // Into a plain folder: its values go too.
    let plain =
        f.db.send_blocking(CreateDocumentFolderCommand {
            workspace_id: f.ws,
            title: "Notes".to_string(),
            parent_id: None,
        })
        .unwrap();
    move_to(&books, dune, Some(plain.id)).unwrap();
    assert!(!held(&year, dune));
}

#[test]
fn a_refused_move_keeps_the_documents_values() {
    let books = Books::new();
    let f = &books.f;
    let other =
        f.db.send_blocking(CreateWorkspaceCommand {
            title: "Work".to_string(),
            emoji: None,
        })
        .unwrap()
        .id;
    let elsewhere =
        f.db.send_blocking(CreateDocumentFolderCommand {
            workspace_id: other,
            title: "Elsewhere".to_string(),
            parent_id: None,
        })
        .unwrap();
    assert!(move_to(&books, books.rows[0], Some(elsewhere.id)).is_err());
    assert_eq!(f.cells(books.rows[0]).len(), 5);
}

#[test]
fn a_value_is_only_set_on_a_document_of_the_propertys_folder() {
    let books = Books::new();
    let f = &books.f;
    let note = f.document_in(None, "Note");
    assert!(matches!(
        f.db.send_blocking(SetDocumentValueCommand {
            workspace_id: f.ws,
            folder_id: f.folder.id,
            document_id: note,
            property_id: books.pages.id,
            value: Some(PropertyValue::Integer(1)),
        }),
        Err(CommandError::NotFound(_))
    ));
}

/// A column a shared case names: a property by name, or `$title`.
fn case_column(properties: &[DocumentFolderProperty], name: &str) -> serde_json::Value {
    if name == "$title" {
        return serde_json::json!({ "kind": "title" });
    }
    let property = properties
        .iter()
        .find(|property| property.name == name)
        .unwrap_or_else(|| panic!("no property {name}"));
    serde_json::json!({ "kind": "property", "property_id": property.id })
}

/// A value a shared case writes, its choice named by its option, as the
/// wire carries it for the property `name`.
fn case_value(
    properties: &[DocumentFolderProperty],
    name: &str,
    mut value: serde_json::Value,
) -> PropertyValue {
    if value["type"] == "choice" {
        let property = properties
            .iter()
            .find(|property| property.name == name)
            .unwrap();
        let PropertyType::Choice { options } = &property.kind else {
            panic!("{name} isn't a choice");
        };
        let option = options
            .iter()
            .find(|option| value["value"] == option.name.as_str())
            .unwrap();
        value["value"] = option.id.into();
    }
    serde_json::from_value(value).unwrap()
}

/// The cases the web UI's mock runs too (`table_cases.json`): what the
/// daemon answers here is what the mock must.
#[test]
fn the_shared_table_cases_hold() {
    let shared: serde_json::Value = serde_json::from_str(include_str!("table_cases.json")).unwrap();
    let f = Fixture::new();
    let properties: Vec<DocumentFolderProperty> = shared["properties"]
        .as_array()
        .unwrap()
        .iter()
        .map(|property| {
            let options = property["options"].as_array().cloned().unwrap_or_default();
            f.db.send_blocking(CreateDocumentFolderPropertyCommand {
                workspace_id: f.ws,
                folder_id: f.folder.id,
                name: property["name"].as_str().unwrap().to_string(),
                data_type: serde_json::from_value(property["type"].clone()).unwrap(),
                formula: None,
                options: options
                    .iter()
                    .map(|name| NewPropertyOption {
                        name: name.as_str().unwrap().to_string(),
                        color: None,
                    })
                    .collect(),
            })
            .unwrap()
        })
        .collect();
    for row in shared["rows"].as_array().unwrap() {
        let id = f.document_in(Some(f.folder.id), row["title"].as_str().unwrap());
        for (name, value) in row["values"].as_object().unwrap() {
            let property = properties.iter().find(|p| &p.name == name).unwrap();
            let value = case_value(&properties, name, value.clone());
            f.set(id, property, Some(value)).unwrap();
        }
    }
    for case in shared["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let filters = case["filters"]
            .as_array()
            .unwrap()
            .iter()
            .map(|filter| {
                let column = filter["column"].as_str().unwrap();
                let mut condition = filter["condition"].clone();
                if !condition["value"].is_null() {
                    let value = case_value(&properties, column, condition["value"].clone());
                    condition["value"] = serde_json::to_value(value).unwrap();
                }
                TableFilter {
                    column: serde_json::from_value(case_column(&properties, column)).unwrap(),
                    condition: serde_json::from_value(condition).unwrap(),
                }
            })
            .collect();
        let sort = (!case["sort"].is_null()).then(|| TableSort {
            column: serde_json::from_value(case_column(
                &properties,
                case["sort"]["column"].as_str().unwrap(),
            ))
            .unwrap(),
            descending: case["sort"]["descending"].as_bool().unwrap(),
        });
        let titles: Vec<String> =
            f.db.send_blocking(ListTableRowsCommand {
                workspace_id: f.ws,
                folder_id: f.folder.id,
                filters,
                sort,
            })
            .unwrap_or_else(|error| panic!("{name}: {error:?}"))
            .into_iter()
            .map(|row| row.title)
            .collect();
        let expected: Vec<&str> = case["titles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|title| title.as_str().unwrap())
            .collect();
        assert_eq!(titles, expected, "{name}");
    }
}
