use super::{range, slice};

#[test]
fn a_range_defaults_to_the_whole_text_and_clamps_to_its_end() {
    assert_eq!(range(None, None, 10), Ok(0..10));
    assert_eq!(range(Some(4), None, 10), Ok(4..10));
    assert_eq!(range(Some(2), Some(500), 10), Ok(2..10));
    assert_eq!(range(Some(3), Some(3), 10), Ok(3..3));
}

#[test]
fn a_backwards_or_negative_range_is_refused() {
    assert!(range(Some(5), Some(2), 10).is_err());
    assert!(range(Some(-1), None, 10).is_err());
    assert!(range(None, Some(-1), 10).is_err());
    assert!(range(Some(11), None, 10).is_err());
}

#[test]
fn a_range_counts_characters_not_bytes() {
    assert_eq!(slice("héllo wörld", 1..8), "éllo wö");
    assert_eq!(slice("héllo", 3..99), "lo");
}
