use super::{
    DatabaseCell, DatabaseDataType, DatabaseProperty, DatabaseRowEntry, DatabaseValue, is_day,
};

#[test]
fn each_type_gives_its_text_and_the_numbers_it_honestly_is() {
    let cases = [
        (DatabaseValue::Text("Ada".into()), "Ada", None, None),
        (DatabaseValue::Integer(42), "42", Some(42), Some(42.0)),
        (DatabaseValue::Decimal(3.5), "3.5", None, Some(3.5)),
        (DatabaseValue::Currency(12.99), "12.99", None, Some(12.99)),
        (DatabaseValue::Checkbox(true), "true", Some(1), None),
        (DatabaseValue::Checkbox(false), "false", Some(0), None),
        (
            DatabaseValue::Date("2026-11-09".into()),
            "2026-11-09",
            None,
            None,
        ),
    ];
    for (value, text, integer, real) in cases {
        assert_eq!(value.text(), text, "{value:?}");
        assert_eq!(value.integer(), integer, "{value:?}");
        assert_eq!(value.real(), real, "{value:?}");
        // What is stored as text reads back as the same value.
        assert_eq!(
            DatabaseValue::parse(value.data_type(), &value.text()),
            Some(value)
        );
    }
}

#[test]
fn text_parses_as_another_type_only_when_it_is_one() {
    use DatabaseDataType::*;
    assert_eq!(
        DatabaseValue::parse(Decimal, "42"),
        Some(DatabaseValue::Decimal(42.0))
    );
    assert_eq!(
        DatabaseValue::parse(Integer, "3"),
        Some(DatabaseValue::Integer(3))
    );
    assert_eq!(DatabaseValue::parse(Integer, "3.5"), None);
    assert_eq!(DatabaseValue::parse(Integer, "true"), None);
    assert_eq!(DatabaseValue::parse(Checkbox, "1"), None);
    assert_eq!(DatabaseValue::parse(Decimal, "inf"), None);
    assert_eq!(DatabaseValue::parse(Decimal, "NaN"), None);
    assert_eq!(DatabaseValue::parse(Date, "Monday"), None);
    assert_eq!(
        DatabaseValue::parse(Text, "true"),
        Some(DatabaseValue::Text("true".into()))
    );
}

#[test]
fn empty_text_is_no_value_and_bad_numbers_and_days_are_refused() {
    assert_eq!(DatabaseValue::Text(String::new()).checked(), Ok(None));
    assert!(DatabaseValue::Decimal(f64::NAN).checked().is_err());
    assert!(DatabaseValue::Currency(f64::INFINITY).checked().is_err());
    assert!(DatabaseValue::Date("2026-02-30".into()).checked().is_err());
}

#[test]
fn a_day_is_a_real_one() {
    for day in ["2026-11-09", "2024-02-29", "2000-02-29", "0001-01-01"] {
        assert!(is_day(day), "{day}");
    }
    for day in [
        "2026-2-09",
        "2026-13-01",
        "2026-00-10",
        "2026-04-31",
        "2025-02-29",
        "1900-02-29",
        "2026-11-09T00:00",
        "2026/11/09",
        "２０２６-11-09",
        "",
    ] {
        assert!(!is_day(day), "{day}");
    }
}

#[test]
fn a_rows_label_is_its_first_text_propertys_value_or_untitled() {
    let property = |id, data_type| DatabaseProperty {
        id,
        database_id: 1,
        name: format!("P{id}"),
        data_type,
        sort_order: id,
        formula: None,
    };
    let properties = [
        property(1, DatabaseDataType::Integer),
        property(2, DatabaseDataType::Text),
        property(3, DatabaseDataType::Text),
    ];
    let row = |cells: Vec<(i64, DatabaseValue)>| DatabaseRowEntry {
        id: 1,
        database_id: 1,
        document_id: None,
        cells: cells
            .into_iter()
            .map(|(property_id, value)| DatabaseCell { property_id, value })
            .collect(),
    };
    let full = row(vec![
        (1, DatabaseValue::Integer(7)),
        (2, DatabaseValue::Text("Dune".into())),
        (3, DatabaseValue::Text("Herbert".into())),
    ]);
    assert_eq!(full.label(&properties), "Dune");
    // The first text property, even when empty, names it: a later one doesn't.
    let unnamed = row(vec![(3, DatabaseValue::Text("Herbert".into()))]);
    assert_eq!(unnamed.label(&properties), "Untitled");
    assert_eq!(full.label(&properties[..1]), "Untitled");
}
