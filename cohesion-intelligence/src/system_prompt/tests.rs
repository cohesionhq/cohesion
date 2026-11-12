use cohesion_core::model::{
    IntelligenceAgent, IntelligenceAgentColor, IntelligenceAgentPersonality, IntelligenceAgentTeam,
    IntelligenceAgentVerbosity, Memory, MemoryScope,
};

use super::{AboutUser, Profile, Roster, SystemPrompt};

fn agent(id: i64, name: &str, team_id: Option<i64>) -> IntelligenceAgent {
    IntelligenceAgent {
        id,
        created_at: 0,
        agent_name: name.to_string(),
        agent_color: IntelligenceAgentColor::Blue,
        agent_emoji: None,
        agent_personality: IntelligenceAgentPersonality::Balanced,
        agent_verbosity: IntelligenceAgentVerbosity::Brief,
        agent_role: String::new(),
        agent_directive: String::new(),
        activity: None,
        foreground_started: None,
        context_tokens: None,
        agent_unread: false,
        last_message_at: None,
        pinned: false,
        sort_order: 0,
        team_id,
        workspace_id: None,
        endpoint_model_id: None,
        following_count: 0,
    }
}

fn teams() -> Vec<IntelligenceAgentTeam> {
    vec![
        IntelligenceAgentTeam {
            id: 1,
            title: "Engineering".to_string(),
        },
        IntelligenceAgentTeam {
            id: 2,
            title: "Research".to_string(),
        },
    ]
}

/// The names of `roster`'s teammates.
fn teammates(roster: &Roster) -> Option<Vec<&str>> {
    roster.team.as_ref().map(|team| {
        team.members
            .iter()
            .map(|member| member.name.as_str())
            .collect()
    })
}

/// The names of `roster`'s other agents, each with its team.
fn others(roster: &Roster) -> Vec<(&str, Option<&str>)> {
    roster
        .others
        .iter()
        .map(|other| (other.name.as_str(), other.team.as_deref()))
        .collect()
}

#[test]
fn teammates_are_apart_from_the_other_agents_who_carry_their_team() {
    let agents = [
        agent(1, "Lead", Some(1)),
        agent(2, "Core", Some(1)),
        agent(3, "Researcher", Some(2)),
        agent(4, "Secretary", None),
    ];
    let roster = Roster::of(&agents[0], &agents, &teams());
    assert_eq!(roster.team.as_ref().unwrap().title, "Engineering");
    assert_eq!(teammates(&roster), Some(vec!["Core"]));
    assert_eq!(
        others(&roster),
        [("Researcher", Some("Research")), ("Secretary", None)]
    );
}

#[test]
fn an_agent_on_no_team_sees_every_other_agent_as_other() {
    let agents = [agent(1, "Secretary", None), agent(2, "Other", None)];
    let roster = Roster::of(&agents[0], &agents, &teams());
    assert_eq!(teammates(&roster), None);
    assert_eq!(others(&roster), [("Other", None)]);
}

#[test]
fn the_roster_lists_agents_by_id_whatever_order_they_come_in() {
    let agents = [
        agent(4, "Secretary", None),
        agent(1, "Lead", Some(1)),
        agent(3, "Researcher", Some(2)),
        agent(2, "Core", None),
    ];
    let roster = Roster::of(&agents[1], &agents, &teams());
    assert_eq!(
        others(&roster),
        [
            ("Core", None),
            ("Researcher", Some("Research")),
            ("Secretary", None)
        ]
    );
}

fn memory(id: i64, scope: MemoryScope, memory: &str) -> Memory {
    Memory {
        id,
        scope,
        created_at: 0,
        memory: memory.to_string(),
    }
}

// The prompt ends with the memories, so a new thread's prompt
// shares everything before them with the last one's, whatever was
// remembered or forgotten between them and whatever the capacity became.
#[test]
fn memories_close_every_prompt_and_change_only_its_tail() {
    let profile = Profile {
        directive: "Never book meetings on Fridays.".to_string(),
        ..Profile::default()
    };
    let memories = vec![
        memory(
            12,
            MemoryScope::Agent { agent_id: 1 },
            "Jo files tasks in Inbox.",
        ),
        memory(
            7,
            MemoryScope::Team { team_id: 1 },
            "Releases go out on Fridays.",
        ),
        memory(3, MemoryScope::Global, "The user is called Jo."),
    ];
    let none = SystemPrompt {
        profile: profile.clone(),
        roster: Roster::default(),
        user: AboutUser::default(),
        memories: Vec::new(),
        memory_limit: 20,
    }
    .prompt("base");
    let some = SystemPrompt {
        profile,
        roster: Roster::default(),
        user: AboutUser::default(),
        memories,
        memory_limit: 5,
    }
    .prompt("base");
    assert!(none.ends_with("\nNone yet."));
    assert!(some.ends_with(
        "\n[m12] Jo files tasks in Inbox.\n[m7] (team) Releases go out on Fridays.\n\
         [m3] (global) The user is called Jo."
    ));
    let at = |prompt: &str| prompt.rfind("\n\n# Memories\n").unwrap();
    assert_eq!(none[..at(&none)], some[..at(&some)]);
    assert!(none[..at(&none)].ends_with("Never book meetings on Fridays.\n\n# Your model: base"));
}

// The model is named after everything agents share as a prefix, and nothing
// else in the prompt moves with it.
#[test]
fn the_prompt_names_the_model_it_is_given_and_differs_only_there() {
    let prompt = SystemPrompt {
        profile: Profile::default(),
        roster: Roster::default(),
        user: AboutUser::default(),
        memories: Vec::new(),
        memory_limit: 20,
    };
    let on = |model: &str| prompt.prompt(model);
    let line = |model: &str| format!("\n\n# Your model: {model}\n\n# Memories\n");
    assert!(on("claude-opus-5-5").contains(&line("claude-opus-5-5")));
    assert_eq!(
        on("claude-opus-5-5").replace(&line("claude-opus-5-5"), ""),
        on("gpt-6").replace(&line("gpt-6"), "")
    );
}

/// The prompt on `base` of an agent with one memory, for a user with `name`
/// and `bio`.
fn prompt_for(name: Option<&str>, bio: Option<&str>) -> String {
    SystemPrompt {
        profile: Profile::default(),
        roster: Roster::default(),
        user: AboutUser {
            name: name.map(str::to_string),
            bio: bio.map(str::to_string),
        },
        memories: vec![memory(3, MemoryScope::Global, "Reports go out as PDFs.")],
        memory_limit: 20,
    }
    .prompt("base")
}

// The user sits between the model and the memories, so memories still close
// the prompt, and their bio reads as they wrote it.
#[test]
fn the_user_comes_after_the_model_and_before_the_memories() {
    let bio = "I run a small bakery.\n\n  Mornings are busy: don't expect quick replies.";
    let prompt = prompt_for(Some("Ada"), Some(bio));
    assert!(prompt.contains(&format!(
        "\n\n# Your model: base\n\n# About the user\nName: Ada\nIn their own words:\n{bio}\n\n# Memories\n"
    )));
    assert!(prompt.ends_with("\n[m3] (global) Reports go out as PDFs."));
}

#[test]
fn the_section_holds_only_what_the_user_set_and_is_left_out_when_nothing_is() {
    assert!(
        prompt_for(Some("Ada"), None)
            .contains("\n\n# Your model: base\n\n# About the user\nName: Ada\n\n# Memories\n")
    );
    assert!(
        prompt_for(None, Some("I bake."))
            .contains("\n\n# About the user\nIn their own words:\nI bake.\n\n# Memories\n")
    );
    let nothing = prompt_for(None, None);
    assert!(!nothing.contains("# About the user"));
    assert!(nothing.contains("\n\n# Your model: base\n\n# Memories\n"));
}

// What agents share as a cached prefix doesn't move with who the user is.
#[test]
fn the_user_changes_nothing_before_the_model_line() {
    let upto = |prompt: String| {
        let at = prompt.find("\n\n# Your model: ").unwrap();
        prompt[..at].to_string()
    };
    assert_eq!(
        upto(prompt_for(Some("Ada"), Some("I bake."))),
        upto(prompt_for(None, None))
    );
}

// A live thread hears what changed since it last heard: new memories as the
// prompt lists them and the ones gone by id, then nothing until the next
// change.
#[test]
fn a_thread_hears_only_what_changed_since_it_last_heard() {
    let jo = memory(3, MemoryScope::Global, "The user is called Jo.");
    let inbox = memory(
        12,
        MemoryScope::Agent { agent_id: 1 },
        "Jo files tasks in Inbox.",
    );
    let fridays = memory(
        14,
        MemoryScope::Team { team_id: 1 },
        "Releases go out on Fridays.",
    );
    let mut prompt = SystemPrompt {
        profile: Profile::default(),
        roster: Roster::default(),
        user: AboutUser::default(),
        memories: vec![inbox.clone(), jo.clone()],
        memory_limit: 20,
    }
    .heard();

    assert!(prompt.hear(vec![inbox.clone(), jo.clone()]).is_none());
    let changed = prompt.hear(vec![fridays.clone(), jo.clone()]).unwrap();
    assert!(changed.starts_with("# Memories changed\n"));
    assert!(changed.ends_with("\n[m14] (team) Releases go out on Fridays.\nforgot [m12]"));
    assert!(prompt.hear(vec![fridays.clone(), jo.clone()]).is_none());
    assert!(
        prompt
            .hear(vec![fridays])
            .unwrap()
            .ends_with("\nforgot [m3]")
    );
}
