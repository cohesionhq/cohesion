use cohesion_core::model::{IntelligenceAgentMessage, IntelligenceAgentMessageSender};

use super::is_news;
use crate::agents::tests::{NOW, text};

// The conversation hears what its forks and runs posted, not what it said
// or heard itself.
#[test]
fn a_live_thread_hears_what_forks_and_runs_posted() {
    let user = text(1, NOW, IntelligenceAgentMessageSender::User, "Hi");
    let own = text(2, NOW, IntelligenceAgentMessageSender::Agent, "Hello");
    let run = text(
        3,
        NOW,
        IntelligenceAgentMessageSender::BackgroundAgent,
        "Found it",
    );
    let fork = IntelligenceAgentMessage {
        run_id: Some(9),
        ..text(4, NOW, IntelligenceAgentMessageSender::Agent, "App is done")
    };
    assert!(!is_news(&user));
    assert!(!is_news(&own));
    assert!(is_news(&run));
    assert!(is_news(&fork));
}
