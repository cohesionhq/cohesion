use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::Module;

use crate::test_support::context;
use crate::{
    CommandError, CreateWorkspaceCommand, DatabaseContext, DeleteWorkspaceCommand,
    ListWorkspaceModulesCommand, SetModuleEnabledCommand,
};

fn workspace(db: &DatabaseContext, title: &str) -> i64 {
    db.send_blocking(CreateWorkspaceCommand {
        title: title.to_string(),
        emoji: None,
    })
    .unwrap()
    .id
}

fn calendar_enabled(db: &DatabaseContext, workspace_id: i64) -> bool {
    db.send_blocking(ListWorkspaceModulesCommand { workspace_id })
        .unwrap()
        .into_iter()
        .find(|setting| setting.module == Module::Calendar)
        .unwrap()
        .enabled
}

fn set_calendar(db: &DatabaseContext, workspace_id: i64, enabled: bool) {
    db.send_blocking(SetModuleEnabledCommand {
        workspace_id,
        module: Module::Calendar,
        enabled,
    })
    .unwrap();
}

#[test]
fn every_module_is_on_in_a_new_workspace() {
    let db = context();
    let home = workspace(&db, "Home");
    let modules = db
        .send_blocking(ListWorkspaceModulesCommand { workspace_id: home })
        .unwrap();
    assert_eq!(modules.len(), Module::ALL.len());
    assert!(modules.iter().all(|setting| setting.enabled));
    assert_eq!(modules[0].title, "Calendar");
}

#[test]
fn a_module_turned_off_is_off_only_in_its_workspace() {
    let db = context();
    let home = workspace(&db, "Home");
    let work = workspace(&db, "Work");
    set_calendar(&db, home, false);
    assert!(!calendar_enabled(&db, home));
    assert!(calendar_enabled(&db, work));
}

#[test]
fn a_module_turned_back_on_is_on_again() {
    let db = context();
    let home = workspace(&db, "Home");
    set_calendar(&db, home, false);
    set_calendar(&db, home, false);
    set_calendar(&db, home, true);
    assert!(calendar_enabled(&db, home));
}

#[test]
fn turning_a_module_on_or_off_invalidates_its_workspace_modules() {
    let db = context();
    let home = workspace(&db, "Home");
    let mut batches = db.subscribe();
    set_calendar(&db, home, false);
    let mut keys = Vec::new();
    while let Ok(batch) = batches.try_recv() {
        keys.extend(batch.keys.iter().cloned());
    }
    assert_eq!(keys, [Invalidation::Modules(home)]);
}

#[test]
fn a_deleted_workspace_has_no_modules() {
    let db = context();
    let home = workspace(&db, "Home");
    db.send_blocking(DeleteWorkspaceCommand { id: home })
        .unwrap();
    assert!(matches!(
        db.send_blocking(ListWorkspaceModulesCommand { workspace_id: home }),
        Err(CommandError::NotFound(_))
    ));
    assert!(matches!(
        db.send_blocking(SetModuleEnabledCommand {
            workspace_id: home,
            module: Module::Calendar,
            enabled: false,
        }),
        Err(CommandError::NotFound(_))
    ));
}
