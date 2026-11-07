use cohesion_core::model::{ImageBytes, IntelligenceThreadImage, IntelligenceThreadMessage};

use super::*;

fn stored(item: ThreadItem, images: Vec<IntelligenceThreadImage>) -> ThreadEntry {
    ThreadEntry {
        message: IntelligenceThreadMessage {
            id: 1,
            thread_id: 1,
            created_at: 0,
            item,
        },
        images,
    }
}

/// Every item reads back from what the database stores of it, images too, or
/// a provider would be sent a different thread after a fork.
#[test]
fn an_item_reads_back_as_it_was_stored() {
    let items = [
        TranscriptItem::UserMessage(Prompt {
            text: "Hi".to_string(),
            images: vec![PromptImage {
                media_type: "image/png".to_string(),
                data: "AAAA".to_string(),
            }],
        }),
        TranscriptItem::AssistantMessage(Prompt {
            text: "Hello".to_string(),
            images: Vec::new(),
        }),
        TranscriptItem::Thinking(Thinking::Standard(StandardThinking {
            thought: "hm".to_string(),
            signature: Some("sig".to_string()),
            in_content: true,
        })),
        TranscriptItem::Thinking(Thinking::Redacted(RedactedThinking {
            trace: "opaque".to_string(),
            summary: None,
        })),
        TranscriptItem::ToolCallRequest(ToolCallRequest {
            call_identifier: "c1".to_string(),
            tool_name: "code".to_string(),
            arguments: "{}".to_string(),
        }),
        TranscriptItem::ToolCallResponse(ToolCallResponse {
            call_identifier: "c1".to_string(),
            response: ToolCallStatus::Failure("boom".to_string()),
        }),
    ];
    for item in items {
        let command = item.as_create_message_command(1);
        let images = command
            .images
            .into_iter()
            .map(
                |(media_type, data): (String, ImageBytes)| IntelligenceThreadImage {
                    id: 1,
                    message_id: 1,
                    media_type,
                    data,
                },
            )
            .collect();
        let back: TranscriptItem = stored(command.item, images).into();
        assert_eq!(format!("{back:?}"), format!("{item:?}"));
    }
}
