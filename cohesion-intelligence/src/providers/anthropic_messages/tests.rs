use super::MessagesResponse;
use crate::turn::TurnOutput;

#[test]
fn an_empty_text_block_is_not_a_message() {
    let response: MessagesResponse = serde_json::from_str(
        r#"{
            "content": [
                { "type": "text", "text": "" },
                { "type": "tool_use", "id": "call_1", "name": "run", "input": {} },
                { "type": "text", "text": "Done." }
            ],
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 3, "output_tokens": 5 }
        }"#,
    )
    .unwrap();
    let turn = response.into_turn().unwrap();
    let messages: Vec<&str> = turn
        .output
        .iter()
        .filter_map(|output| match output {
            TurnOutput::Message(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(messages, ["Done."]);
    assert_eq!(turn.output.len(), 2);
}
