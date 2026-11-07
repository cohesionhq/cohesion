use cohesion_core::model::{
    CatalogModel, Dialect, EndpointBackend, EndpointModel, IntelligenceEndpoint,
};
use cohesion_core::value::Json;
use cohesion_core::{
    Client, ColumnGroup, DayFilter, DayRange, Document, DocumentType, Model, Projection, Task,
    TaskFolder, Workspace,
};

use crate::{Container, DatabaseError, Filtered, Found, ReadableDatabase, WriteContainer};

#[test]
fn roundtrip() {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    let db = WriteContainer::new(&container);

    let created = db
        .insert::<Workspace>()
        .set(Workspace::TITLE, "First")
        .returning_one()
        .unwrap()
        .unwrap();
    assert_eq!(created.title, "First");
    db.insert::<Workspace>()
        .set(Workspace::TITLE, "Second")
        .execute()
        .unwrap();

    let all = db.select::<Workspace>().order("id").all().unwrap();
    assert_eq!(all.len(), 2);
    let ids = db.select_id::<Workspace>().order("id").all().unwrap();
    assert_eq!(ids, all.iter().map(|w| w.id).collect::<Vec<_>>());
    assert_eq!(
        db.select_id::<Workspace>()
            .filter("title = ?")
            .bind("First")
            .one()
            .unwrap(),
        Some(created.id)
    );
    let found = db
        .select::<Workspace>()
        .filter_eq(Workspace::ID, created.id)
        .one()
        .unwrap()
        .unwrap();
    assert_eq!(found.title, "First");
    assert!(
        db.select::<Workspace>()
            .filter_eq(Workspace::ID, 999)
            .one()
            .unwrap()
            .is_none()
    );
    assert!(
        db.select::<Workspace>()
            .filter_eq(Workspace::ID, created.id)
            .exists()
            .unwrap()
    );
    assert!(
        !db.select::<Workspace>()
            .filter_eq(Workspace::ID, 999)
            .exists()
            .unwrap()
    );

    let renamed = db
        .update::<Workspace>()
        .set(Workspace::TITLE, "Renamed")
        .filter_eq(Workspace::ID, created.id)
        .returning_one()
        .unwrap()
        .unwrap();
    assert_eq!(renamed.title, "Renamed");
    let hidden = db
        .update::<Workspace>()
        .set_true(Workspace::DELETED)
        .filter_eq(Workspace::ID, created.id)
        .execute()
        .unwrap();
    assert_eq!(hidden, 1);
    let live = db.select_live::<Workspace>().all().unwrap();
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].title, "Second");

    let journal = db
        .insert::<Document>()
        .set(Document::WORKSPACE_ID, created.id)
        .set(Document::TITLE, "j")
        .set(Document::CONTENTS, "")
        .set(Document::DOCUMENT_TYPE, DocumentType::Journal)
        .set(Document::JOURNAL_DATE, "2026-01-01".to_string())
        .returning_one()
        .unwrap()
        .unwrap();
    assert!(matches!(journal.document_type, DocumentType::Journal));
    assert!(journal.journal_date.is_some());

    let removed = db
        .delete::<Workspace>()
        .filter_eq(Workspace::ID, created.id)
        .returning_one()
        .unwrap()
        .unwrap();
    assert_eq!(removed.id, created.id);
    // ON DELETE CASCADE took the journal with it.
    assert!(db.select::<Document>().all().unwrap().is_empty());
    assert_eq!(db.delete::<Workspace>().execute().unwrap(), 1);
    assert!(db.select::<Workspace>().all().unwrap().is_empty());
}

/// A workspace's title and an expression over it, without the rest.
#[derive(Projection)]
#[projection(of = Workspace)]
struct WorkspaceTitle {
    id: i64,
    title: String,
    #[projection(expr = "length(title)")]
    title_length: i64,
}

#[test]
fn projection_reads_its_columns_and_expressions() {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    let db = WriteContainer::new(&container);
    let kept = db
        .insert::<Workspace>()
        .set(Workspace::TITLE, "Kept")
        .returning_one()
        .unwrap()
        .unwrap();
    db.insert::<Workspace>()
        .set(Workspace::TITLE, "Gone")
        .set_true(Workspace::DELETED)
        .execute()
        .unwrap();

    let live = db.select_live_as::<WorkspaceTitle>().all().unwrap();
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].id, kept.id);
    assert_eq!(live[0].title, "Kept");
    assert_eq!(live[0].title_length, 4);
}

/// A task with its folder, if it is filed in one.
#[derive(Projection)]
#[projection(of = Task)]
#[projection(left_join = TaskFolder, on = "task_folders.id = tasks.task_folder_id")]
struct FiledTask {
    id: i64,
    #[projection(expr = "task_folders.title")]
    folder_title: Option<String>,
    #[projection(from = TaskFolder)]
    sort_order: Option<i64>,
}

/// A task with the title of its workspace; tasks in no workspace drop out.
#[derive(Projection)]
#[projection(of = Task)]
#[projection(join = Workspace, on = "workspaces.id = tasks.workspace_id")]
struct WorkspaceTask {
    id: i64,
    #[projection(expr = "workspaces.title")]
    workspace_title: String,
    #[projection(from = Workspace)]
    emoji: Option<String>,
}

fn migrated() -> Container {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    container
}

fn workspace(db: WriteContainer<'_>, title: &str) -> i64 {
    db.insert::<Workspace>()
        .set(Workspace::TITLE, title)
        .returning_one()
        .unwrap()
        .unwrap()
        .id
}

fn task(db: WriteContainer<'_>, workspace_id: i64, title: &str) -> Task {
    db.insert::<Task>()
        .set(Task::WORKSPACE_ID, workspace_id)
        .set(Task::TITLE, title)
        .returning_one()
        .unwrap()
        .unwrap()
}

#[test]
fn update_live_leaves_soft_deleted_rows_alone() {
    let container = migrated();
    let db = WriteContainer::new(&container);
    let home = workspace(db, "Home");
    let kept = task(db, home, "Dishes");
    let gone = task(db, home, "Laundry");
    db.update::<Task>()
        .set_true(Task::DELETED)
        .filter_eq(Task::ID, gone.id)
        .execute()
        .unwrap();

    let renamed = db
        .update_live::<Task>()
        .set(Task::TITLE, "Renamed")
        .filter_eq(Task::WORKSPACE_ID, home)
        .returning_all()
        .unwrap();
    assert_eq!(renamed.len(), 1);
    assert_eq!(renamed[0].id, kept.id);
    let untouched = db
        .select::<Task>()
        .filter_eq(Task::ID, gone.id)
        .one()
        .found()
        .unwrap();
    assert_eq!(untouched.title, "Laundry");
}

#[test]
fn projection_joins_qualify_columns() {
    let container = migrated();
    let db = WriteContainer::new(&container);
    let home = workspace(db, "Home");
    let work = workspace(db, "Work");
    let errands = db
        .insert::<TaskFolder>()
        .set(TaskFolder::WORKSPACE_ID, home)
        .set(TaskFolder::TITLE, "Errands")
        .set(TaskFolder::SORT_ORDER, 7)
        .returning_one()
        .unwrap()
        .unwrap();
    let filed = task(db, home, "Buy milk");
    db.update::<Task>()
        .set(Task::TASK_FOLDER_ID, errands.id)
        .filter_eq(Task::ID, filed.id)
        .execute()
        .unwrap();
    let loose = task(db, home, "Call mum");
    let hidden = task(db, home, "Hidden");
    db.update::<Task>()
        .set_true(Task::DELETED)
        .filter_eq(Task::ID, hidden.id)
        .execute()
        .unwrap();
    task(db, work, "Ship it");

    // `id` and `deleted` exist in both tables; the builder qualifies its own.
    let rows = db
        .select_live_as::<FiledTask>()
        .filter_eq(Task::WORKSPACE_ID, home)
        .order("tasks.id")
        .all()
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, filed.id);
    assert_eq!(rows[0].folder_title.as_deref(), Some("Errands"));
    assert_eq!(rows[0].sort_order, Some(7));
    assert_eq!(rows[1].id, loose.id);
    assert_eq!(rows[1].folder_title, None);
    assert_eq!(rows[1].sort_order, None);

    // An inner join reads the other table's columns, and drops rows without one.
    container
        .connection()
        .execute("DELETE FROM versions WHERE task_id = ?", [loose.id])
        .unwrap();
    container
        .connection()
        .execute(
            "UPDATE tasks SET workspace_id = NULL WHERE id = ?",
            [loose.id],
        )
        .unwrap();
    let titled = db
        .select_as::<WorkspaceTask>()
        .filter("tasks.id IN (?, ?)")
        .bind(filed.id)
        .bind(loose.id)
        .all()
        .unwrap();
    assert_eq!(titled.len(), 1);
    assert_eq!(titled[0].id, filed.id);
    assert_eq!(titled[0].workspace_title, "Home");
    assert_eq!(titled[0].emoji, None);
}

#[test]
fn subqueries_and_value_lists_narrow_selects_updates_and_deletes() {
    let container = migrated();
    let db = WriteContainer::new(&container);
    let busy = workspace(db, "Busy");
    let quiet = workspace(db, "Quiet");
    let first = task(db, busy, "One");
    task(db, busy, "Two");

    let with_tasks = db
        .select::<Workspace>()
        .filter_in("id", db.select_column::<Task, i64>("workspace_id"))
        .all()
        .unwrap();
    assert_eq!(with_tasks.len(), 1);
    assert_eq!(with_tasks[0].id, busy);
    // A subquery's own parameters line up with the ones bound after it.
    let without = db
        .select::<Workspace>()
        .filter_not_in(
            "id",
            db.select_column::<Task, i64>("workspace_id")
                .filter("title = ?")
                .bind("One"),
        )
        .filter("title <> ?")
        .bind("Busy")
        .all()
        .unwrap();
    assert_eq!(without.len(), 1);
    assert_eq!(without[0].id, quiet);

    let workspaces = db
        .select_column::<Task, i64>("workspace_id")
        .distinct()
        .all()
        .unwrap();
    assert_eq!(workspaces, vec![busy]);

    assert_eq!(
        db.select::<Task>()
            .filter_in_values("id", [first.id, 9999])
            .all()
            .unwrap()
            .len(),
        1
    );
    assert!(
        db.select::<Task>()
            .filter_in_values("id", Vec::<i64>::new())
            .all()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        db.select::<Task>()
            .filter_not_in_values("id", Vec::<i64>::new())
            .all()
            .unwrap()
            .len(),
        2
    );

    assert_eq!(
        db.update::<Task>()
            .set(Task::TITLE, "Renamed")
            .filter_in_values("id", [first.id])
            .execute()
            .unwrap(),
        1
    );
    assert_eq!(
        db.delete::<Task>()
            .filter_not_in_values("id", [first.id])
            .execute()
            .unwrap(),
        1
    );
    let left = db.select::<Task>().all().unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].title, "Renamed");
}

#[test]
fn days_filter_by_day_days_and_range() {
    let container = migrated();
    let db = WriteContainer::new(&container);
    let home = workspace(db, "Home");
    for (title, due) in [
        ("a", "2026-10-01"),
        ("b", "2026-10-15"),
        ("c", "2026-10-30"),
    ] {
        db.insert::<Task>()
            .set(Task::WORKSPACE_ID, home)
            .set(Task::TITLE, title)
            .set(Task::DUE_DATE, due.to_string())
            .execute()
            .unwrap();
    }
    let titles = |days: DayFilter| -> Vec<String> {
        db.select::<Task>()
            .filter_day("due_date", days)
            .order("title")
            .all()
            .unwrap()
            .into_iter()
            .map(|task| task.title)
            .collect()
    };
    assert_eq!(titles(DayFilter::Day("2026-10-15".into())), ["b"]);
    assert_eq!(
        titles(DayFilter::Days(vec![
            "2026-10-01".into(),
            "2026-10-30".into()
        ])),
        ["a", "c"]
    );
    assert_eq!(
        titles(DayFilter::Range(DayRange {
            before: Some("2026-10-30".into()),
            after: Some("2026-10-01".into()),
        })),
        ["b"]
    );
}

#[test]
fn sort_positions_and_appended_text_are_set_by_the_builder() {
    let container = migrated();
    let db = WriteContainer::new(&container);
    let home = workspace(db, "Home");
    let work = workspace(db, "Work");
    let folder = |workspace_id: i64| {
        db.insert::<TaskFolder>()
            .set(TaskFolder::WORKSPACE_ID, workspace_id)
            .set(TaskFolder::TITLE, "f")
            .set_max_incremented_within(
                TaskFolder::SORT_ORDER,
                TaskFolder::WORKSPACE_ID,
                workspace_id,
            )
            .returning_one()
            .unwrap()
            .unwrap()
            .sort_order
    };
    assert_eq!([folder(home), folder(home), folder(work)], [1, 2, 1]);

    let note = task(db, home, "Note");
    let append = |line: bool, text: &str| {
        let update = db.update::<Task>();
        let update = if line {
            update.append_line("description", text)
        } else {
            update.append("description", text)
        };
        update
            .filter("id = ?")
            .bind(note.id)
            .returning_one()
            .unwrap()
            .unwrap()
            .description
    };
    assert_eq!(append(true, "one"), "one");
    assert_eq!(append(true, "two"), "one\ntwo");
    assert_eq!(append(false, "!"), "one\ntwo!");
}

/// How a scratch endpoint is served: either over HTTP or by a client.
#[derive(Debug, PartialEq, ColumnGroup)]
enum Server {
    Http {
        backend: String,
        base_url: String,
        api_key: Option<String>,
    },
    Client {
        client_id: i64,
        offer: String,
    },
}

#[derive(Debug, Model)]
#[model(table = "scratch_endpoints", backup = Included)]
struct ScratchEndpoint {
    id: i64,
    #[allow(dead_code)]
    name: String,
    #[model(group)]
    server: Server,
    #[model(json)]
    tags: Vec<String>,
}

fn scratch() -> Container {
    let container = migrated();
    container
        .connection()
        .execute_batch(
            "CREATE TABLE scratch_endpoints (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                backend TEXT, base_url TEXT, api_key TEXT,
                client_id INTEGER, offer TEXT,
                tags TEXT NOT NULL DEFAULT '[]'
            )",
        )
        .unwrap();
    container
}

#[test]
fn a_column_group_is_one_field_read_and_written_as_all_its_columns() {
    let container = scratch();
    let db = WriteContainer::new(&container);
    let created = db
        .insert::<ScratchEndpoint>()
        .set(ScratchEndpoint::NAME, "Local")
        .set_group(Server::Http {
            backend: "messages".into(),
            base_url: "http://localhost".into(),
            api_key: None,
        })
        .set(
            ScratchEndpoint::TAGS,
            Json(vec!["fast".to_string(), "cheap".to_string()]),
        )
        .returning_one()
        .unwrap()
        .unwrap();
    assert_eq!(
        created.server,
        Server::Http {
            backend: "messages".into(),
            base_url: "http://localhost".into(),
            api_key: None
        }
    );
    assert_eq!(created.tags, ["fast", "cheap"]);

    // Switching variant clears the columns the new one doesn't have.
    let switched = db
        .update::<ScratchEndpoint>()
        .set_group(Server::Client {
            client_id: 3,
            offer: "apple".into(),
        })
        .filter("id = ?")
        .bind(created.id)
        .returning_one()
        .unwrap()
        .unwrap();
    assert_eq!(
        switched.server,
        Server::Client {
            client_id: 3,
            offer: "apple".into()
        }
    );
    let backends = db
        .select_column::<ScratchEndpoint, Option<String>>("backend")
        .all()
        .unwrap();
    assert_eq!(backends, [None]);
    // The group's columns are columns of the model, for filters.
    assert!(
        db.select::<ScratchEndpoint>()
            .filter("client_id = ?")
            .bind(3)
            .exists()
            .unwrap()
    );
}

#[test]
fn a_row_fitting_no_variant_or_a_bad_json_column_is_an_error_not_a_panic() {
    let container = scratch();
    container
        .connection()
        .execute_batch(
            "INSERT INTO scratch_endpoints (id, name) VALUES (1, 'neither set');
             INSERT INTO scratch_endpoints (id, name, backend, base_url, client_id, offer)
                 VALUES (2, 'both set', 'messages', 'http://x', 3, 'apple');
             INSERT INTO scratch_endpoints (id, name, backend) VALUES (3, 'half of http', 'messages');
             INSERT INTO scratch_endpoints (id, name, backend, base_url, tags)
                 VALUES (4, 'bad json', 'messages', 'http://x', '{not json');
             INSERT INTO scratch_endpoints (id, name, backend, base_url)
                 VALUES (5, 'fine', 'messages', 'http://x');",
        )
        .unwrap();
    let db = WriteContainer::new(&container);
    for id in 1..=4 {
        let error = db
            .select::<ScratchEndpoint>()
            .filter("id = ?")
            .bind(id)
            .one()
            .unwrap_err();
        assert!(
            matches!(error, DatabaseError::Row(_)),
            "row {id} should be unreadable, got {error}"
        );
    }
    let fine = db
        .select::<ScratchEndpoint>()
        .filter("id = ?")
        .bind(5)
        .one()
        .found()
        .unwrap();
    assert!(fine.tags.is_empty());
}

/// What serves a scratch account, shaped like `AccountHost`: one of an
/// extension or a client and its offer.
#[derive(Debug, PartialEq, ColumnGroup)]
enum AccountServerColumns {
    Extension { extension_id: i64 },
    Client { client_id: i64, offer: String },
}

#[derive(Debug, Model)]
#[model(table = "scratch_accounts", backup = Included)]
struct ScratchAccount {
    id: i64,
    #[model(group)]
    server: AccountServerColumns,
}

#[test]
fn a_column_group_can_stand_for_an_accounts_server() {
    let container = migrated();
    container
        .connection()
        .execute_batch(
            "CREATE TABLE scratch_accounts (
                id INTEGER PRIMARY KEY,
                extension_id INTEGER, client_id INTEGER, offer TEXT,
                CHECK ((extension_id IS NULL) = (client_id IS NOT NULL)
                    AND (client_id IS NOT NULL) = (offer IS NOT NULL))
            )",
        )
        .unwrap();
    let db = WriteContainer::new(&container);
    db.insert::<ScratchAccount>()
        .set_group(AccountServerColumns::Extension { extension_id: 4 })
        .execute()
        .unwrap();
    db.insert::<ScratchAccount>()
        .set_group(AccountServerColumns::Client {
            client_id: 2,
            offer: "notes".into(),
        })
        .execute()
        .unwrap();
    let servers: Vec<_> = db
        .select::<ScratchAccount>()
        .order("id")
        .all()
        .unwrap()
        .into_iter()
        .map(|account| account.server)
        .collect();
    assert_eq!(
        servers,
        [
            AccountServerColumns::Extension { extension_id: 4 },
            AccountServerColumns::Client {
                client_id: 2,
                offer: "notes".into()
            }
        ]
    );
}

/// A scratch endpoint's name as a reader sees it: its own, else its client's
/// and the offer's title.
const SCRATCH_DISPLAY_NAME: &str = "COALESCE(scratch_clients_endpoints.name, clients.client_name || ' · ' || scratch_clients_endpoints.offer)";

/// `ScratchEndpoint`'s reads: the display name joined from `clients`, and the
/// server group read from its columns.
#[derive(Debug, Projection)]
#[projection(of = ScratchClientEndpoint)]
#[projection(left_join = Client, on = "clients.id = scratch_clients_endpoints.client_id")]
struct ScratchEndpointListing {
    id: i64,
    #[projection(expr = SCRATCH_DISPLAY_NAME)]
    name: String,
    #[projection(group)]
    server: Server,
}

#[derive(Debug, Model)]
#[model(table = "scratch_clients_endpoints", backup = Included)]
#[allow(dead_code)]
struct ScratchClientEndpoint {
    id: i64,
    name: Option<String>,
    #[model(group)]
    server: Server,
}

#[test]
fn a_projection_reads_a_column_group_and_a_name_joined_from_another_table() {
    let container = migrated();
    container
        .connection()
        .execute_batch(
            "CREATE TABLE scratch_clients_endpoints (
                id INTEGER PRIMARY KEY,
                name TEXT,
                backend TEXT, base_url TEXT, api_key TEXT,
                client_id INTEGER, offer TEXT
            );
            INSERT INTO clients (id, key, client_name) VALUES (1, 'k', 'Mac mini');",
        )
        .unwrap();
    let db = WriteContainer::new(&container);
    db.insert::<ScratchClientEndpoint>()
        .set(ScratchClientEndpoint::NAME, "Anthropic".to_string())
        .set_group(Server::Http {
            backend: "messages".into(),
            base_url: "http://x".into(),
            api_key: None,
        })
        .execute()
        .unwrap();
    db.insert::<ScratchClientEndpoint>()
        .set_group(Server::Client {
            client_id: 1,
            offer: "Apple Intelligence".into(),
        })
        .execute()
        .unwrap();
    let listed = db
        .select_as::<ScratchEndpointListing>()
        .order("scratch_clients_endpoints.id")
        .all()
        .unwrap();
    assert_eq!(listed[0].name, "Anthropic");
    assert_eq!(listed[1].name, "Mac mini · Apple Intelligence");
    assert_eq!(
        listed[1].server,
        Server::Client {
            client_id: 1,
            offer: "Apple Intelligence".into()
        }
    );
    // The constant is the same expression, so a filter can match the name.
    let found = db
        .select_as::<ScratchEndpointListing>()
        .filter(&format!("{SCRATCH_DISPLAY_NAME} = ?"))
        .bind("Mac mini · Apple Intelligence")
        .one()
        .found()
        .unwrap();
    assert_eq!(found.id, listed[1].id);
}

/// A task read with its workspace's title in front of its own: a model that
/// joins (inner) and is soft-deletable.
#[derive(Debug, Model)]
#[model(
    table = "tasks",
    backup = Included,
    join = Workspace,
    on = "workspaces.id = tasks.workspace_id"
)]
struct TaskInWorkspace {
    id: i64,
    workspace_id: i64,
    #[model(expr = "workspaces.title || ' / ' || tasks.title")]
    title: String,
    deleted: bool,
}

#[test]
fn a_joined_model_is_read_whole_by_every_select() {
    let container = migrated();
    let db = WriteContainer::new(&container);
    let home = workspace(db, "Home");
    let kept = task(db, home, "Dishes");
    let gone = task(db, home, "Laundry");
    db.update::<Task>()
        .set_true(Task::DELETED)
        .filter_eq(Task::ID, gone.id)
        .execute()
        .unwrap();

    let all = db
        .select::<TaskInWorkspace>()
        .order("tasks.id")
        .all()
        .unwrap();
    assert_eq!(
        all.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(),
        ["Home / Dishes", "Home / Laundry"]
    );
    let live = db.select_live::<TaskInWorkspace>().all().unwrap();
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].title, "Home / Dishes");

    let one = db
        .select_live::<TaskInWorkspace>()
        .filter("tasks.id = ?")
        .bind(kept.id)
        .one()
        .unwrap()
        .unwrap();
    assert_eq!(one.title, "Home / Dishes");
    let required = db
        .select::<TaskInWorkspace>()
        .filter("tasks.id = ?")
        .bind(gone.id)
        .one()
        .found()
        .unwrap();
    assert!(required.deleted);
    let found = db
        .select::<TaskInWorkspace>()
        .filter("tasks.id = ?")
        .bind(kept.id)
        .one()
        .found()
        .unwrap();
    assert_eq!(found.workspace_id, home);

    // A filter may name the joined table, and `exists`, a column select and
    // a subquery all see the join.
    assert!(
        db.select::<TaskInWorkspace>()
            .filter("workspaces.title = ?")
            .bind("Home")
            .exists()
            .unwrap()
    );
    assert!(
        !db.select_live::<TaskInWorkspace>()
            .filter("workspaces.title = ?")
            .bind("Away")
            .exists()
            .unwrap()
    );
    let in_home = || {
        db.select_id::<TaskInWorkspace>()
            .filter("workspaces.title = ?")
            .bind("Home")
    };
    assert_eq!(in_home().all().unwrap().len(), 2);
    assert_eq!(
        db.select::<Task>()
            .filter_in("id", in_home())
            .all()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn a_write_reads_the_row_back_through_the_models_joins() {
    let container = migrated();
    let db = WriteContainer::new(&container);
    let home = workspace(db, "Home");

    // Inserting stores the plain `title`; the returned row has it joined.
    let inserted = db
        .insert::<TaskInWorkspace>()
        .set(TaskInWorkspace::WORKSPACE_ID, home)
        .set(TaskInWorkspace::TITLE, "Dishes".to_string())
        .returning_one()
        .found()
        .unwrap();
    assert_eq!(inserted.title, "Home / Dishes");
    let stored: String = container
        .connection()
        .query_row("SELECT title FROM tasks", [], |row| row.get(0))
        .unwrap();
    assert_eq!(stored, "Dishes");

    let updated = db
        .update::<TaskInWorkspace>()
        .set(TaskInWorkspace::TITLE, "Cooking".to_string())
        .filter("id = ?")
        .bind(inserted.id)
        .returning_one()
        .found()
        .unwrap();
    assert_eq!(updated.title, "Home / Cooking");
    db.insert::<TaskInWorkspace>()
        .set(TaskInWorkspace::WORKSPACE_ID, home)
        .set(TaskInWorkspace::TITLE, "Laundry".to_string())
        .execute()
        .unwrap();
    let all = db
        .update::<TaskInWorkspace>()
        .set_false(TaskInWorkspace::DELETED)
        .returning_all()
        .unwrap();
    assert_eq!(
        all.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(),
        ["Home / Cooking", "Home / Laundry"]
    );
    // Nothing matched: nothing returned.
    assert!(
        db.update::<TaskInWorkspace>()
            .set_false(TaskInWorkspace::DELETED)
            .filter("id = ?")
            .bind(0)
            .returning_one()
            .unwrap()
            .is_none()
    );
}

#[test]
fn a_derived_read_adds_no_column_a_write_could_name() {
    // `title` is read from an expression but stored as a column; nothing
    // else is a column of the model, so nothing else has a `Column` to set.
    assert_eq!(
        TaskInWorkspace::columns(),
        ["id", "workspace_id", "title", "deleted"]
    );
}

#[test]
fn a_field_read_from_a_joined_model_when_null_is_filled_on_every_read() {
    let container = migrated();
    let db = WriteContainer::new(&container);
    let catalog = db
        .insert::<CatalogModel>()
        .set(CatalogModel::PROVIDER_IDENTIFIER, "anthropic")
        .set(CatalogModel::PROVIDER_NAME, "Anthropic")
        .set(CatalogModel::MODEL_IDENTIFIER, "claude")
        .set(CatalogModel::MODEL_NAME, "Claude")
        .set(CatalogModel::CONTEXT_SIZE, 200_000)
        .returning_one()
        .found()
        .unwrap();
    let endpoint = db
        .insert::<IntelligenceEndpoint>()
        .set(IntelligenceEndpoint::NAME, "Anthropic".to_string())
        .set_group(EndpointBackend::Integrated {
            provider_identifier: "anthropic".into(),
            dialect: Dialect::Messages,
            base_url: "http://x".into(),
            api_key: None,
            usage_url: None,
            cache_minutes: 5,
            long_cache: None,
        })
        .returning_one()
        .found()
        .unwrap();
    let linked = db
        .insert::<EndpointModel>()
        .set(EndpointModel::ENDPOINT_ID, endpoint.id)
        .set(EndpointModel::MODEL_IDENTIFIER, "claude")
        .set(EndpointModel::CATALOG_MODEL_ID, catalog.id)
        .returning_one()
        .found()
        .unwrap();
    assert_eq!(linked.context_size, Some(200_000));
    assert_eq!(linked.model_name, "Claude");
    // A value on the row wins; the read of it is the same either way.
    let overridden = db
        .update::<EndpointModel>()
        .set(EndpointModel::CONTEXT_SIZE, 1_000)
        .filter_eq(EndpointModel::ID, linked.id)
        .returning_one()
        .found()
        .unwrap();
    assert_eq!(overridden.context_size, Some(1_000));
    let selected = db
        .select::<EndpointModel>()
        .filter_eq(EndpointModel::ID, linked.id)
        .one()
        .found()
        .unwrap();
    assert_eq!(selected.context_size, Some(1_000));
    assert_eq!(selected.model_name, "Claude");
    // With neither its own name nor a catalog row, the last term names it.
    let unlisted = db
        .insert::<EndpointModel>()
        .set(EndpointModel::ENDPOINT_ID, endpoint.id)
        .set(EndpointModel::MODEL_IDENTIFIER, "claude-next")
        .returning_one()
        .found()
        .unwrap();
    assert_eq!(unlisted.model_name, "claude-next");
}
