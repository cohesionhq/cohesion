use cohesion_core::model::IntelligenceAgentColor;

use crate::test_support::context;
use crate::{
    CommandError, CreateAgentTeamCommand, CreateIntelligenceAgentCommand, DeleteAgentTeamCommand,
    GetIntelligenceAgentCommand, ListAgentTeamsCommand, RenameAgentTeamCommand,
    SetAgentPinnedCommand, SetAgentTeamCommand,
};

fn create_agent(name: &str, team_id: Option<i64>) -> CreateIntelligenceAgentCommand {
    CreateIntelligenceAgentCommand {
        name: Some(name.to_string()),
        color: IntelligenceAgentColor::Teal,
        emoji: None,
        personality: None,
        verbosity: None,
        role: None,
        directive: None,
        team_id,
    }
}

fn create_team(title: &str) -> CreateAgentTeamCommand {
    CreateAgentTeamCommand {
        title: title.to_string(),
    }
}

#[test]
fn a_team_title_is_unique_ignoring_case_and_teams_list_by_title() {
    let db = context();
    let research = db.send_blocking(create_team(" Research ")).unwrap();
    assert_eq!(research.title, "Research");
    let engineering = db.send_blocking(create_team("Engineering")).unwrap();
    assert!(matches!(
        db.send_blocking(create_team("research")),
        Err(CommandError::Validation(_))
    ));
    assert!(matches!(
        db.send_blocking(RenameAgentTeamCommand {
            team_id: engineering.id,
            title: "RESEARCH".to_string(),
        }),
        Err(CommandError::Validation(_))
    ));
    let titles: Vec<String> = db
        .send_blocking(ListAgentTeamsCommand {})
        .unwrap()
        .into_iter()
        .map(|team| team.title)
        .collect();
    assert_eq!(titles, ["Engineering", "Research"]);
}

#[test]
fn moving_an_agent_between_teams_keeps_its_pin() {
    let db = context();
    let team = db.send_blocking(create_team("Engineering")).unwrap();
    let lead = db.send_blocking(create_agent("Lead", None)).unwrap();
    db.send_blocking(SetAgentPinnedCommand {
        agent_id: lead.id,
        pinned: true,
    })
    .unwrap();
    let moved = db
        .send_blocking(SetAgentTeamCommand {
            agent_id: lead.id,
            team_id: Some(team.id),
        })
        .unwrap();
    assert_eq!(moved.team_id, Some(team.id));
    assert!(moved.pinned);
    let unteamed = db
        .send_blocking(SetAgentTeamCommand {
            agent_id: lead.id,
            team_id: None,
        })
        .unwrap();
    assert_eq!(unteamed.team_id, None);
    assert!(unteamed.pinned);
}

#[test]
fn an_agent_cannot_join_a_team_that_does_not_exist() {
    let db = context();
    let agent = db.send_blocking(create_agent("Scout", None)).unwrap();
    assert!(
        db.send_blocking(SetAgentTeamCommand {
            agent_id: agent.id,
            team_id: Some(404),
        })
        .is_err()
    );
}

#[test]
fn deleting_a_team_leaves_its_agents_on_no_team() {
    let db = context();
    let team = db.send_blocking(create_team("Engineering")).unwrap();
    let agent = db
        .send_blocking(create_agent("Core", Some(team.id)))
        .unwrap();
    assert_eq!(agent.team_id, Some(team.id));
    db.send_blocking(DeleteAgentTeamCommand { team_id: team.id })
        .unwrap();
    let agent = db
        .send_blocking(GetIntelligenceAgentCommand { agent_id: agent.id })
        .unwrap();
    assert_eq!(agent.team_id, None);
    assert!(
        db.send_blocking(ListAgentTeamsCommand {})
            .unwrap()
            .is_empty()
    );
}
