use std::collections::BTreeMap;

use cohesion_core::model::{
    Extension, ExtensionHook, ExtensionManifest, ExtensionPermissionKind, HookKind,
    ServiceFunctionManifest, ServiceManifest,
};

use crate::test_support::context;
use crate::{
    AnnounceExtensionUpdateCommand, CommandContext, CommandError, DatabaseCommand,
    DatabaseCommandResult, DatabaseContext, DeleteExtensionCommand, GetCallableServiceCommand,
    GetExtensionHooksCommand, GetExtensionPermissionsCommand, GetExtensionServiceCommand,
    GetExtensionsWithHookCommand, GetExtensionsWithPermissionCommand, InstallExtensionCommand,
    ListServicesCommand, SetExtensionHookCommand, SetExtensionPermissionCommand, Write,
};
use crate::{Changed, Invalidates};

/// Inserts an extension at `priority` with each of `hooks`.
struct InsertExtension {
    identifier: &'static str,
    priority: i64,
    hooks: &'static [(HookKind, bool)],
}
impl DatabaseCommand for InsertExtension {
    type Response = ();
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<()>> {
        let extension: Extension = ctx
            .db
            .insert()
            .set(Extension::NAME, self.identifier)
            .set(Extension::VERSION, 1)
            .set(Extension::SCRIPT, "")
            .set(Extension::SCRIPT_IDENTIFIER, self.identifier)
            .set(Extension::PRIORITY, self.priority)
            .returning_one()?
            .unwrap();
        for (hook, enabled) in self.hooks {
            ctx.db
                .insert::<ExtensionHook>()
                .set(ExtensionHook::EXTENSION_ID, extension.id)
                .set(ExtensionHook::HOOK, hook.clone())
                .set(ExtensionHook::ENABLED, *enabled)
                .execute()?;
        }
        Ok(().invalidating_nothing())
    }
}

#[test]
fn extensions_with_hook_are_enabled_and_ordered() {
    use HookKind::{TaskCreate, TaskStatus};
    let db = context();
    for extension in [
        InsertExtension {
            identifier: "late",
            priority: 1,
            hooks: &[(TaskCreate, true)],
        },
        InsertExtension {
            identifier: "first",
            priority: 0,
            hooks: &[(TaskCreate, true), (TaskStatus, true)],
        },
        InsertExtension {
            identifier: "disabled",
            priority: 0,
            hooks: &[(TaskCreate, false)],
        },
        InsertExtension {
            identifier: "other",
            priority: 0,
            hooks: &[(TaskStatus, true)],
        },
        InsertExtension {
            identifier: "second",
            priority: 0,
            hooks: &[(TaskCreate, true)],
        },
    ] {
        db.send_blocking(extension).unwrap();
    }

    let identifiers: Vec<_> = db
        .send_blocking(GetExtensionsWithHookCommand { hook: TaskCreate })
        .unwrap()
        .into_iter()
        .map(|extension| extension.script_identifier)
        .collect();
    assert_eq!(identifiers, ["first", "second", "late"]);
}

/// An install of `identifier` whose manifest lists `permissions` and `hooks`.
fn installing(
    identifier: &str,
    permissions: &[ExtensionPermissionKind],
    hooks: &[HookKind],
    allow_overwrite: bool,
) -> InstallExtensionCommand {
    InstallExtensionCommand {
        connector: None,
        script: String::new(),
        manifest: ExtensionManifest {
            identifier: identifier.to_string(),
            name: identifier.to_string(),
            version: 1,
            description: None,
            author: None,
            icon: None,
            permissions: permissions.to_vec(),
            hooks: hooks.to_vec(),
            service: None,
        },
        allow_overwrite,
    }
}

/// Installs `identifier`, replacing an installed one. Returns the
/// extension's id.
fn install(
    db: &DatabaseContext,
    identifier: &str,
    permissions: &[ExtensionPermissionKind],
    hooks: &[HookKind],
) -> i64 {
    db.send_blocking(installing(identifier, permissions, hooks, true))
        .unwrap()
        .id
}

/// The extension's recorded permissions, sorted, since the command leaves
/// their order unspecified.
fn permissions(db: &DatabaseContext, extension_id: i64) -> Vec<(ExtensionPermissionKind, bool)> {
    let mut permissions: Vec<_> = db
        .send_blocking(GetExtensionPermissionsCommand { extension_id })
        .unwrap()
        .into_iter()
        .map(|grant| (grant.permission, grant.enabled))
        .collect();
    permissions.sort_by_key(|(permission, _)| permission.as_str());
    permissions
}

fn hooks(db: &DatabaseContext, extension_id: i64) -> Vec<(HookKind, bool)> {
    db.send_blocking(GetExtensionHooksCommand { extension_id })
        .unwrap()
        .into_iter()
        .map(|grant| (grant.hook, grant.enabled))
        .collect()
}

#[test]
fn fresh_install_grants_nothing() {
    use ExtensionPermissionKind::{Network, Registry};
    let db = context();
    let id = install(&db, "fresh", &[Network, Registry], &[HookKind::Launch]);
    assert_eq!(permissions(&db, id), [(Network, false), (Registry, false)]);
    assert_eq!(hooks(&db, id), [(HookKind::Launch, false)]);
}

#[test]
fn reinstall_keeps_grants_and_follows_the_manifest() {
    use ExtensionPermissionKind::{Network, Registry, Timer};
    let db = context();
    let id = install(&db, "ext", &[Network, Timer], &[HookKind::Launch]);
    db.send_blocking(SetExtensionPermissionCommand {
        extension_id: id,
        permission: Network,
        enabled: true,
    })
    .unwrap();
    db.send_blocking(SetExtensionHookCommand {
        extension_id: id,
        hook: HookKind::Launch,
        enabled: true,
    })
    .unwrap();

    // The upgrade drops `timer` and adds `registry`.
    assert_eq!(
        install(&db, "ext", &[Network, Registry], &[HookKind::Launch]),
        id
    );
    assert_eq!(permissions(&db, id), [(Network, true), (Registry, false)]);
    assert_eq!(hooks(&db, id), [(HookKind::Launch, true)]);
}

#[test]
fn an_update_is_announced_once_per_version() {
    let db = context();
    let id = install(&db, "ext", &[], &[]);
    let announce = |version: i64| {
        db.send_blocking(AnnounceExtensionUpdateCommand {
            extension_id: id,
            version,
        })
        .unwrap()
    };
    assert!(announce(2));
    assert!(!announce(2));
    assert!(announce(3));
}

#[test]
fn reinstall_without_overwrite_changes_nothing() {
    use ExtensionPermissionKind::{Network, Registry};
    let db = context();
    let id = install(&db, "ext", &[Network], &[HookKind::Launch]);
    let refused = db.send_blocking(installing("ext", &[Registry], &[], false));
    assert!(matches!(refused, Err(CommandError::Validation(_))));
    assert_eq!(permissions(&db, id), [(Network, false)]);
    assert_eq!(hooks(&db, id), [(HookKind::Launch, false)]);
}

#[test]
fn extensions_with_permission_are_granted_only() {
    use ExtensionPermissionKind::{Network, Registry};
    let db = context();
    let granted = install(&db, "granted", &[Registry], &[]);
    install(&db, "declared", &[Registry], &[]);
    let other = install(&db, "other", &[Network], &[]);
    for (extension_id, permission) in [(granted, Registry), (other, Network)] {
        db.send_blocking(SetExtensionPermissionCommand {
            extension_id,
            permission,
            enabled: true,
        })
        .unwrap();
    }

    let identifiers: Vec<_> = db
        .send_blocking(GetExtensionsWithPermissionCommand {
            permission: Registry,
        })
        .unwrap()
        .into_iter()
        .map(|extension| extension.script_identifier)
        .collect();
    assert_eq!(identifiers, ["granted"]);
}

#[test]
fn deleting_an_extension_drops_its_grants() {
    use ExtensionPermissionKind::Network;
    let db = context();
    let id = install(&db, "gone", &[Network], &[HookKind::Launch]);
    db.send_blocking(DeleteExtensionCommand { extension_id: id })
        .unwrap();
    assert!(permissions(&db, id).is_empty());
    assert!(hooks(&db, id).is_empty());
}

/// A service declaring a function per name in `functions`.
fn service(functions: &[&str]) -> ServiceManifest {
    ServiceManifest {
        description: None,
        functions: functions
            .iter()
            .map(|name| {
                let function = ServiceFunctionManifest {
                    description: None,
                    args: None,
                    returns: None,
                    read_only: None,
                };
                (name.to_string(), function)
            })
            .collect::<BTreeMap<_, _>>(),
    }
}

/// Installs `identifier` listing `permission`, declaring `service`, and
/// replacing an installed one. Returns the extension's id.
fn install_service(
    db: &DatabaseContext,
    identifier: &str,
    permission: ExtensionPermissionKind,
    service: Option<ServiceManifest>,
) -> Result<i64, CommandError> {
    let mut install = installing(identifier, &[permission], &[], true);
    install.manifest.service = service;
    Ok(db.send_blocking(install)?.id)
}

/// The declared function names of extension `extension_id`'s service.
fn declared(db: &DatabaseContext, extension_id: i64) -> Option<Vec<String>> {
    db.send_blocking(GetExtensionServiceCommand { extension_id })
        .unwrap()
        .map(|service| service.functions.into_iter().map(|f| f.name).collect())
}

#[test]
fn a_reinstall_replaces_the_service_declaration() {
    use ExtensionPermissionKind::Service;
    let db = context();
    let id = install_service(&db, "ext", Service, Some(service(&["read", "add"]))).unwrap();
    assert_eq!(declared(&db, id).unwrap(), ["add", "read"]);
    install_service(&db, "ext", Service, Some(service(&["reset"]))).unwrap();
    assert_eq!(declared(&db, id).unwrap(), ["reset"]);
    install_service(&db, "ext", Service, None).unwrap();
    assert_eq!(declared(&db, id), None);

    // An uninstall takes the declaration with it.
    install_service(&db, "ext", Service, Some(service(&["reset"]))).unwrap();
    db.send_blocking(DeleteExtensionCommand { extension_id: id })
        .unwrap();
    assert_eq!(declared(&db, id), None);
}

#[test]
fn only_services_with_service_enabled_are_callable() {
    use ExtensionPermissionKind::{Service, ServiceIsolated};
    let db = context();
    let enabled = install_service(&db, "b.enabled", Service, Some(service(&["f"]))).unwrap();
    let first = install_service(&db, "a.enabled", Service, Some(service(&["g"]))).unwrap();
    install_service(&db, "disabled", Service, Some(service(&["f"]))).unwrap();
    let isolated =
        install_service(&db, "isolated", ServiceIsolated, Some(service(&["f"]))).unwrap();
    let undeclared = install_service(&db, "undeclared", Service, None).unwrap();
    for (extension_id, permission) in [
        (enabled, Service),
        (first, Service),
        (isolated, ServiceIsolated),
        (undeclared, Service),
    ] {
        db.send_blocking(SetExtensionPermissionCommand {
            extension_id,
            permission,
            enabled: true,
        })
        .unwrap();
    }

    let listed = db.send_blocking(ListServicesCommand {}).unwrap();
    let identifiers: Vec<_> = listed
        .iter()
        .map(|s| s.script_identifier.as_str())
        .collect();
    assert_eq!(identifiers, ["a.enabled", "b.enabled"]);
    assert_eq!(listed[1].functions[0].name, "f");

    let callable = |identifier: &str| {
        db.send_blocking(GetCallableServiceCommand {
            script_identifier: identifier.to_string(),
        })
        .unwrap()
        .map(|service| service.extension_id)
    };
    assert_eq!(callable("b.enabled"), Some(enabled));
    for identifier in ["disabled", "isolated", "undeclared", "missing"] {
        assert_eq!(callable(identifier), None, "{identifier} is callable");
    }
}
