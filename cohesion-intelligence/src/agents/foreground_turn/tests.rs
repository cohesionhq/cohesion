use cohesion_core::model::IntelligenceAgentMessageSender;

use super::*;
use crate::agents::tests::{NOW, text};
use crate::turn_input::TurnInput;

fn steered(id: i64, words: &str) -> Steered {
    Steered {
        message: text(id, NOW, IntelligenceAgentMessageSender::User, words),
        attachments: Vec::new(),
    }
}

#[test]
fn a_turn_takes_messages_in_only_while_it_runs_and_until_it_stops() {
    let turn = ForegroundTurn::default();
    assert!(turn.steer(steered(1, "Before")).is_some());

    let began = turn.begin(DatabaseEventTrace::default());
    assert!(turn.steer(steered(2, "During")).is_none());
    let [prompt] = turn.take().try_into().unwrap();
    assert!(prompt.text.ends_with("[#2] During"), "{}", prompt.text);
    let joined: Vec<i64> = turn.take_joined().iter().map(|m| m.id).collect();
    assert_eq!(joined, [2]);

    // Sent before the stop and never taken in, a message waits for a turn
    // of its own; one sent after isn't taken.
    assert!(turn.steer(steered(3, "Then")).is_none());
    turn.stop();
    assert!(turn.is_stopped());
    assert!(turn.take().is_empty());
    assert!(turn.steer(steered(4, "After")).is_some());
    let left: Vec<i64> = turn.end(began).iter().map(|s| s.message.id).collect();
    assert_eq!(left, [3]);

    // A new turn starts unstopped; once it closes, nothing joins it.
    turn.begin(DatabaseEventTrace::default());
    assert!(!turn.is_stopped());
    assert!(turn.close().is_empty());
    assert!(turn.steer(steered(5, "Late")).is_some());
}

#[test]
fn a_turn_that_ends_late_leaves_the_next_one_open_with_its_messages() {
    let turn = ForegroundTurn::default();
    let first = turn.begin(DatabaseEventTrace::default().deeper().hop());
    assert_eq!((turn.trace().depth(), turn.trace().hops()), (1, 1));
    assert!(turn.steer(steered(1, "For the first")).is_none());
    // The user's message starts the trace over, however deep the turn was.
    assert_eq!((turn.trace().depth(), turn.trace().hops()), (0, 0));

    // The next turn begins before the first ends, as it can once the
    // host's lock is released: the late end changes nothing, and the
    // message the first never took in is the second's to take.
    let second = turn.begin(DatabaseEventTrace::default());
    assert!(turn.end(first).is_empty());
    assert!(turn.steer(steered(2, "For the second")).is_none());
    let prompts = turn.take();
    assert_eq!(prompts.len(), 2);
    assert!(turn.end(second).is_empty());
    assert!(turn.steer(steered(3, "After")).is_some());
}
