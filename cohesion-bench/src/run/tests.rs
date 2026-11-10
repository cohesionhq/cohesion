use super::judge;
use serde_json::json;

#[test]
fn a_check_passes_only_on_a_clear_yes() {
    assert_eq!(judge(&json!(true)), (true, String::new()));
    assert_eq!(
        judge(&json!({ "pass": false, "reason": "no task" })),
        (false, "no task".to_string())
    );
    assert!(!judge(&json!({ "reason": "?" })).0);
    assert!(!judge(&json!("true")).0);
    assert!(!judge(&json!(null)).0);
}
