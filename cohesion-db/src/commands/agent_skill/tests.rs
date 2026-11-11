use cohesion_core::model::{
    AgentMessageContent, AgentSkill, AgentSkillScope, IntelligenceAgentColor,
};
use cohesion_core::protocol::AgentForwardTarget;

use crate::test_support::context;
use crate::{
    CommandError, CreateAgentSkillCommand, CreateAgentTeamCommand, CreateIntelligenceAgentCommand,
    CreateUserAgentMessageCommand, DatabaseContext, DeleteAgentSkillCommand,
    DeleteAgentTeamCommand, DeleteIntelligenceAgentCommand, ListAgentSkillsCommand,
    SkillsForAgentCommand, UpdateAgentSkillCommand,
};

fn agent(db: &DatabaseContext, name: &str, team_id: Option<i64>) -> i64 {
    db.send_blocking(CreateIntelligenceAgentCommand {
        name: Some(name.to_string()),
        color: IntelligenceAgentColor::Blue,
        emoji: None,
        personality: None,
        verbosity: None,
        role: None,
        directive: None,
        team_id,
    })
    .unwrap()
    .id
}

fn team(db: &DatabaseContext, title: &str) -> i64 {
    db.send_blocking(CreateAgentTeamCommand {
        title: title.to_string(),
    })
    .unwrap()
    .id
}

fn save(
    db: &DatabaseContext,
    scope: AgentSkillScope,
    name: &str,
) -> Result<AgentSkill, CommandError> {
    db.send_blocking(CreateAgentSkillCommand {
        scope,
        name: name.to_string(),
        description: String::new(),
        prompt: format!("Do {name}."),
        agent_can_use: true,
    })
}

fn own(agent_id: i64) -> AgentSkillScope {
    AgentSkillScope::Agent { agent_id }
}

fn of_team(team_id: i64) -> AgentSkillScope {
    AgentSkillScope::Team { team_id }
}

/// The names of the skills agent `agent_id` has, with whose each is.
fn skills(db: &DatabaseContext, agent_id: i64) -> Vec<(String, AgentSkillScope)> {
    db.send_blocking(SkillsForAgentCommand { agent_id })
        .unwrap()
        .into_iter()
        .map(|skill| (skill.name, skill.scope))
        .collect()
}

#[test]
fn an_agent_has_its_own_skills_and_its_teams_and_its_own_wins_a_name() {
    let db = context();
    let crew = team(&db, "Crew");
    let ada = agent(&db, "Ada", Some(crew));
    let bo = agent(&db, "Bo", Some(crew));
    let loner = agent(&db, "Cy", None);
    save(&db, of_team(crew), "Standup").unwrap();
    save(&db, of_team(crew), "Weekly review").unwrap();
    save(&db, own(ada), "weekly REVIEW").unwrap();
    save(&db, own(bo), "Triage").unwrap();

    assert_eq!(
        skills(&db, ada),
        [
            ("Standup".to_string(), of_team(crew)),
            ("weekly REVIEW".to_string(), own(ada)),
        ]
    );
    assert_eq!(
        skills(&db, bo),
        [
            ("Standup".to_string(), of_team(crew)),
            ("Triage".to_string(), own(bo)),
            ("Weekly review".to_string(), of_team(crew)),
        ]
    );
    assert_eq!(skills(&db, loner), []);
    assert_eq!(
        db.send_blocking(ListAgentSkillsCommand {}).unwrap().len(),
        4
    );
}

#[test]
fn a_name_is_taken_once_in_a_scope_ignoring_case() {
    let db = context();
    let crew = team(&db, "Crew");
    let ada = agent(&db, "Ada", None);
    let bo = agent(&db, "Bo", None);
    save(&db, own(ada), "Standup").unwrap();
    let taken = save(&db, own(ada), " standup ").unwrap_err();
    let CommandError::Validation(error) = taken else {
        panic!("not a refusal: {taken:?}");
    };
    assert_eq!(
        error.message,
        "There's already a skill named “standup” here."
    );
    save(&db, own(bo), "Standup").unwrap();
    save(&db, of_team(crew), "Standup").unwrap();

    let triage = save(&db, own(ada), "Triage").unwrap();
    let renamed = db.send_blocking(UpdateAgentSkillCommand {
        skill_id: triage.id,
        name: "STANDUP".to_string(),
        description: String::new(),
        prompt: "Triage.".to_string(),
        agent_can_use: false,
    });
    assert!(matches!(renamed, Err(CommandError::Validation(_))));
}

#[test]
fn a_skill_needs_a_name_and_a_prompt() {
    let db = context();
    let ada = agent(&db, "Ada", None);
    assert!(matches!(
        save(&db, own(ada), "  "),
        Err(CommandError::Validation(_))
    ));
    let empty = db.send_blocking(CreateAgentSkillCommand {
        scope: own(ada),
        name: "Standup".to_string(),
        description: String::new(),
        prompt: "\n ".to_string(),
        agent_can_use: true,
    });
    assert!(matches!(empty, Err(CommandError::Validation(_))));
}

#[test]
fn a_change_keeps_the_scope_and_trims_the_text() {
    let db = context();
    let ada = agent(&db, "Ada", None);
    let skill = save(&db, own(ada), "Standup").unwrap();
    let changed = db
        .send_blocking(UpdateAgentSkillCommand {
            skill_id: skill.id,
            name: " Daily standup ".to_string(),
            description: " Mornings. ".to_string(),
            prompt: " Say what changed. ".to_string(),
            agent_can_use: false,
        })
        .unwrap();
    assert_eq!(changed.scope, own(ada));
    assert_eq!(changed.name, "Daily standup");
    assert_eq!(changed.description, "Mornings.");
    assert_eq!(changed.prompt, "Say what changed.");
    assert!(!changed.agent_can_use);
}

#[test]
fn skills_go_with_their_agent_or_team() {
    let db = context();
    let crew = team(&db, "Crew");
    let ada = agent(&db, "Ada", None);
    save(&db, own(ada), "Standup").unwrap();
    save(&db, of_team(crew), "Standup").unwrap();
    db.send_blocking(DeleteIntelligenceAgentCommand { agent_id: ada })
        .unwrap();
    db.send_blocking(DeleteAgentTeamCommand { team_id: crew })
        .unwrap();
    assert!(
        db.send_blocking(ListAgentSkillsCommand {})
            .unwrap()
            .is_empty()
    );
}

#[test]
fn the_users_message_keeps_the_skill_as_it_stood() {
    let db = context();
    let ada = agent(&db, "Ada", None);
    let bo = agent(&db, "Bo", None);
    let skill = save(&db, own(ada), "Standup").unwrap();
    let send = |agent_id, skill, forward| {
        db.send_blocking(CreateUserAgentMessageCommand {
            agent_id,
            text: " Keep it short ".to_string(),
            attachments: Vec::new(),
            forward,
            skill,
        })
    };

    let (message, _) = send(ada, Some(skill.id), None).unwrap();
    db.send_blocking(DeleteAgentSkillCommand { skill_id: skill.id })
        .unwrap();
    let AgentMessageContent::Skill { name, prompt, note } = message.message_content else {
        panic!("not a skill: {:?}", message.message_content);
    };
    assert_eq!(
        (name.as_str(), prompt.as_str(), note.as_str()),
        ("Standup", "Do Standup.", "Keep it short")
    );

    let theirs = save(&db, own(bo), "Triage").unwrap();
    assert!(matches!(
        send(ada, Some(theirs.id), None),
        Err(CommandError::Validation(_))
    ));
    let both = send(
        bo,
        Some(theirs.id),
        Some(AgentForwardTarget::Task {
            workspace_id: 1,
            id: 1,
        }),
    );
    assert!(matches!(both, Err(CommandError::Validation(_))));
}
