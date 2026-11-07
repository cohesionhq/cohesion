use cohesion_core::model::{HookKind, IntelligenceAgentColor, IntelligenceEffort};

use crate::test_support::context;
use crate::{
    CreateIntelligenceAgentCommand, DeleteAgentCueCommand, ListAgentCuesCommand,
    ListAgentCuesForHookCommand, SetAgentCueCommand,
};

#[test]
fn a_cue_replaces_only_the_one_with_its_name() {
    use HookKind::{TaskCreate, TaskStatus};
    let db = context();
    let agent = db
        .send_blocking(CreateIntelligenceAgentCommand {
            name: Some("Scout".to_string()),
            color: IntelligenceAgentColor::Blue,
            emoji: None,
            personality: None,
            verbosity: None,
            role: None,
            directive: None,
            team_id: None,
        })
        .unwrap();
    let set = |name: &str, hook, script: &str| {
        db.send_blocking(SetAgentCueCommand {
            agent_id: agent.id,
            name: name.to_string(),
            hook,
            script: script.to_string(),
            instructions: "".to_string(),
            effort: IntelligenceEffort::Low,
        })
        .unwrap()
    };
    let triage = set("Triage", TaskCreate, "first");
    let follow_up = set("Follow up", TaskStatus, "second");
    let replaced = set("Triage", TaskStatus, "third");
    assert_eq!(replaced.id, triage.id);

    let cues = |agent_id| {
        db.send_blocking(ListAgentCuesCommand { agent_id })
            .unwrap()
            .into_iter()
            .map(|cue| (cue.name, cue.script))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        cues(agent.id),
        [
            ("Triage".to_string(), "third".to_string()),
            ("Follow up".to_string(), "second".to_string())
        ]
    );
    assert!(
        db.send_blocking(ListAgentCuesForHookCommand { hook: TaskCreate })
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        db.send_blocking(ListAgentCuesForHookCommand { hook: TaskStatus })
            .unwrap()
            .len(),
        2
    );

    db.send_blocking(DeleteAgentCueCommand {
        cue_id: follow_up.id,
    })
    .unwrap();
    assert_eq!(
        cues(agent.id),
        [("Triage".to_string(), "third".to_string())]
    );
}
