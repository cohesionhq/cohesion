use super::*;
use crate::metrics::{Activity, Outcome};

#[test]
fn a_csv_field_is_quoted_only_when_it_must_be() {
    assert_eq!(csv_field("plain"), "plain");
    assert_eq!(csv_field("a, b"), "\"a, b\"");
    assert_eq!(csv_field("say \"hi\"\nthen"), "\"say \"\"hi\"\"\nthen\"");
}

#[test]
fn the_report_escapes_what_the_agent_said() {
    let result = CaseResult {
        model: "m".to_string(),
        case: "c".to_string(),
        title: "T".to_string(),
        category: "tasks".to_string(),
        repeat: 1,
        outcome: Outcome::Passed,
        reason: String::new(),
        seconds: 1.0,
        priced: false,
        activity: Activity {
            reply: "<script>alert(1)</script>".to_string(),
            ..Activity::default()
        },
    };
    let case = Case {
        name: "c".to_string(),
        title: "T".to_string(),
        category: "tasks".to_string(),
        prompt: "Do it".to_string(),
        setup: None,
        check: "true".to_string(),
        solution: None,
        timeout: std::time::Duration::from_secs(1),
    };
    let models = ["m".to_string()];
    let cases = [case];
    let info = RunInfo {
        started: "now".to_string(),
        models: &models,
        cases: &cases,
        repeats: 1,
    };
    let summaries = crate::metrics::summaries(&models, std::slice::from_ref(&result));
    let page = html(&info, &[result], &summaries);
    assert!(page.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(!page.contains("<script>"));
    assert!(page.contains("unpriced"));
}
