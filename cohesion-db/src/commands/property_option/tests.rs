use cohesion_core::model::{
    DocumentCell, DocumentFolderProperty, FormulaUpdateMode, IntelligenceAgentColor,
    PropertyDataType, PropertyOption, PropertyType, PropertyValue,
};
use cohesion_core::protocol::{
    NewPropertyOption, TableColumn, TableCondition, TableFilter, TableSort,
};

use crate::commands::document_folder_property::tests::{Fixture, definition};
use crate::{
    CommandError, CreateDocumentFolderPropertyCommand, CreatePropertyOptionCommand,
    DeletePropertyOptionCommand, ListStaleTableRowsCommand, ListTableRowsCommand,
    MovePropertyOptionCommand, RenamePropertyOptionCommand,
    SetDocumentFolderPropertyFormulaCommand, SetDocumentFolderPropertyTypeCommand,
    SetPropertyOptionColorCommand,
};

use IntelligenceAgentColor::{Blue, Green};
use PropertyDataType as T;
use PropertyValue as V;

fn new_option(name: &str, color: Option<IntelligenceAgentColor>) -> NewPropertyOption {
    NewPropertyOption {
        name: name.to_string(),
        color,
    }
}

impl Fixture {
    /// A choice property starting with `options`, by name and color.
    fn choice(
        &self,
        name: &str,
        options: &[(&str, Option<IntelligenceAgentColor>)],
    ) -> Result<DocumentFolderProperty, CommandError> {
        self.db.send_blocking(CreateDocumentFolderPropertyCommand {
            workspace_id: self.ws,
            folder_id: self.folder.id,
            name: name.to_string(),
            data_type: T::Choice,
            formula: None,
            options: options
                .iter()
                .map(|(name, color)| new_option(name, *color))
                .collect(),
        })
    }

    fn add_option(
        &self,
        property: &DocumentFolderProperty,
        name: &str,
    ) -> Result<DocumentFolderProperty, CommandError> {
        self.db.send_blocking(CreatePropertyOptionCommand {
            workspace_id: self.ws,
            folder_id: self.folder.id,
            property_id: property.id,
            name: name.to_string(),
            color: None,
        })
    }

    fn rename_option(
        &self,
        property: &DocumentFolderProperty,
        option_id: i64,
        name: &str,
    ) -> Result<DocumentFolderProperty, CommandError> {
        self.db.send_blocking(RenamePropertyOptionCommand {
            workspace_id: self.ws,
            folder_id: self.folder.id,
            property_id: property.id,
            option_id,
            name: name.to_string(),
        })
    }

    fn delete_option(
        &self,
        property: &DocumentFolderProperty,
        option_id: i64,
    ) -> DocumentFolderProperty {
        self.db
            .send_blocking(DeletePropertyOptionCommand {
                workspace_id: self.ws,
                folder_id: self.folder.id,
                property_id: property.id,
                option_id,
            })
            .unwrap()
    }

    fn set_type(
        &self,
        property: &DocumentFolderProperty,
        data_type: T,
    ) -> Result<DocumentFolderProperty, CommandError> {
        self.db.send_blocking(SetDocumentFolderPropertyTypeCommand {
            workspace_id: self.ws,
            folder_id: self.folder.id,
            property_id: property.id,
            data_type,
        })
    }

    /// The rows, by document id, whose value of `property` meets
    /// `condition`, or all of them sorted on it.
    fn rows(
        &self,
        property: &DocumentFolderProperty,
        condition: Option<TableCondition>,
        descending: Option<bool>,
    ) -> Vec<i64> {
        let column = TableColumn::Property {
            property_id: property.id,
        };
        self.db
            .send_blocking(ListTableRowsCommand {
                workspace_id: self.ws,
                folder_id: self.folder.id,
                filters: condition
                    .into_iter()
                    .map(|condition| TableFilter { column, condition })
                    .collect(),
                sort: descending.map(|descending| TableSort { column, descending }),
            })
            .unwrap()
            .into_iter()
            .map(|row| row.document_id)
            .collect()
    }

    /// Each row's value of `property`, by document, in creation order.
    fn values_of(&self, property: &DocumentFolderProperty, rows: &[i64]) -> Vec<Option<V>> {
        rows.iter()
            .map(|row| {
                self.cells(*row)
                    .into_iter()
                    .find(|cell| cell.property_id == property.id)
                    .map(|cell| cell.value)
            })
            .collect()
    }
}

/// The names of `property`'s options, in their order.
fn names(property: &DocumentFolderProperty) -> Vec<&str> {
    property
        .kind
        .options()
        .iter()
        .map(|option| option.name.as_str())
        .collect()
}

/// The option of `property` named `name`.
fn option<'p>(property: &'p DocumentFolderProperty, name: &str) -> &'p PropertyOption {
    property
        .kind
        .options()
        .iter()
        .find(|option| option.name == name)
        .unwrap()
}

#[test]
fn a_choice_property_starts_with_its_options_and_carries_them() {
    let f = Fixture::new();
    let status = f
        .choice(
            "Status",
            &[
                ("To read", Some(Blue)),
                ("Reading", None),
                ("Read", Some(Green)),
            ],
        )
        .unwrap();
    assert_eq!(status.data_type(), T::Choice);
    assert_eq!(names(&status), ["To read", "Reading", "Read"]);
    assert_eq!(option(&status, "To read").color, Some(Blue));
    assert_eq!(option(&status, "Reading").color, None);

    // On the wire the type names itself, and only a choice has options.
    let json = serde_json::to_value(&status).unwrap();
    assert_eq!(json["data_type"], "choice");
    assert_eq!(json["options"][2]["name"], "Read");
    assert_eq!(json["options"][2]["color"], "green");
    let pages = f.property("Pages", T::Integer);
    let json = serde_json::to_value(&pages).unwrap();
    assert_eq!(json["data_type"], "integer");
    assert!(json.get("options").is_none());
    let back: DocumentFolderProperty =
        serde_json::from_value(serde_json::to_value(&status).unwrap()).unwrap();
    assert_eq!(back.kind, status.kind);
}

#[test]
fn options_have_names_unique_ignoring_case_and_only_a_choice_has_them() {
    let f = Fixture::new();
    let refused = |result: Result<DocumentFolderProperty, CommandError>| {
        matches!(result, Err(CommandError::Validation(_)))
    };
    assert!(refused(
        f.choice("Status", &[("Read", None), ("read", None)])
    ));
    assert!(refused(f.choice("Status", &[("", None)])));
    // Another type takes no options, at creation or after.
    assert!(refused(f.db.send_blocking(
        CreateDocumentFolderPropertyCommand {
            workspace_id: f.ws,
            folder_id: f.folder.id,
            name: "Pages".to_string(),
            data_type: T::Integer,
            formula: None,
            options: vec![new_option("Many", None)],
        }
    )));
    let pages = f.property("Pages", T::Integer);
    assert!(refused(f.add_option(&pages, "Many")));

    // A choice may start with none.
    let status = f.choice("Status", &[]).unwrap();
    assert!(names(&status).is_empty());
    let status = f.add_option(&status, "Read").unwrap();
    assert!(refused(f.add_option(&status, "READ")));
    let status = f.add_option(&status, "Reading").unwrap();
    let read = option(&status, "Read").id;
    assert!(refused(f.rename_option(&status, read, "reading")));
    assert!(refused(f.rename_option(&status, read, "")));
    // An option may take another case of its own name.
    let status = f.rename_option(&status, read, "READ").unwrap();
    assert_eq!(names(&status), ["READ", "Reading"]);
}

#[test]
fn options_are_recolored_and_reordered() {
    let f = Fixture::new();
    let status = f
        .choice(
            "Status",
            &[("To read", None), ("Reading", None), ("Read", None)],
        )
        .unwrap();
    let read = option(&status, "Read").id;
    let status =
        f.db.send_blocking(SetPropertyOptionColorCommand {
            workspace_id: f.ws,
            folder_id: f.folder.id,
            property_id: status.id,
            option_id: read,
            color: Some(Green),
        })
        .unwrap();
    assert_eq!(option(&status, "Read").color, Some(Green));
    let status =
        f.db.send_blocking(MovePropertyOptionCommand {
            workspace_id: f.ws,
            folder_id: f.folder.id,
            property_id: status.id,
            option_id: read,
            before_id: Some(option(&status, "To read").id),
        })
        .unwrap();
    assert_eq!(names(&status), ["Read", "To read", "Reading"]);
    let status =
        f.db.send_blocking(MovePropertyOptionCommand {
            workspace_id: f.ws,
            folder_id: f.folder.id,
            property_id: status.id,
            option_id: read,
            before_id: None,
        })
        .unwrap();
    assert_eq!(names(&status), ["To read", "Reading", "Read"]);
    let status =
        f.db.send_blocking(SetPropertyOptionColorCommand {
            workspace_id: f.ws,
            folder_id: f.folder.id,
            property_id: status.id,
            option_id: read,
            color: None,
        })
        .unwrap();
    assert_eq!(option(&status, "Read").color, None);
}

#[test]
fn a_value_names_an_option_of_its_own_property() {
    let f = Fixture::new();
    let status = f.choice("Status", &[("Read", None)]).unwrap();
    let format = f.choice("Format", &[("Paperback", None)]).unwrap();
    let row = f.row();
    let read = option(&status, "Read").id;
    let paperback = option(&format, "Paperback").id;
    assert_eq!(
        f.set(row, &status, Some(V::Choice(read))).unwrap(),
        [DocumentCell {
            property_id: status.id,
            value: V::Choice(read),
        }]
    );
    // Another property's option, or one that doesn't exist, isn't one.
    for option_id in [paperback, 9999] {
        assert!(matches!(
            f.set(row, &status, Some(V::Choice(option_id))),
            Err(CommandError::Validation(_))
        ));
    }
    // Nor is text, and a choice is no text.
    assert!(f.set(row, &status, Some(V::Text("Read".into()))).is_err());
    let notes = f.property("Notes", T::Text);
    assert!(f.set(row, &notes, Some(V::Choice(read))).is_err());
    f.set(row, &status, None).unwrap();
    assert!(f.cells(row).is_empty());
}

#[test]
fn renaming_an_option_keeps_values_and_deleting_one_empties_them() {
    let f = Fixture::new();
    let status = f
        .choice("Status", &[("To read", None), ("Read", None)])
        .unwrap();
    let rows = [f.row(), f.row(), f.row()];
    let (to_read, read) = (option(&status, "To read").id, option(&status, "Read").id);
    f.set(rows[0], &status, Some(V::Choice(read))).unwrap();
    f.set(rows[1], &status, Some(V::Choice(to_read))).unwrap();
    f.set(rows[2], &status, Some(V::Choice(read))).unwrap();

    let status = f.rename_option(&status, read, "Finished").unwrap();
    assert_eq!(names(&status), ["To read", "Finished"]);
    assert_eq!(
        f.values_of(&status, &rows),
        [
            Some(V::Choice(read)),
            Some(V::Choice(to_read)),
            Some(V::Choice(read))
        ]
    );

    let status = f.delete_option(&status, read);
    assert_eq!(names(&status), ["To read"]);
    assert_eq!(
        f.values_of(&status, &rows),
        [None, Some(V::Choice(to_read)), None]
    );
    assert_eq!(f.stored(&status).len(), 1);
}

#[test]
fn choices_filter_by_option_and_sort_by_the_options_order() {
    let f = Fixture::new();
    let status = f
        .choice(
            "Status",
            &[("To read", None), ("Reading", None), ("Read", None)],
        )
        .unwrap();
    let rows = [f.row(), f.row(), f.row(), f.row()];
    let id = |name| option(&status, name).id;
    f.set(rows[0], &status, Some(V::Choice(id("Read"))))
        .unwrap();
    f.set(rows[1], &status, Some(V::Choice(id("To read"))))
        .unwrap();
    f.set(rows[3], &status, Some(V::Choice(id("Reading"))))
        .unwrap();

    let equals = |name| TableCondition::Equals {
        value: V::Choice(id(name)),
    };
    let not_equals = |name| TableCondition::NotEquals {
        value: V::Choice(id(name)),
    };
    assert_eq!(f.rows(&status, Some(equals("Read")), None), [rows[0]]);
    assert_eq!(
        f.rows(&status, Some(not_equals("Read")), None),
        [rows[1], rows[2], rows[3]]
    );
    assert_eq!(
        f.rows(&status, Some(TableCondition::IsEmpty), None),
        [rows[2]]
    );
    assert_eq!(
        f.rows(&status, Some(TableCondition::IsNotEmpty), None),
        [rows[0], rows[1], rows[3]]
    );
    // A choice is no text and has no order to compare against.
    for condition in [
        TableCondition::Contains { text: "Re".into() },
        TableCondition::GreaterThan {
            value: V::Choice(id("Read")),
        },
        TableCondition::Equals {
            value: V::Text("Read".into()),
        },
    ] {
        assert!(matches!(
            f.db.send_blocking(ListTableRowsCommand {
                workspace_id: f.ws,
                folder_id: f.folder.id,
                filters: vec![TableFilter {
                    column: TableColumn::Property {
                        property_id: status.id
                    },
                    condition,
                }],
                sort: None,
            }),
            Err(CommandError::Validation(_))
        ));
    }

    // By the options' order, not their names; empty last either way.
    assert_eq!(
        f.rows(&status, None, Some(false)),
        [rows[1], rows[3], rows[0], rows[2]]
    );
    assert_eq!(
        f.rows(&status, None, Some(true)),
        [rows[0], rows[3], rows[1], rows[2]]
    );
    // Reordering the options reorders the sort.
    f.db.send_blocking(MovePropertyOptionCommand {
        workspace_id: f.ws,
        folder_id: f.folder.id,
        property_id: status.id,
        option_id: id("Read"),
        before_id: Some(id("To read")),
    })
    .unwrap();
    assert_eq!(
        f.rows(&status, None, Some(false)),
        [rows[0], rows[1], rows[3], rows[2]]
    );
}

#[test]
fn text_becomes_a_choice_of_each_distinct_value_and_back() {
    let f = Fixture::new();
    let status = f.property("Status", T::Text);
    let rows: Vec<i64> = (0..5).map(|_| f.row()).collect();
    for (row, text) in rows
        .iter()
        .zip(["reading", "Read", "Reading", "", "To read"])
    {
        f.set(*row, &status, Some(V::Text(text.into()))).unwrap();
    }

    // An option for each text, ignoring case, alphabetically, with no
    // color; nothing is lost.
    let status = f.set_type(&status, T::Choice).unwrap();
    assert_eq!(names(&status), ["Read", "Reading", "To read"]);
    assert!(status.kind.options().iter().all(|o| o.color.is_none()));
    let id = |name| Some(V::Choice(option(&status, name).id));
    assert_eq!(
        f.values_of(&status, &rows),
        [
            id("Reading"),
            id("Read"),
            id("Reading"),
            None,
            id("To read")
        ]
    );

    // Back to text, each value is its option's name, and the options go.
    let status = f
        .rename_option(&status, option(&status, "Read").id, "Done")
        .unwrap();
    let status = f.set_type(&status, T::Text).unwrap();
    assert_eq!(status.kind, PropertyType::Text);
    assert_eq!(
        f.values_of(&status, &rows),
        [
            Some(V::Text("Reading".into())),
            Some(V::Text("Done".into())),
            Some(V::Text("Reading".into())),
            None,
            Some(V::Text("To read".into())),
        ]
    );
    // Becoming a choice again starts from those texts alone.
    let status = f.set_type(&status, T::Choice).unwrap();
    assert_eq!(names(&status), ["Done", "Reading", "To read"]);
    // Changing to the type it has changes nothing.
    let again = f.set_type(&status, T::Choice).unwrap();
    assert_eq!(again.kind, status.kind);
}

#[test]
fn other_types_become_choices_by_their_text_and_choices_other_types_by_name() {
    let f = Fixture::new();
    let pages = f.property("Pages", T::Integer);
    let rows = [f.row(), f.row(), f.row()];
    f.set(rows[0], &pages, Some(V::Integer(412))).unwrap();
    f.set(rows[1], &pages, Some(V::Integer(90))).unwrap();
    f.set(rows[2], &pages, Some(V::Integer(412))).unwrap();

    let pages = f.set_type(&pages, T::Choice).unwrap();
    // Alphabetically, as text.
    assert_eq!(names(&pages), ["412", "90"]);
    let pages = f
        .rename_option(&pages, option(&pages, "90").id, "ninety")
        .unwrap();
    // A name that isn't one of the new type is cleared.
    let pages = f.set_type(&pages, T::Integer).unwrap();
    assert_eq!(
        f.values_of(&pages, &rows),
        [Some(V::Integer(412)), None, Some(V::Integer(412))]
    );
    assert!(pages.kind.options().is_empty());
}

#[test]
fn a_formula_is_never_a_choice() {
    let f = Fixture::new();
    let refused = |result: Result<DocumentFolderProperty, CommandError>| {
        matches!(result, Err(CommandError::Validation(_)))
    };
    assert!(refused(f.create_property(
        "Status",
        T::Choice,
        Some(definition("(row) => 1", FormulaUpdateMode::WhenChanged)),
    )));
    let status = f.choice("Status", &[("Read", None)]).unwrap();
    assert!(refused(f.db.send_blocking(
        SetDocumentFolderPropertyFormulaCommand {
            workspace_id: f.ws,
            folder_id: f.folder.id,
            property_id: status.id,
            formula: Some(definition("(row) => 1", FormulaUpdateMode::WhenChanged)),
        }
    )));
    let shout = f.formula("Shout", T::Text, "(row) => row.Status");
    assert!(refused(f.set_type(&shout, T::Choice)));
}

#[test]
fn a_formula_reads_a_choice_as_its_options_name() {
    let f = Fixture::new();
    let status = f.choice("Status", &[("Read", None)]).unwrap();
    f.formula("Shout", T::Text, "(row) => row.Status");
    let rows = [f.row(), f.row()];
    f.set(
        rows[0],
        &status,
        Some(V::Choice(option(&status, "Read").id)),
    )
    .unwrap();
    let inputs = |f: &Fixture| -> Vec<Vec<(String, Option<V>)>> {
        f.db.send_blocking(ListStaleTableRowsCommand {
            folder_id: f.folder.id,
        })
        .unwrap()
        .rows
        .into_iter()
        .map(|row| row.inputs)
        .collect()
    };
    assert_eq!(
        inputs(&f),
        [
            vec![("Status".to_string(), Some(V::Text("Read".into())))],
            vec![("Status".to_string(), None)],
        ]
    );
    // Renaming the option changes what the formula reads.
    f.rename_option(&status, option(&status, "Read").id, "Done")
        .unwrap();
    assert_eq!(
        inputs(&f)[0],
        [("Status".to_string(), Some(V::Text("Done".into())))]
    );
}
