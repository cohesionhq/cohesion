use super::*;
use crate::metrics::Activity;

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
        difficulty: Difficulty::Easy,
        repeat: 1,
        outcome: Outcome::Failed,
        reason: "missed: <b>bold</b>".to_string(),
        seconds: 1.0,
        priced: false,
        activity: Activity {
            reply: "<script>alert(1)</script>".to_string(),
            ..Activity::default()
        },
    };
    let directory = std::env::temp_dir()
        .join(format!("cohesion-bench-report-{}", std::process::id()))
        .join("c");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("prompt.md"),
        "---\ntitle: T\ndifficulty: easy\nstate: none\n---\nDo it\n",
    )
    .unwrap();
    std::fs::write(
        directory.join("validate.ts"),
        "export default function validate() {}\n",
    )
    .unwrap();
    let case = Case::load(&directory, "tasks".to_string(), "", &Default::default()).unwrap();
    _ = std::fs::remove_dir_all(&directory);
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
    assert!(page.contains("missed: &lt;b&gt;bold&lt;/b&gt;"));
    assert!(page.contains("unpriced"));
}
