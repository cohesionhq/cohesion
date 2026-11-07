use cohesion_core::model::{
    ConnectorDefinition, ConnectorResource, Credential, ExtensionManifest, MetadataKey,
    MetadataOwner, ResourceContentKind, ResourceKindDescription, ResourceSort, ServiceDescription,
};

use crate::test_support::recording_context;
use crate::{
    CommandError, CreateExtensionCommand, CreateWorkspaceCommand, DatabaseContext,
    DatabaseEventKind, GetAccountConnectorCommand, GetMetadataCommand, ImportResourceCommand,
    ListResourcesCommand, ListWorkspaceResourceKindsCommand, ReorderWorkspaceResourceKindsCommand,
    SetResourceKindShownCommand, UpdateResourceCommand, UpsertConnectorAccountCommand,
};

/// A workspace and a connector account to import into it from.
fn setup(db: &DatabaseContext) -> (i64, i64) {
    let (workspace, _, account) = setup_extension(db);
    (workspace, account)
}

/// A workspace, an extension and its connector account, whose definition
/// declares pages and databases: their ids.
fn setup_extension(db: &DatabaseContext) -> (i64, i64, i64) {
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
    let account = connect(
        db,
        extension.id,
        1,
        &[("page", "Pages"), ("database", "Databases")],
    );
    (workspace.id, extension.id, account)
}

/// Adds the extension's account, its definition at `version` declaring
/// `kinds` (each a key and a plural): the account's id.
fn connect(db: &DatabaseContext, extension_id: i64, version: i64, kinds: &[(&str, &str)]) -> i64 {
    db.send_blocking(UpsertConnectorAccountCommand {
        extension_id,
        connector: ConnectorDefinition {
            identifier: "com.example.c".to_string(),
            icon: None,
            version,
            description: ServiceDescription {
                sign_in_hint: None,
                title: "Service".to_string(),
                unit: "Workspace".to_string(),
                kinds: kinds
                    .iter()
                    .map(|(identifier, plural)| ResourceKindDescription {
                        identifier: identifier.to_string(),
                        title: identifier.to_string(),
                        plural: plural.to_string(),
                        contents: "Markdown".to_string(),
                    })
                    .collect(),
                metadata_keys: vec![MetadataKey::Author],
            },
        },
        identifier: "acct-1".to_string(),
        description: "Example Co.".to_string(),
        credential: Credential {
            authorization: "a".to_string(),
            refresh: None,
            expires_at: None,
        },
    })
    .unwrap()
    .id
}

fn remote(identifier: &str, title: &str, last_updated: i64) -> ConnectorResource {
    ConnectorResource {
        identifier: identifier.to_string(),
        last_updated,
        title: title.to_string(),
        contents: "# Notes".to_string(),
        content_kind: ResourceContentKind::Markdown,
        kind_identifier: "page".to_string(),
        metadata: [(MetadataKey::Author, "Ada".to_string())].into(),
    }
}

/// Imports `resource` as one of kind `kind`.
fn import(
    db: &DatabaseContext,
    ws: i64,
    account: i64,
    kind: &str,
    mut resource: ConnectorResource,
) -> i64 {
    resource.kind_identifier = kind.to_string();
    db.send_blocking(ImportResourceCommand {
        workspace_id: ws,
        account_id: account,
        resource,
    })
    .unwrap()
    .id
}

#[test]
fn importing_upserts_by_identifier_and_raises_an_event() {
    let (db, events) = recording_context();
    let (ws, account) = setup(&db);
    events.take();

    let first = import(&db, ws, account, "page", remote("p1", "Roadmap", 1));
    let second = import(&db, ws, account, "page", remote("p1", "Roadmap v2", 2));
    assert_eq!(first, second);

    let listed = db
        .send_blocking(ListResourcesCommand {
            workspace_id: ws,
            kind: None,
            search: None,
            sort: ResourceSort::LastUpdated,
            descending: true,
        })
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].title, "Roadmap v2");
    assert_eq!(
        events.take(),
        [
            DatabaseEventKind::ResourceImported {
                resource_id: first,
                workspace_id: ws
            },
            DatabaseEventKind::ResourceImported {
                resource_id: first,
                workspace_id: ws
            },
        ]
    );
}

#[test]
fn updating_writes_only_when_the_item_changed() {
    let (db, events) = recording_context();
    let (ws, account) = setup(&db);
    let id = import(&db, ws, account, "page", remote("p1", "Roadmap", 1));
    let update = |resource| {
        db.send_blocking(UpdateResourceCommand {
            workspace_id: ws,
            resource_id: id,
            resource,
        })
        .unwrap()
    };

    assert!(update(remote("p1", "Roadmap", 1)).is_none());
    events.take();
    let updated = update(remote("p1", "Roadmap", 2)).unwrap();
    assert_eq!(updated.last_updated, 2);
    // Flush the update's events.
    assert!(update(remote("p1", "Roadmap", 2)).is_none());
    assert_eq!(
        events.take(),
        [DatabaseEventKind::ResourceUpdated {
            resource_id: id,
            workspace_id: ws
        }]
    );
}

#[test]
fn sorting_by_a_metadata_key_puts_resources_without_it_last_either_way() {
    let (db, _) = recording_context();
    let (ws, account) = setup(&db);
    let by = |identifier: &str, author: Option<&str>| {
        let mut resource = remote(identifier, identifier, 1);
        resource.metadata = author
            .map(|author| [(MetadataKey::Author, author.to_string())].into())
            .unwrap_or_default();
        import(&db, ws, account, "page", resource);
    };
    by("a", Some("bea"));
    by("b", None);
    by("c", Some("Ada"));
    let list = |descending| {
        db.send_blocking(ListResourcesCommand {
            workspace_id: ws,
            kind: None,
            search: None,
            sort: ResourceSort::Metadata(MetadataKey::Author),
            descending,
        })
        .unwrap()
        .into_iter()
        .map(|resource| resource.identifier)
        .collect::<Vec<_>>()
    };

    assert_eq!(list(false), ["c", "a", "b"]);
    assert_eq!(list(true), ["a", "c", "b"]);
}

#[test]
fn listing_filters_by_kind_and_title_newest_first() {
    let (db, _) = recording_context();
    let (ws, account) = setup(&db);
    import(&db, ws, account, "page", remote("p1", "Old roadmap", 1));
    import(&db, ws, account, "page", remote("p2", "New Roadmap", 2));
    import(
        &db,
        ws,
        account,
        "database",
        remote("d1", "Roadmap table", 3),
    );
    let list = |kind: Option<&str>, search: Option<&str>| {
        db.send_blocking(ListResourcesCommand {
            workspace_id: ws,
            kind: kind.map(str::to_string),
            search: search.map(str::to_string),
            sort: ResourceSort::LastUpdated,
            descending: true,
        })
        .unwrap()
        .into_iter()
        .map(|resource| resource.identifier)
        .collect::<Vec<_>>()
    };

    assert_eq!(list(None, None), ["d1", "p2", "p1"]);
    assert_eq!(list(Some("com.example.c.page"), None), ["p2", "p1"]);
    assert_eq!(
        list(Some("com.example.c.page"), Some("ROADMAP")),
        ["p2", "p1"]
    );
    assert!(list(Some("page"), None).is_empty());
    assert_eq!(list(None, Some("table")), ["d1"]);
}

#[test]
fn a_fetch_replaces_the_metadata_and_a_change_to_it_is_an_update() {
    let (db, _) = recording_context();
    let (ws, account) = setup(&db);
    let id = import(&db, ws, account, "page", remote("p1", "Roadmap", 1));
    let author = || {
        db.send_blocking(GetMetadataCommand {
            owner: MetadataOwner::Resource { resource_id: id },
            key: MetadataKey::Author.as_str().to_string(),
        })
        .unwrap()
    };
    assert_eq!(author().as_deref(), Some("Ada"));

    let mut fresh = remote("p1", "Roadmap", 1);
    fresh.metadata = [(MetadataKey::Url, "https://example.com".to_string())].into();
    let updated = db
        .send_blocking(UpdateResourceCommand {
            workspace_id: ws,
            resource_id: id,
            resource: fresh,
        })
        .unwrap();
    assert!(updated.is_some());
    assert_eq!(author(), None);
}

#[test]
fn a_kind_a_newer_definition_drops_goes_with_its_resources() {
    let (db, _) = recording_context();
    let (ws, extension, account) = setup_extension(&db);
    import(&db, ws, account, "page", remote("p1", "Roadmap", 1));
    import(&db, ws, account, "database", remote("d1", "Budget", 1));
    // An item of a kind the connector doesn't declare isn't imported.
    let mut stray = remote("x1", "Stray", 1);
    stray.kind_identifier = "board".to_string();
    assert!(matches!(
        db.send_blocking(ImportResourceCommand {
            workspace_id: ws,
            account_id: account,
            resource: stray,
        }),
        Err(CommandError::Validation(_))
    ));

    let page = db
        .send_blocking(GetAccountConnectorCommand {
            account_id: account,
        })
        .unwrap()
        .kinds[0]
        .id;
    connect(&db, extension, 2, &[("page", "Documents")]);
    let connector = db
        .send_blocking(GetAccountConnectorCommand {
            account_id: account,
        })
        .unwrap();
    // The kind kept keeps its id and takes the new plural.
    assert_eq!(
        connector
            .kinds
            .iter()
            .map(|k| (k.id, k.plural.as_str()))
            .collect::<Vec<_>>(),
        [(page, "Documents")]
    );
    let left: Vec<String> = db
        .send_blocking(ListResourcesCommand {
            workspace_id: ws,
            kind: None,
            search: None,
            sort: ResourceSort::LastUpdated,
            descending: true,
        })
        .unwrap()
        .into_iter()
        .map(|resource| resource.identifier)
        .collect();
    assert_eq!(left, ["p1"]);
}

#[test]
fn a_kind_is_hidden_in_a_workspace_until_shown_there() {
    let (db, _) = recording_context();
    let (ws, account) = setup(&db);
    let other = db
        .send_blocking(CreateWorkspaceCommand {
            title: "work".to_string(),
            emoji: None,
        })
        .unwrap()
        .id;
    let page = db
        .send_blocking(GetAccountConnectorCommand {
            account_id: account,
        })
        .unwrap()
        .kinds[0]
        .id;
    let shown = |workspace_id| {
        db.send_blocking(ListWorkspaceResourceKindsCommand { workspace_id })
            .unwrap()
            .into_iter()
            .map(|shown| shown.kind_id)
            .collect::<Vec<_>>()
    };
    assert!(shown(ws).is_empty());
    let set = |shown| {
        db.send_blocking(SetResourceKindShownCommand {
            workspace_id: ws,
            kind_id: page,
            shown,
        })
        .unwrap()
    };
    set(true);
    // Showing it twice is still once.
    assert_eq!(set(true).len(), 1);
    assert_eq!(shown(ws), [page]);
    assert!(shown(other).is_empty());
    set(false);
    assert!(shown(ws).is_empty());
}

#[test]
fn a_shown_kind_goes_last_and_keeps_its_place_until_reordered() {
    let (db, _) = recording_context();
    let (ws, account) = setup(&db);
    let kinds = db
        .send_blocking(GetAccountConnectorCommand {
            account_id: account,
        })
        .unwrap()
        .kinds;
    let (page, database) = (kinds[0].id, kinds[1].id);
    let show = |kind_id, shown| {
        db.send_blocking(SetResourceKindShownCommand {
            workspace_id: ws,
            kind_id,
            shown,
        })
        .unwrap()
        .into_iter()
        .map(|shown| shown.kind_id)
        .collect::<Vec<_>>()
    };
    show(database, true);
    assert_eq!(show(page, true), [database, page]);
    let reordered: Vec<i64> = db
        .send_blocking(ReorderWorkspaceResourceKindsCommand {
            workspace_id: ws,
            kind_ids: vec![page, database],
        })
        .unwrap()
        .into_iter()
        .map(|shown| shown.kind_id)
        .collect();
    assert_eq!(reordered, [page, database]);
    // Showing it again leaves it where it is; hidden and shown, it goes last.
    assert_eq!(show(page, true), [page, database]);
    show(page, false);
    assert_eq!(show(page, true), [database, page]);
    // A hidden kind has no place to take.
    show(page, false);
    assert!(
        db.send_blocking(ReorderWorkspaceResourceKindsCommand {
            workspace_id: ws,
            kind_ids: vec![page, database],
        })
        .is_err()
    );
}
