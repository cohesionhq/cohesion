use cohesion_core::model::{AgentCodeExecution, AgentCodeOutcome, AgentMessageContent};

use super::AgentCodeCall;
use crate::agents::agent_transcript::message_text;

// A script's long output reaches the model cut to its first 32,000
// characters, and `read_message` reads the rest from the run's recorded
// message, from where the cut stops.
#[test]
fn a_long_script_output_is_capped_and_read_whole() {
    let output = format!("{}{}", "a".repeat(32_000), "b".repeat(500));
    let capped = AgentCodeCall::capped(output.clone(), Some(9));
    assert!(capped.starts_with(&format!("{}…", "a".repeat(32_000))));
    assert!(capped.contains("[500 more characters; `read_message` 9 from 32000"));
    assert!(!capped.contains('b'));
    assert_eq!(AgentCodeCall::capped(output.clone(), None), output);
    let content = AgentMessageContent::CodeExecution(AgentCodeExecution {
        title: "Listing everything".to_string(),
        code: "everything()".to_string(),
        outcome: AgentCodeOutcome::Succeeded { result: output },
    });
    let rest: String = message_text(&content)
        .unwrap()
        .chars()
        .skip(32_000)
        .collect();
    assert_eq!(rest, "b".repeat(500));
}
