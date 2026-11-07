use cohesion_core::model::{IntelligenceAgentMessage, IntelligenceAgentMessageSender};

use super::relevant_set;
use crate::agents::tests::{NOW, text};

fn conversation(count: i64, sent_at: impl Fn(i64) -> i64) -> Vec<IntelligenceAgentMessage> {
    (1..=count)
        .map(|id| text(id, sent_at(id), IntelligenceAgentMessageSender::User, "hi"))
        .collect()
}

#[test]
fn relevant_set_keeps_fresh_messages_oldest_first() {
    let mut candidates = conversation(5, |id| NOW - 60 + id);
    candidates.reverse();
    let ids: Vec<_> = relevant_set(candidates, NOW).iter().map(|m| m.id).collect();
    assert_eq!(ids, [1, 2, 3, 4, 5]);
}

#[test]
fn relevant_set_drops_old_messages() {
    // Ten messages from eight days ago, then the one just sent.
    let mut candidates = conversation(10, |_| NOW - 8 * 24 * 60 * 60);
    candidates.push(text(
        11,
        NOW,
        IntelligenceAgentMessageSender::User,
        "back again",
    ));
    candidates.reverse();
    let ids: Vec<_> = relevant_set(candidates, NOW).iter().map(|m| m.id).collect();
    assert_eq!(ids, [11]);
}

#[test]
fn relevant_set_keeps_recent_messages_across_a_day() {
    // Ten messages from yesterday, then the one just sent.
    let mut candidates = conversation(10, |_| NOW - 24 * 60 * 60);
    candidates.push(text(
        11,
        NOW,
        IntelligenceAgentMessageSender::User,
        "back again",
    ));
    candidates.reverse();
    let ids: Vec<_> = relevant_set(candidates, NOW).iter().map(|m| m.id).collect();
    assert_eq!(ids, (1..=11).collect::<Vec<_>>());
}

#[test]
fn relevant_set_keeps_recent_messages_across_a_weekend() {
    // Ten messages from three days ago, then the one just sent.
    let mut candidates = conversation(10, |_| NOW - 72 * 60 * 60);
    candidates.push(text(
        11,
        NOW,
        IntelligenceAgentMessageSender::User,
        "back again",
    ));
    candidates.reverse();
    let ids: Vec<_> = relevant_set(candidates, NOW).iter().map(|m| m.id).collect();
    assert_eq!(ids, (1..=11).collect::<Vec<_>>());
}

#[test]
fn relevant_set_counts_long_messages_by_their_excerpts_and_keeps_the_newest() {
    let long = "word ".repeat(12_000);
    let mut candidates: Vec<_> = (1..=10)
        .map(|id| text(id, NOW, IntelligenceAgentMessageSender::User, &long))
        .collect();
    candidates.reverse();
    let ids: Vec<_> = relevant_set(candidates, NOW).iter().map(|m| m.id).collect();
    // Whole they would overrun the budget; excerpted, all ten fit.
    assert_eq!(ids, (1..=10).collect::<Vec<_>>());

    let huge = "word ".repeat(100_000);
    let candidates = vec![text(1, NOW, IntelligenceAgentMessageSender::User, &huge)];
    assert_eq!(relevant_set(candidates, NOW).len(), 1);
}

#[test]
fn relevant_set_drops_deep_messages() {
    let mut candidates = conversation(200, |_| NOW);
    candidates.reverse();
    let relevant = relevant_set(candidates, NOW);
    assert!(!relevant.is_empty() && relevant.len() < 200);
    assert_eq!(relevant.last().unwrap().id, 200);
    assert!(relevant.windows(2).all(|pair| pair[0].id < pair[1].id));
}

// Scripts and notes leave far sooner than other messages, and don't take up the
// room of those that stay.
#[test]
fn old_scripts_and_notes_leave_before_other_messages() {
    use cohesion_core::model::AgentMessageContent;

    use crate::agents::tests::message;

    let hour = 60 * 60;
    let note = |id: i64, sent_at: i64| {
        message(
            id,
            sent_at,
            IntelligenceAgentMessageSender::Agent,
            AgentMessageContent::Note {
                note: "Stalled.".to_string(),
            },
        )
    };
    let candidates = vec![
        text(4, NOW, IntelligenceAgentMessageSender::User, "Now"),
        note(3, NOW - 10),
        note(2, NOW - 2 * hour),
        text(
            1,
            NOW - 2 * hour,
            IntelligenceAgentMessageSender::User,
            "Earlier",
        ),
    ];
    let kept: Vec<i64> = relevant_set(candidates, NOW)
        .iter()
        .map(|message| message.id)
        .collect();
    assert_eq!(kept, [1, 3, 4]);
}
