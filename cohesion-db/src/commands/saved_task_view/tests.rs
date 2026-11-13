use cohesion_core::model::{ListSymbol, SavedTaskView, TagFilter, TaskViewList, TaskViewScope};
use cohesion_core::{TaskPriority, TaskSort};

use crate::test_support::context;
use crate::{
    CommandError, CreateSavedTaskViewCommand, CreateTagCommand, CreateTaskFolderCommand,
    CreateWorkspaceCommand, DatabaseContext, DeleteTagCommand, DeleteTaskFolderCommand,
    ListSavedTaskViewsCommand, RenameSavedTaskViewCommand, SetSavedTaskViewSymbolCommand,
    UpdateSavedTaskViewCommand,
};

fn workspace(db: &DatabaseContext) -> i64 {
    db.send_blocking(CreateWorkspaceCommand {
        title: "w".to_string(),
        emoji: None,
    })
    .unwrap()
    .id
}

fn tag(db: &DatabaseContext, ws: i64, name: &str) -> i64 {
    db.send_blocking(CreateTagCommand {
        workspace_id: ws,
        name: name.to_string(),
    })
    .unwrap()
    .id
}

fn folder(db: &DatabaseContext, ws: i64) -> i64 {
    db.send_blocking(CreateTaskFolderCommand {
        workspace_id: ws,
        title: "f".to_string(),
    })
    .unwrap()
    .id
}

fn filter(with: Vec<i64>, without: Vec<i64>) -> TagFilter {
    TagFilter { with, without }
}

const INBOX: TaskViewScope = TaskViewScope::List {
    list: TaskViewList::Inbox,
};

fn create(
    db: &DatabaseContext,
    ws: i64,
    name: &str,
    scope: TaskViewScope,
    tags: TagFilter,
) -> Result<SavedTaskView, CommandError> {
    db.send_blocking(CreateSavedTaskViewCommand {
        workspace_id: ws,
        name: name.to_string(),
        scope,
        tags,
        priorities: vec![TaskPriority::Urgent, TaskPriority::High],
        sizes: Vec::new(),
        sort: TaskSort::DueDate,
        descending: true,
    })
}

fn names(db: &DatabaseContext, ws: i64) -> Vec<String> {
    db.send_blocking(ListSavedTaskViewsCommand { workspace_id: ws })
        .unwrap()
        .into_iter()
        .map(|v| v.name)
        .collect()
}

#[test]
fn a_view_keeps_its_scope_tags_and_order_until_updated() {
    let db = context();
    let ws = workspace(&db);
    let core = tag(&db, ws, "Core");
    let plus = tag(&db, ws, "Plus");
    let features = folder(&db, ws);
    let scope = TaskViewScope::Folder {
        task_folder_id: features,
    };

    let view = create(&db, ws, "Core work", scope, filter(vec![core], vec![plus])).unwrap();
    assert_eq!(view.scope, scope);
    assert_eq!(view.tags, filter(vec![core], vec![plus]));
    assert_eq!(
        view.priorities.0,
        [TaskPriority::High, TaskPriority::Urgent]
    );
    assert!(matches!(view.sort, TaskSort::DueDate));
    assert!(view.descending);

    let updated = db
        .send_blocking(UpdateSavedTaskViewCommand {
            workspace_id: ws,
            view_id: view.id,
            scope: INBOX,
            tags: filter(vec![], vec![core]),
            priorities: Vec::new(),
            sizes: Vec::new(),
            sort: TaskSort::Title,
            descending: false,
        })
        .unwrap();
    assert_eq!(updated.name, "Core work");
    assert_eq!(updated.scope, INBOX);
    assert_eq!(updated.tags, filter(vec![], vec![core]));
    assert!(updated.priorities.0.is_empty());
    assert!(matches!(updated.sort, TaskSort::Title));
}

#[test]
fn view_names_are_unique_ignoring_case_and_new_views_go_last() {
    let db = context();
    let ws = workspace(&db);
    create(&db, ws, "Mine", INBOX, TagFilter::default()).unwrap();
    create(&db, ws, "Later", INBOX, TagFilter::default()).unwrap();
    assert!(matches!(
        create(&db, ws, "MINE", INBOX, TagFilter::default()),
        Err(CommandError::Validation(_))
    ));
    assert!(matches!(
        create(&db, ws, "", INBOX, TagFilter::default()),
        Err(CommandError::Validation(_))
    ));
    let later = db
        .send_blocking(ListSavedTaskViewsCommand { workspace_id: ws })
        .unwrap()[1]
        .id;
    assert!(matches!(
        db.send_blocking(RenameSavedTaskViewCommand {
            workspace_id: ws,
            view_id: later,
            name: "mine".to_string(),
        }),
        Err(CommandError::Validation(_))
    ));
    assert_eq!(names(&db, ws), ["Mine", "Later"]);
}

#[test]
fn a_view_narrows_only_by_its_workspaces_tags_and_lists_its_folders() {
    let db = context();
    let ws = workspace(&db);
    let other = workspace(&db);
    let theirs = tag(&db, other, "Core");
    let their_folder = folder(&db, other);

    assert!(create(&db, ws, "a", INBOX, filter(vec![theirs], vec![])).is_err());
    let folder_scope = TaskViewScope::Folder {
        task_folder_id: their_folder,
    };
    assert!(create(&db, ws, "c", folder_scope, TagFilter::default()).is_err());
    assert!(names(&db, ws).is_empty());
}

#[test]
fn a_view_goes_with_its_folder_and_loses_a_deleted_tag() {
    let db = context();
    let ws = workspace(&db);
    let core = tag(&db, ws, "Core");
    let plus = tag(&db, ws, "Plus");
    let features = folder(&db, ws);
    let scope = TaskViewScope::Folder {
        task_folder_id: features,
    };
    create(&db, ws, "In folder", scope, TagFilter::default()).unwrap();
    create(&db, ws, "Tagged", INBOX, filter(vec![core], vec![plus])).unwrap();

    db.send_blocking(DeleteTaskFolderCommand {
        workspace_id: ws,
        folder_id: features,
    })
    .unwrap();
    db.send_blocking(DeleteTagCommand {
        workspace_id: ws,
        tag_id: plus,
    })
    .unwrap();

    let views = db
        .send_blocking(ListSavedTaskViewsCommand { workspace_id: ws })
        .unwrap();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].name, "Tagged");
    assert_eq!(views[0].tags, filter(vec![core], vec![]));
}

#[test]
fn a_view_starts_with_no_symbol_and_takes_one_until_cleared() {
    let db = context();
    let ws = workspace(&db);
    let away = workspace(&db);
    let view = create(&db, ws, "Mine", INBOX, TagFilter::default()).unwrap();
    assert_eq!(view.symbol, None);

    let set = db
        .send_blocking(SetSavedTaskViewSymbolCommand {
            workspace_id: ws,
            view_id: view.id,
            symbol: Some(ListSymbol::Star),
        })
        .unwrap();
    assert_eq!(set.symbol, Some(ListSymbol::Star));
    let listed = db
        .send_blocking(ListSavedTaskViewsCommand { workspace_id: ws })
        .unwrap();
    assert_eq!(listed[0].symbol, Some(ListSymbol::Star));

    let elsewhere = db.send_blocking(SetSavedTaskViewSymbolCommand {
        workspace_id: away,
        view_id: view.id,
        symbol: Some(ListSymbol::Home),
    });
    assert!(matches!(elsewhere, Err(CommandError::NotFound(_))));

    let cleared = db
        .send_blocking(SetSavedTaskViewSymbolCommand {
            workspace_id: ws,
            view_id: view.id,
            symbol: None,
        })
        .unwrap();
    assert_eq!(cleared.symbol, None);
}
