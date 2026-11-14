use super::{CalendarColorChoice, CalendarColorSupport, nearest_palette_color};
use crate::model::IntelligenceAgentColor;

#[test]
fn a_palette_color_is_its_own_nearest() {
    for &color in IntelligenceAgentColor::ALL {
        assert_eq!(nearest_palette_color(color.hex()), Some(color));
    }
}

#[test]
fn an_exact_color_binds_to_the_nearest_palette_color() {
    assert_eq!(
        nearest_palette_color("#D50000"),
        Some(IntelligenceAgentColor::Red)
    );
    assert_eq!(
        nearest_palette_color("#39f"),
        Some(IntelligenceAgentColor::Blue)
    );
    assert_eq!(
        nearest_palette_color("#2e7d32"),
        Some(IntelligenceAgentColor::Green)
    );
}

#[test]
fn what_isnt_a_color_is_refused() {
    for text in ["", "red", "#12345", "#gggggg", "123456", "#1234567"] {
        assert!(nearest_palette_color(text).is_none(), "{text}");
    }
    let exact = CalendarColorChoice::Exact {
        hex: "blue".to_string(),
    };
    assert!(exact.palette_color().is_none());
}

#[test]
fn a_backend_without_colors_declares_none() {
    let red = Some("#d50000");
    assert_eq!(
        CalendarColorSupport::Exact.declared(red),
        Some(IntelligenceAgentColor::Red)
    );
    assert_eq!(
        CalendarColorSupport::Declared.declared(red),
        Some(IntelligenceAgentColor::Red)
    );
    assert_eq!(CalendarColorSupport::None.declared(red), None);
    assert_eq!(CalendarColorSupport::Declared.declared(Some("red")), None);
}
