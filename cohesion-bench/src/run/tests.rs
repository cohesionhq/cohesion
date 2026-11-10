use super::judge;
use serde_json::json;

#[test]
fn nothing_or_false_passes_and_a_string_fails() {
    assert_eq!(judge(&json!(null)), Ok(()));
    assert_eq!(judge(&json!(false)), Ok(()));
    assert_eq!(
        judge(&json!("no task about stamps")),
        Err("no task about stamps".to_string())
    );
    assert!(judge(&json!("  ")).is_err());
    // A check that answers otherwise doesn't follow the rule.
    for other in [json!(true), json!(1), json!({ "pass": true })] {
        assert!(judge(&other).is_err(), "{other}");
    }
}
