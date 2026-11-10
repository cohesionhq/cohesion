use cohesion_core::model::{DatabaseDataType, DatabaseProperty, DatabaseValue};
use cohesion_core::protocol::{DatabaseCondition, DatabaseFilter, DatabaseSort};
use cohesion_core::{Document, DocumentType};

use crate::commands::database::tests::Fixture;
use crate::{
    BacklinksOfCommand, CommandError, CreateDatabaseRowDocumentCommand, CreateDocumentCommand,
    DeleteDatabaseRowCommand, DeleteDocumentCommand, EditDocumentCommand, GetDatabaseRowCommand,
    GetDocumentCommand, ListDatabaseRowsCommand, ListDeletedItemsCommand, ListDocumentsCommand,
    SearchDocumentsCommand, SearchWorkspaceCommand,
};

/// A database of five books, one per row in this order, with every type of
/// property; the fourth book holds no values at all, and each other leaves
/// one property empty.
struct Books {
    f: Fixture,
    title: DatabaseProperty,
    pages: DatabaseProperty,
    rating: DatabaseProperty,
    price: DatabaseProperty,
    read: DatabaseProperty,
    finished: DatabaseProperty,
    rows: Vec<i64>,
}

impl Books {
    fn new() -> Self {
        use DatabaseValue as V;
        let f = Fixture::new();
        let title = f.property("Title", DatabaseDataType::Text);
        let pages = f.property("Pages", DatabaseDataType::Integer);
        let rating = f.property("Rating", DatabaseDataType::Decimal);
        let price = f.property("Price", DatabaseDataType::Currency);
        let read = f.property("Read on", DatabaseDataType::Date);
        let finished = f.property("Finished", DatabaseDataType::Checkbox);
        let rows: Vec<i64> = (0..5).map(|_| f.row()).collect();
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
        filters: Vec<(&DatabaseProperty, DatabaseCondition)>,
        sort: Option<(&DatabaseProperty, bool)>,
    ) -> Result<Vec<usize>, CommandError> {
        let rows = self.f.db.send_blocking(ListDatabaseRowsCommand {
            workspace_id: self.f.ws,
            database_id: self.f.database.id,
            filters: filters
                .into_iter()
                .map(|(property, condition)| DatabaseFilter {
                    property_id: property.id,
                    condition,
                })
                .collect(),
            sort: sort.map(|(property, descending)| DatabaseSort {
                property_id: property.id,
                descending,
            }),
        })?;
        Ok(rows
            .iter()
            .map(|row| self.rows.iter().position(|id| *id == row.id).unwrap())
            .collect())
    }

    fn filter(&self, property: &DatabaseProperty, condition: DatabaseCondition) -> Vec<usize> {
        self.list(vec![(property, condition)], None).unwrap()
    }

    fn sort(&self, property: &DatabaseProperty, descending: bool) -> Vec<usize> {
        self.list(Vec::new(), Some((property, descending))).unwrap()
    }
}

#[test]
fn rows_come_with_their_values_in_the_order_they_were_added() {
    let books = Books::new();
    let rows = books
        .f
        .db
        .send_blocking(ListDatabaseRowsCommand {
            workspace_id: books.f.ws,
            database_id: books.f.database.id,
            filters: Vec::new(),
            sort: None,
        })
        .unwrap();
    assert_eq!(rows.len(), 5);
    let ids: Vec<i64> = rows.iter().map(|row| row.id).collect();
    assert_eq!(ids, books.rows);
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
    use DatabaseCondition as C;
    use DatabaseValue as V;
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
                    (pages, C::IsNotEmpty),
                    (
                        finished,
                        C::NotEquals {
                            value: V::Checkbox(false)
                        }
                    )
                ],
                Some((&books.pages, true))
            )
            .unwrap(),
        [4, 0]
    );
}

#[test]
fn a_filter_the_property_cant_meet_is_refused() {
    use DatabaseCondition as C;
    use DatabaseValue as V;
    let books = Books::new();
    let refused = |property: &DatabaseProperty, condition: C| {
        matches!(
            books.list(vec![(property, condition)], None),
            Err(CommandError::Validation(_))
        )
    };
    assert!(refused(
        &books.pages,
        C::Equals {
            value: V::Text("90".into())
        }
    ));
    assert!(refused(&books.pages, C::Contains { text: "9".into() }));
    assert!(refused(
        &books.title,
        C::GreaterThan {
            value: V::Text("a".into())
        }
    ));
    assert!(refused(
        &books.finished,
        C::LessThan {
            value: V::Checkbox(true)
        }
    ));
    assert!(refused(
        &books.read,
        C::Equals {
            value: V::Date("someday".into())
        }
    ));

    // A property of no database of these is not found, for a filter or a sort.
    let mut stranger = books.title.clone();
    stranger.id += 1000;
    assert!(matches!(
        books.list(vec![(&stranger, C::IsEmpty)], None),
        Err(CommandError::NotFound(_))
    ));
    assert!(matches!(
        books.list(Vec::new(), Some((&stranger, false))),
        Err(CommandError::NotFound(_))
    ));
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
            metadata: Default::default(),
            has_metadata: Vec::new(),
        })
        .unwrap()
        .into_iter()
        .map(|d| d.title)
        .collect()
}

fn row_document(books: &Books, row: usize) -> Document {
    books
        .f
        .db
        .send_blocking(CreateDatabaseRowDocumentCommand {
            workspace_id: books.f.ws,
            database_id: books.f.database.id,
            row_id: books.rows[row],
        })
        .unwrap()
}

#[test]
fn a_rows_document_stays_out_of_the_document_list_and_search_and_links_as_its_row() {
    let books = Books::new();
    let (db, ws) = (&books.f.db, books.f.ws);
    let plan = db
        .send_blocking(CreateDocumentCommand {
            workspace_id: ws,
            title: "Plan".to_string(),
            contents: "pears".to_string(),
            folder_id: None,
        })
        .unwrap();
    let document = row_document(&books, 0);
    assert_eq!(
        db.send_blocking(GetDatabaseRowCommand {
            workspace_id: ws,
            database_id: books.f.database.id,
            row_id: books.rows[0],
        })
        .unwrap()
        .document_id,
        Some(document.id)
    );
    // One document a row.
    assert!(matches!(
        db.send_blocking(CreateDatabaseRowDocumentCommand {
            workspace_id: ws,
            database_id: books.f.database.id,
            row_id: books.rows[0],
        }),
        Err(CommandError::Validation(_))
    ));
    // It opens and edits like any document…
    db.send_blocking(EditDocumentCommand {
        workspace_id: ws,
        document_id: document.id,
        contents: format!(
            "pears, see [the plan](cohesion://document/{ws}/{})",
            plan.id
        ),
    })
    .unwrap();
    // …but no list or search shows it, and a backlink shows its row.
    assert_eq!(listed(&books), ["Plan"]);
    let found = db
        .send_blocking(SearchWorkspaceCommand {
            workspace_id: ws,
            query: "pears".to_string(),
        })
        .unwrap();
    let found: Vec<i64> = found.iter().map(|hit| hit.entity_id).collect();
    assert_eq!(found, [plan.id]);
    assert!(
        db.send_blocking(SearchDocumentsCommand {
            workspace_id: ws,
            query: String::new(),
        })
        .unwrap()
        .iter()
        .all(|d| d.id != document.id)
    );
    let linking = db
        .send_blocking(BacklinksOfCommand {
            link: format!("cohesion://document/{ws}/{}", plan.id),
        })
        .unwrap();
    let linking: Vec<&str> = linking.iter().map(|embed| embed.link.as_str()).collect();
    assert_eq!(
        linking,
        [format!(
            "cohesion://database-row/{ws}/{}/{}",
            books.f.database.id, books.rows[0]
        )]
    );
}

#[test]
fn a_rows_document_goes_with_its_row_and_deleting_it_takes_it_off_the_row() {
    let books = Books::new();
    let (db, ws) = (&books.f.db, books.f.ws);
    let first = row_document(&books, 0);
    let second = row_document(&books, 1);
    let gone = |document: &Document| {
        matches!(
            db.send_blocking(GetDocumentCommand {
                workspace_id: ws,
                document_id: document.id,
            }),
            Err(CommandError::NotFound(_))
        )
    };

    db.send_blocking(DeleteDatabaseRowCommand {
        workspace_id: ws,
        database_id: books.f.database.id,
        row_id: books.rows[0],
    })
    .unwrap();
    assert!(gone(&first));

    // Deleting a row's document removes it for good, never to Rewind's
    // deleted items, and the row has no document after.
    let deleted = db
        .send_blocking(DeleteDocumentCommand {
            workspace_id: ws,
            document_id: second.id,
        })
        .unwrap();
    assert!(deleted.deleted);
    assert!(gone(&second));
    assert!(
        db.send_blocking(ListDeletedItemsCommand { workspace_id: ws })
            .unwrap()
            .is_empty()
    );
    let row = db
        .send_blocking(GetDatabaseRowCommand {
            workspace_id: ws,
            database_id: books.f.database.id,
            row_id: books.rows[1],
        })
        .unwrap();
    assert_eq!(row.document_id, None);
    assert!(matches!(
        row_document(&books, 1).document_type,
        DocumentType::DatabaseRow
    ));
}
