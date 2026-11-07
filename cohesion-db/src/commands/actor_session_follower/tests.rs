use cohesion_core::model::{ActorSessionMove, Initiative, IntelligenceAgentColor};

use super::*;
use crate::test_support::{actor_session, context};
use crate::{
    CreateIntelligenceAgentCommand, DatabaseContext, DeleteActorSessionCommand,
    DeleteIntelligenceAgentCommand, GetActorSessionCommand, GetIntelligenceAgentCommand,
    SetActorSessionStatusCommand,
};

fn agent(db: &DatabaseContext, name: &str) -> i64 {
    db.send_blocking(CreateIntelligenceAgentCommand {
        name: Some(name.to_string()),
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

#[test]
fn the_last_agent_to_tell_a_session_something_follows_it_through_every_move() {
    let db = context();
    let session_id = actor_session(&db);
    let (scout, pilot) = (agent(&db, "Scout"), agent(&db, "Pilot"));
    let follower = || {
        db.send_blocking(GetActorSessionCommand { id: session_id })
            .unwrap()
            .follower
            .map(|follower| follower.agent_id)
    };
    let follow = |agent_id, initiative| {
        db.send_blocking(FollowActorSessionCommand {
            session_id,
            agent_id,
            initiative,
        })
        .unwrap()
    };
    assert_eq!(follower(), None);

    follow(scout, Initiative::User);
    follow(pilot, Initiative::Event);
    assert_eq!(follower(), Some(pilot));
    let session = db
        .send_blocking(GetActorSessionCommand { id: session_id })
        .unwrap();
    assert_eq!(session.follower.unwrap().initiative, Initiative::Event);
    // No move ends a follow, a stop included.
    for moved in [
        ActorSessionMove::NeedsYou,
        ActorSessionMove::Finished,
        ActorSessionMove::Failed,
        ActorSessionMove::Stopped,
    ] {
        db.send_blocking(SetActorSessionStatusCommand { session_id, moved })
            .unwrap();
        assert_eq!(follower(), Some(pilot));
    }
    // Only deleting the agent does.
    db.send_blocking(DeleteIntelligenceAgentCommand { agent_id: pilot })
        .unwrap();
    assert_eq!(follower(), None);
}

#[test]
fn an_agent_counts_the_sessions_it_follows_until_they_are_resolved() {
    let db = context();
    let session_id = actor_session(&db);
    let (scout, pilot) = (agent(&db, "Scout"), agent(&db, "Pilot"));
    let count = |agent_id| {
        db.send_blocking(GetIntelligenceAgentCommand { agent_id })
            .unwrap()
            .following_count
    };
    let follow = |agent_id| {
        db.send_blocking(FollowActorSessionCommand {
            session_id,
            agent_id,
            initiative: Initiative::User,
        })
        .unwrap()
    };
    let moved = |moved| {
        db.send_blocking(SetActorSessionStatusCommand { session_id, moved })
            .unwrap();
    };
    assert_eq!(count(scout), 0);

    follow(scout);
    assert_eq!(count(scout), 1);

    // A new follower takes the session from the last.
    follow(pilot);
    assert_eq!((count(scout), count(pilot)), (0, 1));

    // Done still counts; only resolving it ends that, and work reopens it.
    moved(ActorSessionMove::Finished);
    assert_eq!(count(pilot), 1);
    moved(ActorSessionMove::Resolved);
    assert_eq!(count(pilot), 0);
    moved(ActorSessionMove::Working);
    assert_eq!(count(pilot), 1);

    db.send_blocking(DeleteActorSessionCommand { id: session_id })
        .unwrap();
    assert_eq!(count(pilot), 0);
}

#[test]
fn the_agents_whose_count_changes_are_invalidated_and_no_others() {
    let db = context();
    let session_id = actor_session(&db);
    let (scout, pilot) = (agent(&db, "Scout"), agent(&db, "Pilot"));
    let mut batches = db.subscribe();
    let mut agents_invalidated = || -> Vec<i64> {
        let mut agents = Vec::new();
        while let Ok(batch) = batches.try_recv() {
            agents.extend(batch.keys.iter().filter_map(|key| match key {
                Invalidation::Agent(id) => Some(*id),
                _ => None,
            }));
        }
        agents.sort();
        agents
    };
    let follow = |agent_id| {
        db.send_blocking(FollowActorSessionCommand {
            session_id,
            agent_id,
            initiative: Initiative::User,
        })
        .unwrap()
    };
    let moved = |moved| {
        db.send_blocking(SetActorSessionStatusCommand { session_id, moved })
            .unwrap();
    };

    follow(scout);
    assert_eq!(agents_invalidated(), [scout]);
    follow(pilot);
    assert_eq!(agents_invalidated(), [scout, pilot]);
    follow(pilot);
    assert!(agents_invalidated().is_empty());

    // A move that stays short of resolved leaves the open conversation be.
    moved(ActorSessionMove::Finished);
    moved(ActorSessionMove::Working);
    assert!(agents_invalidated().is_empty());

    moved(ActorSessionMove::Resolved);
    assert_eq!(agents_invalidated(), [pilot]);
    moved(ActorSessionMove::Working);
    assert_eq!(agents_invalidated(), [pilot]);

    db.send_blocking(DeleteActorSessionCommand { id: session_id })
        .unwrap();
    assert_eq!(agents_invalidated(), [pilot]);
}
