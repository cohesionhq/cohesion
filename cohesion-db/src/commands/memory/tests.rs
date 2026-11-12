use cohesion_core::model::{IntelligenceAgentColor, Memory, MemoryFilter, MemoryScope};

use crate::test_support::context;
use crate::{
    CommandError, CreateAgentTeamCommand, CreateIntelligenceAgentCommand, CreateMemoryCommand,
    DatabaseContext, DeleteAgentTeamCommand, DeleteIntelligenceAgentCommand, DeleteMemoryCommand,
    GetStorageSettingsCommand, ListMemoriesCommand, SetAgentTeamCommand,
    UpdateStorageSettingsCommand,
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

fn remember(db: &DatabaseContext, scope: MemoryScope, memory: &str) -> Memory {
    db.send_blocking(CreateMemoryCommand {
        scope,
        memory: memory.to_string(),
    })
    .unwrap()
}

fn personal(agent_id: i64) -> MemoryScope {
    MemoryScope::Agent { agent_id }
}

fn of_team(team_id: i64) -> MemoryScope {
    MemoryScope::Team { team_id }
}

fn list(db: &DatabaseContext, filter: MemoryFilter) -> Vec<String> {
    db.send_blocking(ListMemoriesCommand { filter })
        .unwrap()
        .into_iter()
        .map(|memory| memory.memory)
        .collect()
}

#[test]
fn an_agent_sees_its_own_then_its_teams_then_the_global_memories() {
    let db = context();
    let engineering = team(&db, "Engineering");
    let research = team(&db, "Research");
    let ada = agent(&db, "Ada", Some(engineering));
    let bo = agent(&db, "Bo", Some(research));
    let cy = agent(&db, "Cy", None);
    remember(&db, MemoryScope::Global, "Everyone's");
    remember(&db, of_team(engineering), "Engineering's");
    remember(&db, personal(ada), "Ada's");
    remember(&db, of_team(research), "Research's");
    remember(&db, personal(bo), "Bo's");

    assert_eq!(
        list(&db, MemoryFilter::SeenBy(ada)),
        ["Ada's", "Engineering's", "Everyone's"]
    );
    assert_eq!(
        list(&db, MemoryFilter::SeenBy(bo)),
        ["Bo's", "Research's", "Everyone's"]
    );
    assert_eq!(list(&db, MemoryFilter::SeenBy(cy)), ["Everyone's"]);
    assert_eq!(list(&db, MemoryFilter::Global), ["Everyone's"]);
    assert_eq!(
        list(&db, MemoryFilter::Every),
        ["Ada's", "Bo's", "Engineering's", "Research's", "Everyone's"]
    );
}

#[test]
fn moving_to_another_team_changes_which_team_memories_an_agent_sees() {
    let db = context();
    let engineering = team(&db, "Engineering");
    let research = team(&db, "Research");
    let ada = agent(&db, "Ada", Some(engineering));
    remember(&db, of_team(engineering), "Engineering's");
    remember(&db, of_team(research), "Research's");

    db.send_blocking(SetAgentTeamCommand {
        agent_id: ada,
        team_id: Some(research),
    })
    .unwrap();
    assert_eq!(list(&db, MemoryFilter::SeenBy(ada)), ["Research's"]);
    db.send_blocking(SetAgentTeamCommand {
        agent_id: ada,
        team_id: None,
    })
    .unwrap();
    assert!(list(&db, MemoryFilter::SeenBy(ada)).is_empty());
}

/// Sets the storage settings' memory limit to `limit`, keeping the rest.
fn limit_memories(db: &DatabaseContext, limit: i64) {
    let settings = db.send_blocking(GetStorageSettingsCommand {}).unwrap();
    db.send_blocking(UpdateStorageSettingsCommand {
        thread_retention_days: settings.thread_retention_days,
        actor_session_retention_days: settings.actor_session_retention_days,
        rewind_retention_days: 7,
        deleted_item_retention_days: 30,
        memory_limit: limit,
    })
    .unwrap();
}

#[test]
fn each_scope_keeps_as_many_memories_as_the_storage_settings_allow() {
    let db = context();
    let engineering = team(&db, "Engineering");
    let ada = agent(&db, "Ada", Some(engineering));
    let bo = agent(&db, "Bo", None);
    assert_eq!(
        db.send_blocking(GetStorageSettingsCommand {})
            .unwrap()
            .memory_limit,
        20
    );
    limit_memories(&db, 3);
    let fill = |scope: MemoryScope| {
        (0..3)
            .map(|n| remember(&db, scope, &format!("Memory {n}")))
            .collect::<Vec<_>>()
    };
    let one_more = |scope| {
        db.send_blocking(CreateMemoryCommand {
            scope,
            memory: "One more".to_string(),
        })
    };

    let ada_memories = fill(personal(ada));
    fill(of_team(engineering));
    fill(MemoryScope::Global);
    for full in [personal(ada), of_team(engineering), MemoryScope::Global] {
        assert!(matches!(one_more(full), Err(CommandError::Validation(_))));
    }
    // Each limit is its own scope's.
    assert!(one_more(personal(bo)).is_ok());
    db.send_blocking(DeleteMemoryCommand {
        memory_id: ada_memories[0].id,
    })
    .unwrap();
    assert!(one_more(personal(ada)).is_ok());
    // Lowering the limit forgets nothing, and refuses another until the
    // scope is under it again.
    limit_memories(&db, 1);
    assert_eq!(list(&db, MemoryFilter::SeenBy(bo)).len(), 4);
    assert!(matches!(
        one_more(personal(bo)),
        Err(CommandError::Validation(_))
    ));
    let none_below_one = db.send_blocking(UpdateStorageSettingsCommand {
        thread_retention_days: 14,
        actor_session_retention_days: 7,
        rewind_retention_days: 7,
        deleted_item_retention_days: 30,
        memory_limit: 0,
    });
    assert!(matches!(none_below_one, Err(CommandError::Validation(_))));
}

#[test]
fn a_memory_is_trimmed_and_needs_some_text() {
    let db = context();
    assert_eq!(remember(&db, MemoryScope::Global, "  Jo  ").memory, "Jo");
    let blank = db.send_blocking(CreateMemoryCommand {
        scope: MemoryScope::Global,
        memory: " \n ".to_string(),
    });
    assert!(matches!(blank, Err(CommandError::Validation(_))));
}

#[test]
fn forgetting_returns_the_memory_once() {
    let db = context();
    let memory = remember(&db, MemoryScope::Global, "Call me Jo");
    let forget = || {
        db.send_blocking(DeleteMemoryCommand {
            memory_id: memory.id,
        })
    };
    assert_eq!(forget().unwrap().memory, "Call me Jo");
    assert!(matches!(forget(), Err(CommandError::NotFound(_))));
    assert!(list(&db, MemoryFilter::Global).is_empty());
}

#[test]
fn an_agents_memories_go_with_it_and_a_teams_with_the_team() {
    let db = context();
    let engineering = team(&db, "Engineering");
    let ada = agent(&db, "Ada", Some(engineering));
    let bo = agent(&db, "Bo", Some(engineering));
    remember(&db, personal(ada), "Ada files the groceries");
    remember(&db, of_team(engineering), "Releases go out on Fridays");
    remember(
        &db,
        MemoryScope::Global,
        "Groceries go in the Home workspace",
    );

    db.send_blocking(DeleteIntelligenceAgentCommand { agent_id: ada })
        .unwrap();
    assert_eq!(
        list(&db, MemoryFilter::SeenBy(bo)),
        [
            "Releases go out on Fridays",
            "Groceries go in the Home workspace"
        ]
    );
    db.send_blocking(DeleteAgentTeamCommand {
        team_id: engineering,
    })
    .unwrap();
    assert_eq!(
        list(&db, MemoryFilter::SeenBy(bo)),
        ["Groceries go in the Home workspace"]
    );
}
