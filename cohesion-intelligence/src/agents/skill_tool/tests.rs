use std::sync::Arc;

use cohesion_core::model::{AgentMessageContent, AgentSkillScope, IntelligenceAgentColor};
use cohesion_db::{
    Container, CreateAgentSkillCommand, CreateIntelligenceAgentCommand, DatabaseContext,
    ListAgentMessagesCommand, Secrets,
};

use super::*;
use crate::agents::conversation_session::Branch;

fn agent(db: &DatabaseContext) -> i64 {
    db.send_blocking(CreateIntelligenceAgentCommand {
        name: Some("Scout".to_string()),
        color: IntelligenceAgentColor::Blue,
        emoji: None,
        personality: None,
        verbosity: None,
        role: None,
        directive: None,
        team_id: None,
    })
    .unwrap()
    .id
}

async fn save(db: &DatabaseContext, agent_id: i64, name: &str, agent_can_use: bool) {
    db.send(CreateAgentSkillCommand {
        scope: AgentSkillScope::Agent { agent_id },
        name: name.to_string(),
        description: String::new(),
        prompt: format!("Do {name}."),
        agent_can_use,
    })
    .await
    .unwrap();
}

// The agent runs a skill it may use by name, ignoring case, and the feed
// shows it did; a skill the user keeps to themselves is as unknown to it as
// one that doesn't exist, and the refusal names the ones it has.
#[test]
fn the_agent_runs_only_the_skills_it_may_use() {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    let db = DatabaseContext::new(container, Secrets::new(), ());
    let agent_id = agent(&db);
    let tool = UseSkillTool {
        database: db.clone(),
        agent_id,
    };
    let seat = Seat {
        turn: Arc::default(),
        branch: Branch::Conversation {
            sent: Arc::default(),
            reacted: Arc::default(),
        },
    };
    let call = |name: &str| serde_json::json!({ "name": name }).to_string();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let ToolCallStatus::Failure(none) = tool.call(&seat, &call("Standup")).await else {
            panic!("ran a skill that doesn't exist");
        };
        assert_eq!(none, "You have no skills to use.");

        save(&db, agent_id, "Standup", true).await;
        save(&db, agent_id, "Private", false).await;
        let ToolCallStatus::Success(prompt) = tool.call(&seat, &call(" standup ")).await else {
            panic!("the skill didn't run");
        };
        assert_eq!(
            prompt,
            "Skill \"Standup\":\nDo Standup.\n\nCarry it out now."
        );
        let ToolCallStatus::Failure(hidden) = tool.call(&seat, &call("Private")).await else {
            panic!("ran a skill the agent may not use");
        };
        assert_eq!(
            hidden,
            "You have no skill named \"Private\". Your skills: \"Standup\"."
        );

        let feed = db
            .send(ListAgentMessagesCommand {
                agent_id,
                before: None,
                limit: 10,
                ids: None,
            })
            .await
            .unwrap();
        let [used] = feed.as_slice() else {
            panic!("expected one line: {feed:?}");
        };
        assert!(matches!(
            &used.message_content,
            AgentMessageContent::UsedSkill { name } if name == "Standup"
        ));
        assert_eq!(used.excerpt, "Using skill “Standup”");
    });
}
