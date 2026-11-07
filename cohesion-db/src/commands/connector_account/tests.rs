use cohesion_core::Workspace;
use cohesion_core::model::{
    AccountHost, ConnectorDefinition, ConnectorResource, Credential, ExtensionManifest,
    MetadataKey, ResourceContentKind, ResourceKindDescription, ServiceDescription,
};
use cohesion_core::protocol::RedactedConnectorAccount;

use crate::test_support::context;
use crate::{
    CommandError, CreateExtensionCommand, CreateWorkspaceCommand, DatabaseContext,
    DeleteConnectorAccountCommand, DeleteExtensionCommand, GetConnectorCredentialCommand,
    ImportResourceCommand, ListAccountResourcesCommand, ListConnectorAccountRowsCommand,
    ListConnectorAccountsCommand, MigrateResourcesCommand, UpsertConnectorAccountCommand,
};

/// Connector `identifier` at `version`, titled `title`.
fn definition(identifier: &str, version: i64, title: &str) -> ConnectorDefinition {
    ConnectorDefinition {
        identifier: identifier.to_string(),
        icon: None,
        version,
        description: ServiceDescription {
            sign_in_hint: None,
            title: title.to_string(),
            unit: "Workspace".to_string(),
            kinds: vec![ResourceKindDescription {
                identifier: "page".to_string(),
                title: "Page".to_string(),
                plural: "Pages".to_string(),
                contents: "Markdown".to_string(),
            }],
            metadata_keys: vec![MetadataKey::Author],
        },
    }
}

fn upsert(
    db: &DatabaseContext,
    extension_id: i64,
    authorization: &str,
) -> RedactedConnectorAccount {
    db.send_blocking(UpsertConnectorAccountCommand {
        extension_id,
        connector: definition("com.example.c", 1, "Service"),
        identifier: "acct-1".to_string(),
        description: "Example Co.".to_string(),
        credential: Credential {
            authorization: authorization.to_string(),
            refresh: Some("r".to_string()),
            expires_at: Some(100),
        },
    })
    .unwrap()
}

fn setup(db: &DatabaseContext) -> (Workspace, i64) {
    let workspace = db
        .send_blocking(CreateWorkspaceCommand {
            title: "home".to_string(),
            emoji: None,
        })
        .unwrap();
    let extension = db
        .send_blocking(CreateExtensionCommand {
            script: String::new(),
            manifest: ExtensionManifest {
                identifier: "com.example.c".to_string(),
                name: "c".to_string(),
                version: 1,
                description: None,
                author: None,
                icon: None,
                permissions: vec![],
                hooks: vec![],
                service: None,
            },
        })
        .unwrap();
    (workspace, extension.id)
}

#[test]
fn adding_the_same_account_again_replaces_its_credential() {
    let db = context();
    let (_, extension_id) = setup(&db);
    let first = upsert(&db, extension_id, "a");
    let second = upsert(&db, extension_id, "b");
    assert_eq!(first.id, second.id);
    let rows = db
        .send_blocking(ListConnectorAccountRowsCommand {})
        .unwrap();
    assert_eq!(rows.len(), 1);
    let stored = db
        .send_blocking(GetConnectorCredentialCommand {
            extension_id,
            account_id: first.id,
        })
        .unwrap();
    assert_eq!(stored.authorization, "b");
}

/// Imports item `identifier` of account `account_id` into `workspace`.
fn import(db: &DatabaseContext, workspace: &Workspace, account_id: i64, identifier: &str) {
    db.send_blocking(ImportResourceCommand {
        workspace_id: workspace.id,
        account_id,
        resource: ConnectorResource {
            identifier: identifier.to_string(),
            last_updated: 1,
            title: "Roadmap".to_string(),
            contents: "# Roadmap".to_string(),
            content_kind: ResourceContentKind::Markdown,
            kind_identifier: "page".to_string(),
            metadata: [(MetadataKey::Author, "Ada".to_string())].into(),
        },
    })
    .unwrap();
}

/// The identifiers of the resources imported from `account_id`.
fn resources(db: &DatabaseContext, account_id: i64) -> Vec<String> {
    db.send_blocking(ListAccountResourcesCommand {
        account_id,
        workspace_id: None,
    })
    .unwrap()
    .into_iter()
    .map(|resource| resource.identifier)
    .collect()
}

#[test]
fn deleting_an_account_removes_its_resources() {
    let db = context();
    let (workspace, extension_id) = setup(&db);
    let account = upsert(&db, extension_id, "a");
    import(&db, &workspace, account.id, "p1");

    db.send_blocking(DeleteConnectorAccountCommand {
        account_id: account.id,
    })
    .unwrap();
    assert!(resources(&db, account.id).is_empty());
}

#[test]
fn uninstalling_an_extension_orphans_its_accounts_without_their_credential() {
    let db = context();
    let (workspace, extension_id) = setup(&db);
    let account = upsert(&db, extension_id, "a");
    import(&db, &workspace, account.id, "p1");

    db.send_blocking(DeleteExtensionCommand { extension_id })
        .unwrap();
    let listed = db.send_blocking(ListConnectorAccountsCommand {}).unwrap();
    assert!(matches!(
        listed.as_slice(),
        [RedactedConnectorAccount { host: AccountHost::Orphaned, importable: false, description, .. }]
            if description == "Example Co."
    ));
    assert_eq!(resources(&db, account.id), ["p1"]);
    assert!(matches!(
        db.send_blocking(GetConnectorCredentialCommand {
            extension_id,
            account_id: account.id,
        }),
        Err(CommandError::NotFound(_))
    ));
}

#[test]
fn a_definition_is_rewritten_only_by_a_higher_version() {
    let db = context();
    let (_, extension_id) = setup(&db);
    let title = |version: i64, title: &str| {
        db.send_blocking(UpsertConnectorAccountCommand {
            extension_id,
            connector: definition("COM.example.c", version, title),
            identifier: "acct-1".to_string(),
            description: "Example Co.".to_string(),
            credential: Credential {
                authorization: "a".to_string(),
                refresh: None,
                expires_at: None,
            },
        })
        .unwrap()
        .connector
        .title
    };
    assert_eq!(title(2, "Second"), "Second");
    assert_eq!(title(1, "First"), "Second");
    assert_eq!(title(2, "Again"), "Second");
    assert_eq!(title(3, "Third"), "Third");
}

/// Extension `extension_id`'s account `identifier` of `connector`.
fn account(
    db: &DatabaseContext,
    extension_id: i64,
    connector: ConnectorDefinition,
    identifier: &str,
) -> RedactedConnectorAccount {
    db.send_blocking(UpsertConnectorAccountCommand {
        extension_id,
        connector,
        identifier: identifier.to_string(),
        description: identifier.to_string(),
        credential: Credential {
            authorization: "a".to_string(),
            refresh: None,
            expires_at: None,
        },
    })
    .unwrap()
}

#[test]
fn migrating_keeps_the_targets_copy_and_stays_within_a_connector() {
    let db = context();
    let (workspace, extension_id) = setup(&db);
    let service = definition("com.example.c", 1, "Service");
    let from = account(&db, extension_id, service.clone(), "acct-1");
    let to = account(&db, extension_id, service, "acct-2");
    import(&db, &workspace, from.id, "n1");
    import(&db, &workspace, from.id, "n2");
    import(&db, &workspace, to.id, "n2");
    let kept = db
        .send_blocking(ListAccountResourcesCommand {
            account_id: to.id,
            workspace_id: None,
        })
        .unwrap()[0]
        .id;

    db.send_blocking(MigrateResourcesCommand {
        from_account_id: from.id,
        to_account_id: to.id,
    })
    .unwrap();
    assert!(resources(&db, from.id).is_empty());
    let moved = db
        .send_blocking(ListAccountResourcesCommand {
            account_id: to.id,
            workspace_id: None,
        })
        .unwrap();
    assert_eq!(moved.len(), 2);
    assert!(moved.iter().any(|resource| resource.id == kept));

    let other = account(
        &db,
        extension_id,
        definition("com.example.d", 1, "Other"),
        "acct-3",
    );
    assert!(
        db.send_blocking(MigrateResourcesCommand {
            from_account_id: to.id,
            to_account_id: other.id,
        })
        .is_err()
    );
    assert_eq!(resources(&db, to.id).len(), 2);
}
