use super::{PropertyDataType, PropertyValue, is_day};

#[test]
fn each_type_gives_its_text_and_the_numbers_it_honestly_is() {
    let cases = [
        (PropertyValue::Text("Ada".into()), "Ada", None, None),
        (PropertyValue::Integer(42), "42", Some(42), Some(42.0)),
        (PropertyValue::Decimal(3.5), "3.5", None, Some(3.5)),
        (PropertyValue::Currency(12.99), "12.99", None, Some(12.99)),
        (PropertyValue::Checkbox(true), "true", Some(1), None),
        (PropertyValue::Checkbox(false), "false", Some(0), None),
        (
            PropertyValue::Date("2026-11-09".into()),
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
            PropertyValue::parse(value.data_type(), &value.text()),
            Some(value)
        );
    }
}

#[test]
fn text_parses_as_another_type_only_when_it_is_one() {
    use PropertyDataType::*;
    assert_eq!(
        PropertyValue::parse(Decimal, "42"),
        Some(PropertyValue::Decimal(42.0))
    );
    assert_eq!(
        PropertyValue::parse(Integer, "3"),
        Some(PropertyValue::Integer(3))
    );
    assert_eq!(PropertyValue::parse(Integer, "3.5"), None);
    assert_eq!(PropertyValue::parse(Integer, "true"), None);
    assert_eq!(PropertyValue::parse(Checkbox, "1"), None);
    assert_eq!(PropertyValue::parse(Decimal, "inf"), None);
    assert_eq!(PropertyValue::parse(Decimal, "NaN"), None);
    assert_eq!(PropertyValue::parse(Date, "Monday"), None);
    assert_eq!(
        PropertyValue::parse(Text, "true"),
        Some(PropertyValue::Text("true".into()))
    );
}

#[test]
fn empty_text_is_no_value_and_bad_numbers_and_days_are_refused() {
    assert_eq!(PropertyValue::Text(String::new()).checked(), Ok(None));
    assert!(PropertyValue::Decimal(f64::NAN).checked().is_err());
    assert!(PropertyValue::Currency(f64::INFINITY).checked().is_err());
    assert!(PropertyValue::Date("2026-02-30".into()).checked().is_err());
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
