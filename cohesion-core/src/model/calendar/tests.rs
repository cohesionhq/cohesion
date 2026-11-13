use super::{CalendarColor, CalendarColorChoice};
use crate::model::IntelligenceAgentColor;

#[test]
fn a_palette_color_is_its_own_nearest() {
    for &color in IntelligenceAgentColor::ALL {
        let parsed = CalendarColor::parse(color.hex()).unwrap();
        assert_eq!(parsed.nearest, color);
        assert_eq!(parsed, CalendarColor::palette(color));
    }
}

#[test]
fn an_exact_color_keeps_its_value_and_finds_the_nearest() {
    let google_tomato = CalendarColor::parse("#D50000").unwrap();
    assert_eq!(google_tomato.hex, "#d50000");
    assert_eq!(google_tomato.nearest, IntelligenceAgentColor::Red);
    let short = CalendarColor::parse("#39f").unwrap();
    assert_eq!(short.hex, "#3399ff");
    assert_eq!(short.nearest, IntelligenceAgentColor::Blue);
    assert_eq!(
        CalendarColor::parse("#2e7d32").unwrap().nearest,
        IntelligenceAgentColor::Green
    );
}

#[test]
fn what_isnt_a_color_is_refused() {
    for text in ["", "red", "#12345", "#gggggg", "123456", "#1234567"] {
        assert!(CalendarColor::parse(text).is_none(), "{text}");
    }
    let exact = CalendarColorChoice::Exact {
        hex: "blue".to_string(),
    };
    assert!(CalendarColor::chosen(&exact).is_none());
}
