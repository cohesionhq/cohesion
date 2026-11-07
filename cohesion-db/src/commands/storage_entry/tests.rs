use cohesion_core::model::{ExtensionManifest, IntelligenceAgentColor};

use crate::test_support::context;
use crate::{
    CreateIntelligenceAgentCommand, DatabaseContext, DeleteExtensionCommand,
    DeleteIntelligenceAgentCommand, GetStorageEntryCommand, InstallExtensionCommand,
    ListStorageKeysCommand, SandboxKey, SetStorageEntryCommand,
};

/// Installs `identifier`, replacing an installed one. Returns its id.
fn install(db: &DatabaseContext, identifier: &str) -> i64 {
    db.send_blocking(InstallExtensionCommand {
        connector: None,
        script: String::new(),
        manifest: ExtensionManifest {
            identifier: identifier.to_string(),
            name: identifier.to_string(),
            version: 1,
            description: None,
            author: None,
            icon: None,
            permissions: Vec::new(),
            hooks: Vec::new(),
            service: None,
        },
        allow_overwrite: true,
    })
    .unwrap()
    .id
}

fn set(db: &DatabaseContext, owner: &SandboxKey, key: &str, value: Option<&str>) {
    db.send_blocking(SetStorageEntryCommand {
        sandbox: owner.clone(),
        key: key.to_string(),
        value: value.map(str::to_string),
    })
    .unwrap()
}

fn get(db: &DatabaseContext, owner: &SandboxKey, key: &str) -> Option<String> {
    db.send_blocking(GetStorageEntryCommand {
        sandbox: owner.clone(),
        key: key.to_string(),
    })
    .unwrap()
}

fn keys(db: &DatabaseContext, owner: &SandboxKey) -> Vec<String> {
    db.send_blocking(ListStorageKeysCommand {
        sandbox: owner.clone(),
    })
    .unwrap()
}

#[test]
fn each_owner_has_its_own_store() {
    let db = context();
    install(&db, "com.example.one");
    install(&db, "com.example.two");
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
    let one = SandboxKey::Extension("com.example.one".to_string());
    let two = SandboxKey::Extension("com.example.two".to_string());
    let scout = SandboxKey::Agent(agent.id);
    let impostor = SandboxKey::Extension(format!("agent:{}", agent.id));

    set(&db, &one, "cursor", Some("1"));
    set(&db, &one, "cursor", Some("2"));
    set(&db, &one, "seen", Some("true"));
    set(&db, &two, "cursor", Some("\"two\""));
    set(&db, &scout, "cursor", Some("\"scout\""));

    assert_eq!(get(&db, &one, "cursor").as_deref(), Some("2"));
    assert_eq!(get(&db, &two, "cursor").as_deref(), Some("\"two\""));
    assert_eq!(get(&db, &scout, "cursor").as_deref(), Some("\"scout\""));
    assert_eq!(get(&db, &impostor, "cursor"), None);
    assert_eq!(keys(&db, &one), ["cursor", "seen"]);
    assert_eq!(keys(&db, &two), ["cursor"]);

    set(&db, &one, "seen", None);
    assert_eq!(get(&db, &one, "seen"), None);
    assert_eq!(keys(&db, &one), ["cursor"]);
}

#[test]
fn a_store_outlives_a_reinstall_but_not_its_owner() {
    let db = context();
    let id = install(&db, "com.example.one");
    let one = SandboxKey::Extension("com.example.one".to_string());
    set(&db, &one, "cursor", Some("1"));

    install(&db, "com.example.one");
    assert_eq!(get(&db, &one, "cursor").as_deref(), Some("1"));

    db.send_blocking(DeleteExtensionCommand { extension_id: id })
        .unwrap();
    install(&db, "com.example.one");
    assert_eq!(get(&db, &one, "cursor"), None);

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
    let scout = SandboxKey::Agent(agent.id);
    set(&db, &scout, "seen", Some("[]"));
    db.send_blocking(DeleteIntelligenceAgentCommand { agent_id: agent.id })
        .unwrap();
    assert!(keys(&db, &scout).is_empty());
}
