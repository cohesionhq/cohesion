use cohesion_core::{
    Client, Document, DocumentFolder, Model, Task, TaskFolder, TaskPriority, Workspace,
    model::{
        AccountHost, Activity, ActivityStatus, Actor, ActorMessageContent, ActorPermissionDecision,
        ActorSession, ActorSessionMessage, ActorSessionMessageContent, ActorSessionStatus,
        ActorToolKind, ActorUserContent, AgentAgendaItem, AgentCodeOutcome, AgentMessageContent,
        AgentSkill, AgentThreadMark, Article, Attachment, Backlink, CatalogModel, Connector,
        ConnectorAccount, Contact, ContactMethod, DocumentFolderProperty, DocumentValue,
        EndpointModel, Extension, ExtensionHook, ExtensionPermission, ExtensionService,
        ExtensionServiceFunction, Feed, FeedFullPagePolicy, FeedStatus, HookKind,
        IntelligenceAgent, IntelligenceAgentCue, IntelligenceAgentMessage, IntelligenceAgentRun,
        IntelligenceAgentTeam, IntelligenceCostEvent, IntelligenceDefault, IntelligenceEffort,
        IntelligenceEndpoint, IntelligenceSpendLimit, IntelligenceThread, IntelligenceThreadImage,
        IntelligenceThreadMessage, LinkKind, Memory, MemoryScope, Metadata, PairingOpener, Photo,
        Profile, PropertyOption, PublishedPrecision, Resource, ResourceKind, RewindWriter,
        SavedTaskView, SavedTaskViewPriority, SavedTaskViewSize, SavedTaskViewTag, SecurityEvent,
        SecurityEventKind, SecuritySettings, StorageEntry, StorageSettings, Tag, TaskTag,
        ThreadItem, ThreadOrigin, ThreadPrompt, Version, VersionedItem, WorkspaceResourceKind,
    },
    value::IntoValue,
};

use super::{CASCADING_MIGRATIONS, MIGRATIONS};
use crate::db_macros::every_model;
use crate::{Container, DatabaseError, Filtered, ReadableDatabase, WriteContainer};

fn user_version(db: &Container) -> i64 {
    db.connection()
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap()
}

fn migrated() -> Container {
    let mut db = Container::in_memory().unwrap();
    db.migrate().unwrap();
    db
}

fn foreign_keys_enabled(db: &Container) -> bool {
    db.connection()
        .pragma_query_value(None, "foreign_keys", |row| row.get(0))
        .unwrap()
}

#[test]
fn migrates_fresh_database_idempotently() {
    let mut db = migrated();
    assert_eq!(user_version(&db), MIGRATIONS.len() as i64);
    db.migrate().unwrap();
    assert_eq!(user_version(&db), MIGRATIONS.len() as i64);
    assert!(foreign_keys_enabled(&db));
}

#[test]
fn refuses_a_database_with_more_migrations_than_it_knows() {
    let mut db = migrated();
    db.connection()
        .pragma_update(None, "user_version", MIGRATIONS.len() as i64 + 1)
        .unwrap();
    assert!(matches!(
        db.migrate(),
        Err(DatabaseError::UnknownSchema { .. })
    ));
}

/// `(entity_kind, entity_id)` of every indexed row matching `query`.
fn search(db: &Container, query: &str) -> Vec<(String, i64)> {
    let mut statement = db
        .connection()
        .prepare(
            "SELECT e.entity_kind, e.entity_id FROM search_fts
             JOIN search_entries e ON e.id = search_fts.rowid
             WHERE search_fts MATCH ? ORDER BY e.entity_kind, e.entity_id",
        )
        .unwrap();
    statement
        .query_map([query], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn search_index_follows_documents_and_tasks() {
    let db = migrated();
    db.connection()
        .execute_batch(
            "INSERT INTO workspaces (id, title) VALUES (1, 'w');
             INSERT INTO documents (id, workspace_id, title, contents) VALUES (1, 1, 'Plans', 'apples');
             INSERT INTO tasks (id, workspace_id, title, status) VALUES (1, 1, 'Buy apples', 'incomplete');
             INSERT INTO tasks (id, workspace_id, title, status, deleted) VALUES (2, 1, 'Old apples', 'incomplete', TRUE);",
        )
        .unwrap();
    let both = vec![("document".to_string(), 1), ("task".to_string(), 1)];
    assert_eq!(search(&db, "apple"), both);

    db.connection()
        .execute_batch("UPDATE documents SET contents = 'pears' WHERE id = 1")
        .unwrap();
    assert_eq!(search(&db, "apples"), [("task".to_string(), 1)]);
    assert_eq!(search(&db, "pears"), [("document".to_string(), 1)]);

    db.connection()
        .execute_batch("UPDATE tasks SET description = 'from the orchard' WHERE id = 1")
        .unwrap();
    assert_eq!(search(&db, "orchard"), [("task".to_string(), 1)]);
    assert_eq!(search(&db, "apples"), [("task".to_string(), 1)]);
    db.connection()
        .execute_batch(
            "INSERT INTO tasks (id, workspace_id, title, description) VALUES (3, 1, 'Jam', 'orchard plums')",
        )
        .unwrap();
    assert_eq!(
        search(&db, "orchard"),
        [("task".to_string(), 1), ("task".to_string(), 3)]
    );
    db.connection()
        .execute_batch("DELETE FROM tasks WHERE id = 3")
        .unwrap();

    db.connection()
        .execute_batch("UPDATE tasks SET deleted = TRUE WHERE id = 1")
        .unwrap();
    assert!(search(&db, "apples").is_empty());
    assert!(search(&db, "orchard").is_empty());

    db.connection()
        .execute_batch("DELETE FROM workspaces WHERE id = 1")
        .unwrap();
    let remaining: i64 = db
        .connection()
        .query_row(
            "SELECT (SELECT count(*) FROM search_entries) + (SELECT count(*) FROM search_fts)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(remaining, 0);
}

#[test]
fn search_index_follows_resources() {
    let db = migrated();
    db.connection()
        .execute_batch(
            "INSERT INTO workspaces (id, title) VALUES (1, 'w');
             INSERT INTO extensions (id, name, version, script, script_identifier)
                 VALUES (1, 'c', '1', '', 'c');
             INSERT INTO connectors (id, identifier, version, title, unit) VALUES (1, 'c', 1, 's', 'u');
             INSERT INTO connector_accounts (id, connector_id, extension_id, identifier,
                 description, authorization)
                 VALUES (1, 1, 1, 'a', 'd', 't');
             INSERT INTO resource_kinds (id, connector_id, identifier, title, plural, contents)
                 VALUES (1, 1, 'page', 'Page', 'Pages', '');
             INSERT INTO resources (id, workspace_id, account_id, kind_id, identifier,
                 title, content_kind, contents, last_updated) VALUES
                 (1, 1, 1, 1, 'p1', 'Roadmap', 'markdown', 'apples', 1),
                 (2, 1, 1, 1, 'd1', 'Budget', 'json', '{\"apples\": 1}', 1),
                 (3, 1, 1, 1, 't1', 'Memo', 'text', 'plums', 1);",
        )
        .unwrap();
    let resource = |id| ("resource".to_string(), id);
    assert_eq!(search(&db, "roadmap"), [resource(1)]);
    assert_eq!(search(&db, "budget"), [resource(2)]);
    assert_eq!(search(&db, "apples"), [resource(1)]);
    assert_eq!(search(&db, "plums"), [resource(3)]);

    db.connection()
        .execute_batch("UPDATE resources SET contents = 'pears' WHERE id = 1")
        .unwrap();
    assert!(search(&db, "apples").is_empty());
    assert_eq!(search(&db, "pears"), [resource(1)]);

    db.connection()
        .execute_batch("DELETE FROM resources WHERE id = 1")
        .unwrap();
    assert!(search(&db, "pears").is_empty());

    db.connection()
        .execute_batch("DELETE FROM workspaces WHERE id = 1")
        .unwrap();
    let remaining: i64 = db
        .connection()
        .query_row(
            "SELECT (SELECT count(*) FROM search_entries) + (SELECT count(*) FROM search_fts)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(remaining, 0);
}

#[test]
fn search_index_follows_contacts() {
    let db = migrated();
    db.connection()
        .execute_batch(
            "INSERT INTO workspaces (id, title) VALUES (1, 'w');
             INSERT INTO contacts (id, workspace_id, first_name, middle_name, last_name, notes)
                 VALUES (1, 1, 'Ada', 'King', 'Lovelace', 'engines'),
                        (2, 1, 'Grace', NULL, 'Hopper', 'compilers');
             INSERT INTO contacts (id, workspace_id, first_name, last_name, deleted)
                 VALUES (3, 1, 'Old', 'Friend', TRUE);",
        )
        .unwrap();
    let contact = |id| ("contact".to_string(), id);
    assert_eq!(search(&db, "king"), [contact(1)]);
    assert_eq!(search(&db, "hopper"), [contact(2)]);
    assert_eq!(search(&db, "compilers"), [contact(2)]);
    assert!(search(&db, "friend").is_empty());
    let title: String = db
        .connection()
        .query_row(
            "SELECT search_fts.title FROM search_fts
             JOIN search_entries e ON e.id = search_fts.rowid
             WHERE e.entity_kind = 'contact' AND e.entity_id = 2",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(title, "Grace Hopper");

    db.connection()
        .execute_batch("UPDATE contacts SET last_name = 'Byron', notes = 'poems' WHERE id = 1")
        .unwrap();
    assert!(search(&db, "lovelace").is_empty());
    assert!(search(&db, "engines").is_empty());
    assert_eq!(search(&db, "byron poems"), [contact(1)]);

    db.connection()
        .execute_batch("UPDATE contacts SET deleted = TRUE WHERE id = 2")
        .unwrap();
    assert!(search(&db, "hopper").is_empty());

    db.connection()
        .execute_batch("DELETE FROM workspaces WHERE id = 1")
        .unwrap();
    let remaining: i64 = db
        .connection()
        .query_row(
            "SELECT (SELECT count(*) FROM search_entries) + (SELECT count(*) FROM search_fts)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(remaining, 0);
}

#[test]
fn only_a_captured_article_has_contents() {
    let db = migrated();
    db.connection()
        .execute_batch(
            "INSERT INTO feeds (id, url, title, status_state, status_at, added)
                 VALUES (1, 'f', 'f', 'refreshed', 1, 0);",
        )
        .unwrap();
    let failed = |contents: &str| {
        db.connection().execute(
            "INSERT INTO articles (feed_id, url, title, snapshot_state, snapshot_at, failure_kind,
                 contents)
             VALUES (1, ?1, 't', 'failed', 1, 'tooLarge', ?2)",
            (format!("https://{contents}.example"), contents),
        )
    };
    assert!(failed("text").is_err());
    failed("").unwrap();
    db.connection()
        .execute(
            "INSERT INTO articles (feed_id, url, title, snapshot_state, snapshot_at, final_url,
                 truncated, contents)
             VALUES (1, 'https://c.example', 't', 'captured', 1, 'u', 0, 'text')",
            [],
        )
        .unwrap();
}

#[test]
fn hard_deletes_remove_metadata() {
    let db = migrated();
    db.connection()
        .execute_batch(
            "INSERT INTO workspaces (id, title) VALUES (1, 'Home'), (2, 'Work');
             INSERT INTO tasks (id, workspace_id, title) VALUES (1, 1, 'Apples'), (2, 2, 'Pears');
             INSERT INTO task_folders (id, workspace_id, title) VALUES (1, 2, 'Folder');
             INSERT INTO metadata (workspace_id, key, value) VALUES (1, 'k', 'v');
             INSERT INTO metadata (task_id, key, value) VALUES (1, 'k', 'v'), (2, 'k', 'v');
             INSERT INTO metadata (task_folder_id, key, value) VALUES (1, 'k', 'v');
             DELETE FROM workspaces WHERE id = 1;",
        )
        .unwrap();
    let mut statement = db
        .connection()
        .prepare("SELECT task_id, task_folder_id FROM metadata ORDER BY id")
        .unwrap();
    let remaining: Vec<(Option<i64>, Option<i64>)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    // The workspace and its task lose theirs; the unrelated rows stay.
    assert_eq!(remaining, [(Some(2), None), (None, Some(1))]);
    // A folder's pairs cascade too, which no trigger ever did.
    db.connection()
        .execute("DELETE FROM task_folders WHERE id = 1", [])
        .unwrap();
    let left: i64 = db
        .connection()
        .query_row("SELECT COUNT(*) FROM metadata", [], |row| row.get(0))
        .unwrap();
    assert_eq!(left, 1);
}

#[test]
fn task_priority_is_stored_as_its_number() {
    let db = migrated();
    db.connection()
        .execute_batch(
            "INSERT INTO workspaces (id, title) VALUES (1, 'w');
             INSERT INTO tasks (id, workspace_id, title) VALUES (1, 1, 'Default');
             INSERT INTO tasks (id, workspace_id, title, priority) VALUES (2, 1, 'High', 3), (3, 1, 'Unknown', 9);",
        )
        .unwrap();
    let write = WriteContainer::new(&db);
    write
        .update::<Task>()
        .set(Task::PRIORITY, TaskPriority::Urgent)
        .filter_eq(Task::ID, 1)
        .execute()
        .unwrap();
    let stored: Vec<i64> = db
        .connection()
        .prepare("SELECT priority FROM tasks ORDER BY id")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(stored, [4, 3, 9]);

    // An unknown number reads as the default rather than failing the row.
    let read: Vec<TaskPriority> = write
        .select::<Task>()
        .all()
        .unwrap()
        .into_iter()
        .map(|task| task.priority)
        .collect();
    assert_eq!(
        read,
        [TaskPriority::Urgent, TaskPriority::High, TaskPriority::None]
    );
}

#[test]
fn task_size_is_one_to_five_or_none() {
    let db = migrated();
    db.connection()
        .execute_batch(
            "INSERT INTO workspaces (id, title) VALUES (1, 'w');
             INSERT INTO tasks (id, workspace_id, title, size) VALUES (1, 1, 'a', 1), (2, 1, 'b', 5), (3, 1, 'c', NULL);",
        )
        .unwrap();
    for size in [0, 6] {
        let refused = db.connection().execute(
            "INSERT INTO tasks (workspace_id, title, size) VALUES (1, 'x', ?1)",
            [size],
        );
        assert!(refused.is_err(), "size {size} was stored");
    }
}

#[test]
fn a_task_is_never_assigned_to_both_the_user_and_an_agent() {
    let db = migrated();
    db.connection()
        .execute_batch(
            "INSERT INTO workspaces (id, title) VALUES (1, 'w');
             INSERT INTO intelligence_agents (id, agent_name, agent_color) VALUES (1, 'Ada', 'blue');
             INSERT INTO tasks (id, workspace_id, title, assigned_to_user, assignee_agent_id)
                 VALUES (1, 1, 'mine', TRUE, NULL), (2, 1, 'hers', FALSE, 1), (3, 1, 'none', FALSE, NULL);",
        )
        .unwrap();
    let both = db.connection().execute(
        "INSERT INTO tasks (workspace_id, title, assigned_to_user, assignee_agent_id) VALUES (1, 'x', TRUE, 1)",
        [],
    );
    assert!(both.is_err(), "a task was assigned to both");
}

#[test]
fn a_task_assigned_to_a_contact_is_assigned_to_no_one_else() {
    let db = migrated();
    db.connection()
        .execute_batch(
            "INSERT INTO workspaces (id, title) VALUES (1, 'w');
             INSERT INTO intelligence_agents (id, agent_name, agent_color) VALUES (1, 'Ada', 'blue');
             INSERT INTO contacts (id, workspace_id, first_name, last_name) VALUES (1, 1, 'Bo', 'Lee');
             INSERT INTO tasks (id, workspace_id, title, assignee_contact_id) VALUES (1, 1, 'his', 1);",
        )
        .unwrap();
    for both in [
        "INSERT INTO tasks (workspace_id, title, assigned_to_user, assignee_contact_id) VALUES (1, 'x', TRUE, 1)",
        "INSERT INTO tasks (workspace_id, title, assignee_agent_id, assignee_contact_id) VALUES (1, 'x', 1, 1)",
    ] {
        assert!(db.connection().execute(both, []).is_err(), "{both}");
    }
}

/// Required columns a model leaves out on purpose: an account's credential,
/// which only `CredentialManager` reads and writes.
const UNMODELED: &[(&str, &str)] = &[("connector_accounts", "authorization")];

/// Nullable columns a `CHECK` or a trigger asks for on a row that otherwise
/// takes every default: an endpoint that isn't a device's must have a name,
/// provider, dialect, base URL and cache lifetime, a connector account that isn't a client's
/// an extension, identifier, description and credential, and a skill an agent or a team.
const CHECKED: &[(&str, &str)] = &[
    ("intelligence_thread_messages", "text"),
    ("intelligence_agent_messages", "text"),
    ("actor_session_messages", "text"),
    ("attachments", "agent_message_id"),
    ("versions", "document_id"),
    ("metadata", "task_id"),
    ("articles", "final_url"),
    ("articles", "truncated"),
    ("feeds", "added"),
    ("storage_entries", "agent_id"),
    ("agent_skills", "agent_id"),
    ("intelligence_endpoints", "name"),
    ("intelligence_endpoints", "provider_identifier"),
    ("intelligence_endpoints", "dialect"),
    ("intelligence_endpoints", "base_url"),
    ("intelligence_endpoints", "cache_minutes"),
    ("connector_accounts", "extension_id"),
    ("connector_accounts", "identifier"),
    ("connector_accounts", "description"),
    ("connector_accounts", "authorization"),
    ("saved_task_views", "list"),
];

/// Required number columns a `CHECK` keeps above zero, which take 1.
const POSITIVE: &[(&str, &str)] = &[
    ("intelligence_endpoints", "cache_minutes"),
    ("intelligence_spend_limits", "amount"),
    ("agent_thread_marks", "cache_seconds"),
    ("agent_thread_marks", "memory_limit"),
    ("saved_task_view_sizes", "size"),
    ("agent_agenda_items", "position"),
];

/// Required text columns a `CHECK` keeps to a set of words, with one of
/// them, taken in place of the `x` other text columns get.
const WORDS: &[(&str, &str, &str)] = &[
    ("intelligence_agent_messages", "kind", "text"),
    ("articles", "snapshot_state", "captured"),
    ("feeds", "status_state", "refreshed"),
    ("actor_session_messages", "sender", "user"),
    ("actor_session_messages", "kind", "text"),
    ("versions", "change", "edited"),
    ("versions", "author", "user"),
    ("intelligence_spend_limits", "period", "day"),
    ("contact_methods", "kind", "email"),
    ("agent_thread_marks", "source", "conversation"),
    ("backlinks", "source_kind", "document"),
    ("backlinks", "target_kind", "task"),
    ("document_folder_properties", "data_type", "text"),
];

/// Required JSON columns of a table's own, which take `[]`.
const JSON_LISTS: &[(&str, &str)] = &[
    ("connectors", "metadata_keys"),
    ("actors", "modes"),
    ("actors", "models"),
    ("agent_thread_marks", "memories"),
];

/// Required JSON columns, which take `{}` rather than the `x` other text
/// columns get, since the unread trigger reads the message's tag.
const JSON_COLUMNS: &[(&str, &str)] = &[];

/// Checks that `M` matches the table the migrations create: every model
/// column exists, and every column the table requires is a model field, or
/// one in [`UNMODELED`], that a row can be stored with.
fn assert_conforms<M: Model>(db: WriteContainer<'_>) {
    db.container()
        .connection()
        .prepare(&format!(
            "SELECT {} FROM {}{} LIMIT 0",
            M::select_list(),
            M::TABLE,
            M::joins()
        ))
        .unwrap_or_else(|error| panic!("{} does not match its model: {error}", M::TABLE));

    // Columns that are NOT NULL, have no default, and are not the rowid,
    // and those in `CHECKED`.
    let mut statement = db
        .container()
        .connection()
        .prepare(&format!("PRAGMA table_info({})", M::TABLE))
        .unwrap();
    let required: Vec<(String, String)> = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>("name")?,
                row.get::<_, String>("type")?,
                row.get::<_, bool>("notnull")?,
                row.get::<_, Option<String>>("dflt_value")?,
                row.get::<_, i64>("pk")?,
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .filter(|(name, _, notnull, default, pk)| {
            (*notnull && default.is_none() && *pk == 0)
                || CHECKED.contains(&(M::TABLE, name.as_str()))
        })
        .map(|(name, kind, ..)| (name, kind))
        .collect();

    let mut values = Vec::new();
    for (name, kind) in &required {
        assert!(
            M::columns().contains(&name.as_str()) || UNMODELED.contains(&(M::TABLE, name.as_str())),
            "{}.{name} is required but not a model field",
            M::TABLE
        );
        values.push(if JSON_COLUMNS.contains(&(M::TABLE, name.as_str())) {
            "{}".into_value()
        } else if JSON_LISTS.contains(&(M::TABLE, name.as_str())) {
            "[]".into_value()
        } else if let Some((.., word)) = WORDS
            .iter()
            .find(|(table, column, _)| (*table, *column) == (M::TABLE, name.as_str()))
        {
            word.into_value()
        } else if kind.contains("TEXT") {
            "x".into_value()
        } else if POSITIVE.contains(&(M::TABLE, name.as_str())) {
            1i64.into_value()
        } else {
            0i64.into_value()
        });
    }
    let sql = if required.is_empty() {
        format!("INSERT INTO {} DEFAULT VALUES", M::TABLE)
    } else {
        let columns: Vec<&str> = required.iter().map(|(name, _)| name.as_str()).collect();
        format!(
            "INSERT INTO {} ({}) VALUES ({})",
            M::TABLE,
            columns.join(", "),
            vec!["?"; columns.len()].join(", ")
        )
    };
    db.container()
        .execute_statement(&sql, &values)
        .unwrap_or_else(|error| panic!("cannot insert into {}: {error}", M::TABLE));
}

#[test]
fn every_model_matches_the_schema() {
    let db = migrated();
    // Messages, permissions, hooks, agent rows, accounts, resources, service
    // functions, attachments, articles, endpoint models and tier
    // defaults are checked with `thread_id = 0`, `extension_id = 0`,
    // `agent_id = 0`, `connector_id = 0`, `account_id = 0`, `kind_id = 0`, `workspace_id = 0`,
    // `service_id = 0`, `message_id = 0`, `feed_id = 0`, `endpoint_id = 0`
    // `endpoint_model_id = 0`, `client_id = 0`, `actor_id = 0`,
    // `task_id = 0`, `contact_id = 0`, `tag_id = 0` (a task tag, with task 0 in
    // workspace 0 to put it on) and `view_id = 0` (a saved view of
    // workspace 0), `folder_id = 0`, `document_id = 0` and `property_id = 0`
    // (a document's value, on document 0 in folder 0 and that folder's
    // property 0, a choice so it can hold an option), which must exist; `message_id = 0` is both
    // an agent's message and an actor task's. That service belongs to another
    // extension, leaving extension 0 free to declare the one checked. The
    // security and storage settings' and the profile's one rows make way for
    // the ones checked, as does Rewind's writer. Versions hang on task 0 and
    // on document 0.
    db.connection()
        .execute_batch(
            "DELETE FROM security_settings;
             DELETE FROM storage_settings;
             DELETE FROM profile;
             DELETE FROM rewind_writer;
             INSERT INTO thread_prompts (id, prompt) VALUES (0, '');
             INSERT INTO intelligence_threads (id, prompt_id) VALUES (0, 0);
             INSERT INTO intelligence_thread_messages (id, thread_id, kind, text)
                 VALUES (0, 0, 0, '');
             INSERT INTO extensions (id, name, version, script, script_identifier)
                 VALUES (0, '', 0, '', '');
             INSERT INTO intelligence_agent_teams (id, title) VALUES (0, '');
             INSERT INTO intelligence_agents (id, agent_name, agent_color, team_id)
                 VALUES (0, '', 'blue', 0);
             INSERT INTO intelligence_agent_messages (id, agent_id, sender, kind, text)
                 VALUES (0, 0, 'user', 'text', '');
             INSERT INTO connectors (id, identifier, version, title, unit) VALUES (0, '', 0, '', '');
             INSERT INTO connector_accounts (id, connector_id, extension_id, identifier,
                 description, authorization)
                 VALUES (0, 0, 0, '', '', '');
             INSERT INTO resource_kinds (id, connector_id, identifier, title, plural, contents)
                 VALUES (0, 0, '', '', '', '');
             INSERT INTO workspaces (id, title) VALUES (0, '');
             INSERT INTO feeds (id, url, title, status_state, status_at, added)
                 VALUES (0, '', '', 'refreshed', 0, 0);
             INSERT INTO tasks (id, workspace_id, title) VALUES (0, 0, '');
             INSERT INTO document_folders (id, workspace_id, title) VALUES (0, 0, '');
             INSERT INTO documents (id, workspace_id, title, contents, folder_id)
                 VALUES (0, 0, '', '', 0);
             INSERT INTO document_folder_properties (id, folder_id, name, data_type)
                 VALUES (0, 0, '', 'choice');
             INSERT INTO contacts (id, workspace_id, first_name, last_name)
                 VALUES (0, 0, '', '');
             INSERT INTO tags (id, workspace_id, name) VALUES (0, 0, 'checked');
             INSERT INTO saved_task_views (id, workspace_id, name, list, sort, descending)
                 VALUES (0, 0, 'checked', 'inbox', 'created', 0);
             INSERT INTO extensions (id, name, version, script, script_identifier)
                 VALUES (1, '', 0, '', 'service');
             INSERT INTO extension_services (id, extension_id, description)
                 VALUES (0, 1, '');
             INSERT INTO intelligence_endpoints
                     (id, name, provider_identifier, dialect, base_url, cache_minutes)
                 VALUES (0, '', '', 'messages', '', 5);
             INSERT INTO endpoint_models (id, endpoint_id, model_identifier, model_name)
                 VALUES (0, 0, '', '');
             INSERT INTO clients (id, key, client_name) VALUES (0, '', '');
             INSERT INTO actors (id, client_id, identifier, product, scope, modes, models)
                 VALUES (0, 0, '', '', '', '[]', '[]');
             INSERT INTO actor_sessions (id, actor_id, title) VALUES (0, 0, '');
             INSERT INTO actor_session_messages (id, session_id, sender, kind, text)
                 VALUES (0, 0, 'user', 'text', '');",
        )
        .unwrap();
    let db = WriteContainer::new(&db);
    every_model!(assert_conforms(db));
}

#[test]
fn an_endpoint_has_a_server_and_a_cache_lifetime_and_its_name_is_unique_ignoring_case() {
    let db = migrated();
    let insert = |values: &str| {
        db.connection().execute(
            &format!(
                "INSERT INTO intelligence_endpoints
                     (name, provider_identifier, dialect, base_url, api_key, cache_minutes)
                     VALUES {values}"
            ),
            [],
        )
    };
    // Every endpoint has a name, provider, dialect, base URL and a cache
    // lifetime of at least a minute.
    insert("(NULL, 'anthropic', 'messages', 'https://a', NULL, 5)").unwrap_err();
    insert("('a', NULL, 'messages', 'https://a', NULL, 5)").unwrap_err();
    insert("('a', 'anthropic', NULL, 'https://a', NULL, 5)").unwrap_err();
    insert("('a', 'anthropic', 'messages', NULL, NULL, 5)").unwrap_err();
    insert("('a', 'anthropic', 'messages', 'https://a', NULL, NULL)").unwrap_err();
    insert("('a', 'anthropic', 'messages', 'https://a', NULL, 0)").unwrap_err();
    insert("('Anthropic', 'anthropic', 'messages', 'https://a', 'key', 5)").unwrap();
    // One per name ignoring case.
    insert("('ANTHROPIC', 'x', 'messages', 'https://a', NULL, 5)").unwrap_err();
    // The name compares ignoring case wherever it is used.
    let found: i64 = db
        .connection()
        .query_row(
            "SELECT count(*) FROM intelligence_endpoints WHERE name = 'anthropic'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(found, 1);
}

#[test]
fn endpoints_take_their_providers_cache_lifetime() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "endpoint_cache_minutes")
        .unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO clients (id, key, client_name) VALUES (1, 'k', 'Mac mini');
             INSERT INTO intelligence_endpoints (id, name, provider_identifier, dialect, base_url)
                 VALUES (1, 'OpenAI', 'openai', 'responses', 'https://a'),
                        (2, 'Anthropic', 'anthropic', 'messages', 'https://b');
             INSERT INTO intelligence_endpoints (id, client_id) VALUES (3, 1);",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    // The client's endpoint, which had none, went once clients offered
    // only actors.
    let minutes: Vec<i64> = db
        .connection()
        .prepare("SELECT cache_minutes FROM intelligence_endpoints ORDER BY id")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(minutes, [30, 5]);
}

#[test]
fn endpoints_speaking_their_providers_api_take_its_long_cache() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "endpoint_long_cache_minutes")
        .unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO intelligence_endpoints
                 (id, name, provider_identifier, dialect, base_url, cache_minutes)
                 VALUES (1, 'OpenAI', 'openai', 'responses', 'https://api.openai.com/v1', 30),
                        (2, 'Anthropic', 'anthropic', 'messages', 'https://api.anthropic.com/v1', 5),
                        (3, 'Proxy', 'openai', 'responses', 'https://proxy', 30),
                        (4, 'Mistral', 'mistral', 'chat-completions', 'https://api.mistral.ai/v1', 5);",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    // The next migration keeps the minutes in `long_cache`, with the
    // provider's price.
    let long: Vec<(Option<i64>, Option<f64>)> = db
        .connection()
        .prepare(
            "SELECT long_cache ->> '$.minutes', long_cache ->> '$.write_multiple'
             FROM intelligence_endpoints ORDER BY id",
        )
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        long,
        [
            (Some(720), Some(2.0)),
            (Some(60), Some(1.6)),
            (None, None),
            (None, None)
        ]
    );
}

#[test]
fn an_account_has_a_credential_only_with_an_extension_and_always_a_description() {
    let db = migrated();
    db.connection()
        .execute_batch(
            "INSERT INTO extensions (id, name, version, script, script_identifier)
                 VALUES (4, 'c', '1', '', 'com.example.c');
             INSERT INTO connectors (id, identifier, version, title, unit)
                 VALUES (1, 'notes', 1, 'Notes', 'Device');",
        )
        .unwrap();
    let insert = |extension: &str, identifier: &str, description: &str, credential: &str| {
        db.connection().execute(
            &format!(
                "INSERT INTO connector_accounts (connector_id, extension_id, identifier,
                     description, authorization)
                     VALUES (1, {extension}, {identifier}, {description}, {credential})"
            ),
            [],
        )
    };
    // An extension's account has an identifier and a credential.
    insert("4", "NULL", "'d'", "'token'").unwrap_err();
    insert("4", "'a'", "'d'", "NULL").unwrap_err();
    insert("4", "'a'", "NULL", "'token'").unwrap_err();
    insert("4", "'a'", "'d'", "'token'").unwrap();
    // An orphan has neither, and a description.
    insert("NULL", "'n'", "'Old Mac'", "NULL").unwrap_err();
    insert("NULL", "NULL", "'Old Mac'", "'token'").unwrap_err();
    insert("NULL", "NULL", "NULL", "NULL").unwrap_err();
    insert("NULL", "NULL", "'Old Mac'", "NULL").unwrap();
}

#[test]
fn an_extensions_removal_orphans_its_accounts_and_keeps_their_resources() {
    let db = migrated();
    db.connection()
        .execute_batch(
            "INSERT INTO extensions (id, name, version, script, script_identifier)
                 VALUES (4, 'c', '1', '', 'com.example.c');
             INSERT INTO workspaces (id, title) VALUES (2, 'w');
             INSERT INTO connectors (id, identifier, version, title, unit)
                 VALUES (1, 'notes', 1, 'Notes', 'Device'), (2, 'com.example.c', 1, 'C', 'Space');
             INSERT INTO connector_accounts (id, connector_id, description) VALUES (1, 1, 'Old Mac');
             INSERT INTO connector_accounts (id, connector_id, extension_id, identifier,
                 description, authorization, refresh) VALUES (2, 2, 4, 'a', 'Ada', 't', 'r');
             INSERT INTO resource_kinds (id, connector_id, identifier, title, plural, contents)
                 VALUES (1, 1, 'note', 'Note', 'Notes', ''), (2, 2, 'page', 'Page', 'Pages', '');
             INSERT INTO resources (workspace_id, account_id, kind_id, identifier, title,
                 content_kind, contents, last_updated) VALUES
                 (2, 1, 1, 'n1', 'Groceries', 'markdown', '', 1),
                 (2, 2, 2, 'p1', 'Plan', 'markdown', '', 1);
             DELETE FROM extensions WHERE id = 4;",
        )
        .unwrap();
    let (hosted, credentials, resources): (i64, i64, i64) = db
        .connection()
        .query_row(
            "SELECT (SELECT count(extension_id) FROM connector_accounts),
                 (SELECT count(authorization) + count(refresh) FROM connector_accounts),
                 (SELECT count(*) FROM resources)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!((hosted, credentials, resources), (0, 0, 2));

    // A resource moves only within its connector.
    db.connection()
        .execute_batch("UPDATE resources SET account_id = 2 WHERE account_id = 1")
        .unwrap_err();
}

#[test]
fn clients_offering_only_actors_take_their_endpoints_and_orphan_their_accounts() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "clients_offer_only_actors")
        .unwrap();
    // The first migration to run with foreign keys off.
    assert_eq!(at, CASCADING_MIGRATIONS);
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO clients (id, key, client_name) VALUES (1, 'k', 'Mac mini');
             INSERT INTO extensions (id, name, version, script, script_identifier)
                 VALUES (4, 'c', '1', '', 'com.example.c');
             INSERT INTO workspaces (id, title) VALUES (2, 'w');
             INSERT INTO connectors (id, identifier, version, title, unit)
                 VALUES (1, 'notes', 1, 'Notes', 'Device'), (2, 'com.example.c', 1, 'C', 'Space');
             INSERT INTO connector_accounts (id, connector_id, client_id) VALUES (1, 1, 1);
             INSERT INTO connector_accounts (id, connector_id, extension_id, identifier,
                 description, authorization) VALUES (2, 2, 4, 'a', 'Ada', 't');
             INSERT INTO resource_kinds (id, connector_id, identifier, title, plural, contents)
                 VALUES (1, 1, 'note', 'Note', 'Notes', '');
             INSERT INTO resources (workspace_id, account_id, kind_id, identifier, title,
                 content_kind, contents, last_updated)
                 VALUES (2, 1, 1, 'n1', 'Groceries', 'markdown', '', 1);
             INSERT INTO intelligence_endpoints (id, client_id) VALUES (1, 1);
             INSERT INTO intelligence_endpoints (id, name, provider_identifier, dialect,
                 base_url, cache_minutes) VALUES (2, 'Anthropic', 'anthropic', 'messages', 'https://a', 5);
             INSERT INTO intelligence_endpoints (id, name, provider_identifier, dialect,
                 base_url, account_id, cache_minutes) VALUES (3, 'C', 'openai', 'responses', 'https://c', 2, 30);
             INSERT INTO endpoint_models (id, endpoint_id, model_identifier)
                 VALUES (1, 1, 'base'), (2, 2, 'claude'), (3, 3, 'gpt');
             INSERT INTO intelligence_defaults (tier, endpoint_model_id)
                 VALUES ('low', 1), ('agent', 2), ('high', 3);",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();
    assert!(foreign_keys_enabled(&db));

    let ids = |sql: &str| -> Vec<i64> {
        db.connection()
            .prepare(sql)
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    // The client's endpoint went, with its model and tier; the others stay
    // whole, and so does every account and resource.
    assert_eq!(
        ids("SELECT id FROM intelligence_endpoints ORDER BY id"),
        [2, 3]
    );
    assert_eq!(ids("SELECT id FROM endpoint_models ORDER BY id"), [2, 3]);
    assert_eq!(
        ids("SELECT endpoint_model_id FROM intelligence_defaults ORDER BY endpoint_model_id"),
        [2, 3]
    );
    assert_eq!(ids("SELECT count(*) FROM resources"), [1]);
    // The client's account is orphaned, described by the client's name.
    let db = WriteContainer::new(&db);
    let accounts = db
        .select::<ConnectorAccount>()
        .order("connector_accounts.id")
        .all()
        .unwrap();
    assert_eq!(accounts[0].host, AccountHost::Orphaned);
    assert_eq!(accounts[0].description, "Mac mini");
    assert!(matches!(accounts[1].host, AccountHost::Extension { .. }));

    // The triggers that follow the rebuilt tables still fire: uninstalling
    // the extension orphans its account and takes the endpoint signing in
    // with it.
    db.container()
        .connection()
        .execute_batch("DELETE FROM extensions WHERE id = 4")
        .unwrap();
    assert_eq!(ids("SELECT id FROM intelligence_endpoints"), [2]);
}

#[test]
fn connector_definitions_carry_accounts_and_assets_over() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "connector_definitions")
        .unwrap();
    let earlier = &MIGRATIONS[..at];
    for migration in earlier {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO clients (id, key, client_name) VALUES (1, 'k', 'Mac mini');
             INSERT INTO extensions (id, name, version, script, script_identifier, icon)
                 VALUES (4, 'c', '1', '', 'com.example.c', 'https://example.com/c.png');
             INSERT INTO workspaces (id, title) VALUES (2, 'w');
             INSERT INTO connector_accounts (id, extension_id, extension_identifier, identifier,
                 service_title, account_unit, asset_kinds, description, authorization)
                 VALUES (5, 4, 'com.example.c', 'a', 'C', 'Space', '[]', 'Ada', 't');
             INSERT INTO connector_accounts (id, client_id, offer, service_title, account_unit)
                 VALUES (6, 1, 'notes', 'Notes', 'Device');
             INSERT INTO edge_resources (account_id, identifier, title, kind)
                 VALUES (6, 'n1', 'Groceries', 'Note');
             INSERT INTO assets (id, workspace_id, connector_account_id, identifier, kind,
                 kind_description, title, content_kind, contents, last_updated) VALUES
                 (8, 2, 5, 'p1', 'com.example.c.page', 'Page', 'Plan', 'markdown', 'apples', 1),
                 (9, 2, 6, 'n1', 'client.notes.note', 'Note', 'Groceries', 'markdown', 'milk', 1);",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", earlier.len() as i64)
        .unwrap();
    db.migrate().unwrap();
    assert_eq!(user_version(&db), MIGRATIONS.len() as i64);

    let db = WriteContainer::new(&db);
    let mut kinds: Vec<(i64, i64, String)> = db
        .select::<Resource>()
        .all()
        .unwrap()
        .into_iter()
        .map(|resource| (resource.id, resource.account_id, resource.kind))
        .collect();
    kinds.sort();
    assert_eq!(
        kinds,
        [
            (8, 5, "com.example.c.page".to_string()),
            (9, 6, "notes.note".to_string())
        ]
    );
    let accounts = db.select::<ConnectorAccount>().all().unwrap();
    assert_eq!(accounts.len(), 2);
    let icons: Vec<Option<String>> = db
        .select::<Connector>()
        .order("identifier")
        .all()
        .unwrap()
        .into_iter()
        .map(|connector| connector.icon)
        .collect();
    assert_eq!(icons, [Some("https://example.com/c.png".to_string()), None]);
    assert_eq!(
        search(db.container(), "apples"),
        [("resource".to_string(), 8)]
    );
}

#[test]
fn extension_versions_keep_whole_numbers_and_zero_the_rest() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "extension_versions")
        .unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO extensions (id, name, version, script, script_identifier, announced_version)
                 VALUES (1, 'a', '2', '', 'a', '3'), (2, 'b', '1.0.0', '', 'b', '1.1'),
                        (3, 'c', '', '', 'c', NULL);",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let db = WriteContainer::new(&db);
    let versions: Vec<(i64, Option<i64>)> = db
        .select::<Extension>()
        .order("id")
        .all()
        .unwrap()
        .into_iter()
        .map(|extension| (extension.version, extension.announced_version))
        .collect();
    assert_eq!(versions, [(2, Some(3)), (0, Some(0)), (0, None)]);
}

#[test]
fn resource_kinds_become_rows_and_start_shown_where_resources_are() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "resource_kinds")
        .unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO workspaces (id, title) VALUES (1, 'a'), (2, 'b');
             INSERT INTO clients (id, key, client_name) VALUES (1, 'k', 'Mac mini');
             INSERT INTO connectors (id, identifier, version, title, unit, kinds)
                 VALUES (1, 'notes', 1, 'Notes', 'Device',
                     '[{\"identifier\":\"note\",\"title\":\"Note\",\"contents\":\"Markdown\"}]');
             INSERT INTO connector_accounts (id, connector_id, client_id) VALUES (1, 1, 1);
             INSERT INTO resources (id, workspace_id, account_id, identifier, kind, title,
                 content_kind, contents, last_updated) VALUES
                 (7, 1, 1, 'n1', 'note', 'Groceries', 'markdown', 'milk', 1),
                 (8, 1, 1, 'f1', 'folder', 'Recipes', 'markdown', '', 1);",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();
    // The search index still finds the moved rows.
    assert_eq!(search(&db, "milk"), [("resource".to_string(), 7)]);

    let db = WriteContainer::new(&db);
    // A declared kind takes its title as its plural; one only a resource
    // names becomes a kind named by its key.
    let kinds: Vec<(String, String, String)> = db
        .select::<ResourceKind>()
        .order("id")
        .all()
        .unwrap()
        .into_iter()
        .map(|kind| (kind.identifier, kind.title, kind.plural))
        .collect();
    assert_eq!(
        kinds,
        [
            ("note".to_string(), "Note".to_string(), "Note".to_string()),
            (
                "folder".to_string(),
                "folder".to_string(),
                "folder".to_string()
            ),
        ]
    );
    let resources: Vec<(i64, String)> = db
        .select::<Resource>()
        .order("resources.id")
        .all()
        .unwrap()
        .into_iter()
        .map(|resource| (resource.id, resource.kind))
        .collect();
    assert_eq!(
        resources,
        [
            (7, "notes.note".to_string()),
            (8, "notes.folder".to_string())
        ]
    );
    // Both are shown where they are held, and nowhere else.
    let shown: Vec<i64> = db
        .select::<WorkspaceResourceKind>()
        .all()
        .unwrap()
        .into_iter()
        .map(|shown| shown.workspace_id)
        .collect();
    assert_eq!(shown, [1, 1]);
}

#[test]
fn shown_kinds_start_in_their_connectors_order() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "workspace_resource_kind_order")
        .unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO workspaces (id, title) VALUES (1, 'a'), (2, 'b');
             INSERT INTO connectors (id, identifier, version, title, unit)
                 VALUES (1, 'notes', 1, 'Notes', 'Device'), (2, 'mail', 1, 'Mail', 'Device');
             INSERT INTO resource_kinds (id, connector_id, identifier, title, plural, contents)
                 VALUES (1, 2, 'message', 'Message', 'Messages', ''),
                        (2, 1, 'note', 'Note', 'Notes', ''),
                        (3, 1, 'folder', 'Folder', 'Folders', '');
             INSERT INTO workspace_resource_kinds (id, workspace_id, kind_id)
                 VALUES (1, 1, 1), (2, 1, 3), (3, 1, 2), (4, 2, 1);",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let db = WriteContainer::new(&db);
    let shown: Vec<(i64, i64, i64)> = db
        .select::<WorkspaceResourceKind>()
        .order("workspace_id, sort_order")
        .all()
        .unwrap()
        .into_iter()
        .map(|shown| (shown.workspace_id, shown.kind_id, shown.sort_order))
        .collect();
    assert_eq!(shown, [(1, 2, 1), (1, 3, 2), (1, 1, 3), (2, 1, 1)]);
}

#[test]
fn articles_dated_midnight_utc_become_days() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "article_published_precision")
        .unwrap();
    let earlier = &MIGRATIONS[..at];
    for migration in earlier {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            r#"INSERT INTO feeds (id, url, title, status)
                 VALUES (1, 'https://example.com/feed', 'f', '{"state":"refreshed","at":0,"added":0}');
             INSERT INTO articles (id, feed_id, url, title, published_at, snapshot) VALUES
                 (1, 1, 'https://example.com/a', 'a', 1709251200, '{"state":"failed","failure":{"kind":"tooLarge"},"attempted_at":0}'),
                 (2, 1, 'https://example.com/b', 'b', 1709294400, '{"state":"failed","failure":{"kind":"tooLarge"},"attempted_at":0}');"#,
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", earlier.len() as i64)
        .unwrap();
    db.migrate().unwrap();

    let precisions: Vec<(i64, PublishedPrecision)> = WriteContainer::new(&db)
        .select::<Article>()
        .order("id")
        .all()
        .unwrap()
        .into_iter()
        .map(|article| (article.id, article.published_precision))
        .collect();
    assert_eq!(
        precisions,
        [(1, PublishedPrecision::Day), (2, PublishedPrecision::Time)]
    );
}

#[test]
fn cost_events_keep_their_parts_and_gain_their_total() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "cost_event_total")
        .unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO intelligence_threads (id, system_prompt) VALUES (1, '');
             INSERT INTO intelligence_cost_events (id, thread_id, provider_identifier,
                     model_identifier, input_cost, output_cost, cache_read_cost, cache_write_cost)
                 VALUES (1, 1, 'anthropic', 'haiku', 0.5, 0.25, 0.125, 0.0625);",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let db = WriteContainer::new(&db);
    let event = db
        .select::<IntelligenceCostEvent>()
        .filter_eq(IntelligenceCostEvent::ID, 1)
        .one()
        .unwrap()
        .unwrap();
    assert_eq!(event.thread_id, Some(1));
    assert_eq!(event.total_cost, 0.9375);
    assert_eq!(
        [
            event.input_cost,
            event.output_cost,
            event.cache_read_cost,
            event.cache_write_cost
        ],
        [Some(0.5), Some(0.25), Some(0.125), Some(0.0625)]
    );
}

#[test]
fn pinned_text_messages_become_memories_and_pins_go() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "memories")
        .unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO intelligence_agents (id, agent_name, agent_color) VALUES (1, 'Ada', 'blue');
             INSERT INTO intelligence_agent_messages (agent_id, sent_at, sender, pinned,
                     message_content) VALUES
                 (1, 100, 'user', TRUE, '{\"kind\":\"text\",\"text\":\"Call me Jo\"}'),
                 (1, 200, 'agent', TRUE, '{\"kind\":\"status\",\"status\":\"Reading\"}'),
                 (1, 300, 'user', FALSE, '{\"kind\":\"text\",\"text\":\"Hello\"}');",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let columns: Vec<String> = db
        .connection()
        .prepare("SELECT name FROM pragma_table_info('intelligence_agent_messages')")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(!columns.iter().any(|column| column == "pinned"));

    let db = WriteContainer::new(&db);
    let memories: Vec<(MemoryScope, i64, String)> = db
        .select::<Memory>()
        .all()
        .unwrap()
        .into_iter()
        .map(|memory| (memory.scope, memory.created_at, memory.memory))
        .collect();
    assert_eq!(
        memories,
        [(
            MemoryScope::Agent { agent_id: 1 },
            100,
            "Call me Jo".to_string()
        )]
    );
}

#[test]
fn memories_keep_their_ids_and_scopes_and_a_teams_go_with_it() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "team_memories")
        .unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO intelligence_agents (id, agent_name, agent_color) VALUES (1, 'Ada', 'blue');
             INSERT INTO memories (id, agent_id, created_at, memory) VALUES
                 (5, NULL, 100, 'Call me Jo'),
                 (9, 1, 200, 'Ada files the groceries');",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    {
        let db = WriteContainer::new(&db);
        let memories: Vec<(i64, MemoryScope)> = db
            .select::<Memory>()
            .order("id")
            .all()
            .unwrap()
            .into_iter()
            .map(|memory| (memory.id, memory.scope))
            .collect();
        assert_eq!(
            memories,
            [
                (5, MemoryScope::Global),
                (9, MemoryScope::Agent { agent_id: 1 })
            ]
        );
    }
    let connection = db.connection();
    assert!(
        connection
            .execute_batch("SELECT 1 FROM memories_fts")
            .is_err()
    );
    // A memory is an agent's or a team's, never both.
    connection
        .execute_batch("INSERT INTO intelligence_agent_teams (id, title) VALUES (1, 'Home');")
        .unwrap();
    assert!(
        connection
            .execute_batch(
                "INSERT INTO memories (agent_id, team_id, memory) VALUES (1, 1, 'Both');"
            )
            .is_err()
    );
    connection
        .execute_batch(
            "INSERT INTO memories (team_id, memory) VALUES (1, 'Repairs go under Home');
             DELETE FROM intelligence_agent_teams WHERE id = 1;",
        )
        .unwrap();
    let left: i64 = connection
        .query_row(
            "SELECT count(*) FROM memories WHERE team_id IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(left, 0);
}

#[test]
fn agent_statuses_become_an_activity_and_status_lines_go() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "agent_activity")
        .unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO intelligence_agents (id, agent_name, agent_color, foreground_status,
                     system_defined_status) VALUES (1, 'Ada', 'blue', 'Watching', 'working');
             INSERT INTO intelligence_agent_messages (agent_id, sent_at, sender,
                     message_content) VALUES
                 (1, 100, 'agent', '{\"kind\":\"status\",\"status\":\"Reading\"}'),
                 (1, 200, 'agent', '{\"kind\":\"text\",\"text\":\"Hello\"}');",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let db = WriteContainer::new(&db);
    let agent = db
        .select::<IntelligenceAgent>()
        .filter_eq(IntelligenceAgent::ID, 1)
        .one()
        .unwrap()
        .unwrap();
    assert_eq!(agent.activity, None);
    let messages: Vec<i64> = db
        .select::<IntelligenceAgentMessage>()
        .all()
        .unwrap()
        .into_iter()
        .map(|message| message.sent_at)
        .collect();
    assert_eq!(messages, [200]);
}

#[test]
fn activities_are_an_actor_tasks_and_end_exactly_with_an_outcome() {
    let db = migrated();
    db.connection()
        .execute_batch(
            "INSERT INTO clients (id, key, client_name) VALUES (1, 'k', 'Mac');
             INSERT INTO actors (id, client_id, identifier, product, scope, modes, models)
                 VALUES (1, 1, 'code', 'Claude Code', '/src', '[]', '[]');
             INSERT INTO actor_sessions (id, actor_id, title) VALUES (1, 1, 'Fix'), (2, 1, 'Ship');",
        )
        .unwrap();
    let insert = |values: &str| {
        db.connection().execute_batch(&format!(
            "INSERT INTO activities (title, actor_session_id, status, ended_at) VALUES {values}"
        ))
    };
    insert("('Running', 1, 'running', NULL)").unwrap();
    insert("('Ended', 2, 'succeeded', 5)").unwrap();
    // No task.
    assert!(insert("('Orphan', NULL, 'running', NULL)").is_err());
    // Ended without an outcome, or with one but not ended.
    assert!(insert("('Odd', 2, 'running', 5)").is_err());
    assert!(insert("('Odd', 2, 'failed', NULL)").is_err());
    assert!(insert("('Odd', 2, 'paused', 5)").is_err());
    assert!(insert("('Odd', 2, 'stuck', NULL)").is_err());
    assert!(
        db.connection()
            .execute_batch("UPDATE activities SET status = 'cancelled' WHERE title = 'Running'")
            .is_err()
    );
}

#[test]
fn removing_detached_runs_keeps_actor_task_activities_and_their_unread_trigger() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "remove_detached_runs")
        .unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO intelligence_agents (id, agent_name, agent_color) VALUES (1, 'Scout', 'blue');
             INSERT INTO clients (id, key, client_name) VALUES (1, 'k', 'Mac');
             INSERT INTO actors (id, client_id, identifier, product, scope, modes, models)
                 VALUES (1, 1, 'code', 'Claude Code', '/src', '[]', '[]');
             INSERT INTO actor_tasks (id, actor_id, title) VALUES (1, 1, 'Fix');
             INSERT INTO activities (id, title, agent_id, status, ended_at)
                 VALUES (1, 'Research', 1, 'succeeded', 5);
             INSERT INTO intelligence_agent_detached_runs (agent_id, activity_id, instructions, effort)
                 VALUES (1, 1, 'Look', 'low');
             INSERT INTO activities (id, title, actor_task_id, status, progress)
                 VALUES (2, 'Fix', 1, 'running', 'Building');",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let kept = WriteContainer::new(&db).select::<Activity>().all().unwrap();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].id, 2);
    assert_eq!(kept[0].progress.as_deref(), Some("Building"));
    db.connection()
        .execute_batch("UPDATE activities SET status = 'succeeded', ended_at = 9 WHERE id = 2")
        .unwrap();
    let unread: bool = db
        .connection()
        .query_row(
            "SELECT unread FROM actor_sessions WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(unread);
}

#[test]
fn actor_tasks_keep_their_messages_and_activities_as_they_take_a_status() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "actor_task_status")
        .unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO clients (id, key, client_name) VALUES (1, 'k', 'Mac');
             INSERT INTO actors (id, client_id, identifier, product, scope, modes, models)
                 VALUES (1, 1, 'code', 'Claude Code', '/src', '[]', '[]');
             INSERT INTO actor_tasks (id, actor_id, title, outcome, ended_at) VALUES
                 (1, 1, 'Open', NULL, NULL),
                 (2, 1, 'Broke', 'failed', 5),
                 (3, 1, 'Done', 'succeeded', 5);
             INSERT INTO actor_task_messages (task_id, sent_at, content) VALUES
                 (1, 100, '{\"sender\":\"user\",\"kind\":\"text\",\"text\":\"Go\"}'),
                 (2, 200, '{\"sender\":\"user\",\"kind\":\"text\",\"text\":\"Go\"}');
             INSERT INTO activities (title, actor_task_id, status, ended_at)
                 VALUES ('Broke', 2, 'failed', 5);",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let tasks = WriteContainer::new(&db);
    let statuses: Vec<(i64, ActorSessionStatus)> = tasks
        .select::<ActorSession>()
        .order("id")
        .all()
        .unwrap()
        .into_iter()
        .map(|task| (task.id, task.status))
        .collect();
    assert_eq!(
        statuses,
        [
            (1, ActorSessionStatus::Idle),
            (2, ActorSessionStatus::Failed),
            (3, ActorSessionStatus::Idle)
        ]
    );
    assert_eq!(
        tasks.select::<ActorSessionMessage>().all().unwrap().len(),
        2
    );
    assert_eq!(tasks.select::<Activity>().all().unwrap().len(), 1);
    db.connection()
        .execute_batch("UPDATE actor_tasks SET status = 'running' WHERE id = 1")
        .unwrap_err();
}

/// A cue from before cues had to name an effort was a foreground one: it
/// took the `foreground` effort, and goes once cues are background only.
#[test]
fn foreground_cues_take_the_foreground_effort_then_go() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "agent_cue_foreground_effort")
        .unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO intelligence_agents (id, agent_name, agent_color) VALUES (1, 'Scout', 'blue');
             INSERT INTO intelligence_agent_cues (agent_id, name, hook, script, instructions, effort)
                 VALUES (1, 'Triage', 'task:create', '() => true', '', 'low'),
                        (1, 'Follow', 'actor:moved', '() => true', '', NULL);",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let cues: Vec<(String, IntelligenceEffort)> = WriteContainer::new(&db)
        .select::<IntelligenceAgentCue>()
        .order("id")
        .all()
        .unwrap()
        .into_iter()
        .map(|cue| (cue.name, cue.effort))
        .collect();
    assert_eq!(cues, [("Triage".to_string(), IntelligenceEffort::Low)]);
    db.connection()
        .execute_batch("UPDATE intelligence_agent_cues SET effort = NULL")
        .unwrap_err();
}

#[test]
fn code_runs_say_how_they_ended_in_one_outcome() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "agent_foreground_started_code_outcome")
        .unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO intelligence_agents (id, agent_name, agent_color) VALUES (1, 'Ada', 'blue');
             INSERT INTO intelligence_agent_messages (agent_id, sender, message_content) VALUES
                 (1, 'agent', '{\"kind\":\"codeExecution\",\"title\":\"A\",\"code\":\"1\",\"result\":\"1\",\"error\":null}'),
                 (1, 'agent', '{\"kind\":\"codeExecution\",\"title\":\"B\",\"code\":\"x\",\"result\":null,\"error\":\"ReferenceError\"}'),
                 (1, 'user', '{\"kind\":\"text\",\"text\":\"Hello\"}');",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let db = WriteContainer::new(&db);
    let outcomes: Vec<String> = db
        .select::<IntelligenceAgentMessage>()
        .all()
        .unwrap()
        .into_iter()
        .map(|message| match message.message_content {
            AgentMessageContent::CodeExecution(execution) => match execution.outcome {
                AgentCodeOutcome::Succeeded { result } => format!("succeeded {result}"),
                AgentCodeOutcome::Failed { error } => format!("failed {error}"),
                AgentCodeOutcome::Cancelled => "cancelled".to_string(),
            },
            _ => "other".to_string(),
        })
        .collect();
    assert_eq!(outcomes, ["succeeded 1", "failed ReferenceError", "other"]);
    let agent = db
        .select::<IntelligenceAgent>()
        .filter_eq(IntelligenceAgent::ID, 1)
        .one()
        .unwrap()
        .unwrap();
    assert_eq!(agent.foreground_started, None);
}

#[test]
fn hourly_cues_move_to_the_timer_and_keep_their_code() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "hour_start_becomes_timer")
        .unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO intelligence_agents (id, agent_name, agent_color) VALUES (1, 'Ada', 'blue');
             INSERT INTO intelligence_agent_cues (agent_id, name, hook, script, instructions, effort)
                 VALUES (1, 'Nightly', 'hour:start', '({ hour }) => hour === 21', '', 'medium'),
                        (1, 'Triage', 'task:create', '() => true', '', 'low');",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let cues: Vec<(String, HookKind, String)> = WriteContainer::new(&db)
        .select::<IntelligenceAgentCue>()
        .order("id")
        .all()
        .unwrap()
        .into_iter()
        .map(|cue| (cue.name, cue.hook, cue.script))
        .collect();
    let [(_, nightly_hook, nightly), (_, triage_hook, triage)] = cues.as_slice() else {
        panic!("both cues survive");
    };
    assert_eq!(*nightly_hook, HookKind::Timer);
    assert!(nightly.contains("({ hour }) => hour === 21"));
    assert_eq!(*triage_hook, HookKind::TaskCreate);
    assert_eq!(triage, "() => true");
}

#[test]
fn hourly_runs_move_to_the_timer() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "hour_start_runs_become_timer")
        .unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO intelligence_agents (id, agent_name, agent_color) VALUES (1, 'Ada', 'blue');
             INSERT INTO intelligence_agent_runs (agent_id, hook, outcome)
                 VALUES (1, 'hour:start', 'completed'), (1, 'task:create', 'completed');",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let hooks: Vec<HookKind> = WriteContainer::new(&db)
        .select::<IntelligenceAgentRun>()
        .order("id")
        .all()
        .unwrap()
        .into_iter()
        .map(|run| run.hook)
        .collect();
    assert_eq!(hooks, [HookKind::Timer, HookKind::TaskCreate]);
}

#[test]
fn forwards_sent_as_text_become_forward_messages() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "agent_message_forwards")
        .unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO intelligence_agents (id, agent_name, agent_color) VALUES (1, 'Ada', 'blue');
             INSERT INTO workspaces (id, title) VALUES (3, 'Home');
             INSERT INTO documents (id, workspace_id, document_type, title, contents, journal_date)
                 VALUES (57, 3, 'journal', '2026-10-20', '', '2026-10-20');",
        )
        .unwrap();
    let sent = [
        ("user", "Forwarded task [Fix login](cohesion://task/3/42)."),
        (
            "user",
            "Look at this\n\nand this\n\nForwarded document [A \\[b\\] c\\\\d](cohesion://document/3/7).",
        ),
        (
            "user",
            "Forwarded journal entry [Tuesday, October 20](cohesion://document/3/57).",
        ),
        (
            "user",
            "Note\n\nForwarded task #42 “Fix “login”” in workspace 3. Read it with the code tool: `cohesion.workspace(3).task(42)`.",
        ),
        (
            "user",
            "Forwarded journal entry #57 “Tuesday, October 20” in workspace 3. Read it with the code tool: `cohesion.workspace(3).document(57)`.",
        ),
        // Each of these only looks like a forward.
        (
            "user",
            "Forwarded task [Fix login](cohesion://document/3/42).",
        ),
        (
            "user",
            "Forwarded task [Fix login](cohesion://task/3/42).\n\nAnd more",
        ),
        ("agent", "Forwarded task [Fix login](cohesion://task/3/42)."),
        (
            "user",
            "Forwarded task #42 “X” in workspace 3. Read it with the code tool: `cohesion.workspace(4).task(42)`.",
        ),
    ];
    for (sender, text) in sent {
        db.connection()
            .execute(
                "INSERT INTO intelligence_agent_messages (agent_id, sender, message_content)
                 VALUES (1, ?1, json_object('kind', 'text', 'text', ?2))",
                (sender, text),
            )
            .unwrap();
    }
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let messages: Vec<String> = WriteContainer::new(&db)
        .select::<IntelligenceAgentMessage>()
        .order("id")
        .all()
        .unwrap()
        .into_iter()
        .map(|message| match message.message_content {
            AgentMessageContent::Forward { note, item } => format!(
                "{note}|{:?} {}/{} {}",
                item.kind, item.workspace_id, item.id, item.title
            ),
            AgentMessageContent::Text { .. } => "text".to_string(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        messages,
        [
            "|Task 3/42 Fix login",
            "Look at this\n\nand this|Document 3/7 A [b] c\\d",
            "|Journal 3/57 2026-10-20",
            "Note|Task 3/42 Fix “login”",
            "|Journal 3/57 2026-10-20",
            "text",
            "text",
            "text",
            "text",
        ]
    );
}

#[test]
fn security_keeps_existing_clients_and_defaults_to_allowing_what_it_did() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "security")
        .unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch("INSERT INTO clients (id, key, client_name) VALUES (1, 'k', 'Phone')")
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let db = WriteContainer::new(&db);
    let client = db.select::<Client>().one().unwrap().unwrap();
    assert_eq!(client.client_name, "Phone");
    assert_eq!(
        (client.created_at, client.last_seen_at, client.last_address),
        (None, None, None)
    );
    let settings = db.select::<SecuritySettings>().one().unwrap().unwrap();
    assert_eq!(settings.pairing_opened_by, PairingOpener::AnyClient);
    assert!(!settings.alert_new_client);
    assert_eq!(settings.revoke_inactive_after_days, None);
    // There is only ever the one row.
    assert!(
        db.container()
            .connection()
            .execute_batch("INSERT INTO security_settings (id) VALUES (2)")
            .is_err()
    );
}

#[test]
fn dropping_the_network_controls_keeps_the_other_choices_and_what_still_means_something() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "security_without_networks")
        .unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "UPDATE security_settings SET connect_from = 'allowedNetworks',
                 pair_from = 'thisComputer', pairing_opened_by = 'anyClient',
                 alert_new_client = TRUE, alert_new_address = TRUE,
                 revoke_inactive_after_days = 30;
             INSERT INTO allowed_networks (network, label) VALUES ('10.0.0.0/24', 'Home');
             INSERT INTO security_events (kind, address) VALUES
                 ('pairingOpened', '10.0.0.1'), ('newAddress', '10.0.0.1'),
                 ('connectionRefused', '203.0.113.9'), ('pairingRefused', '203.0.113.9'),
                 ('addressBlocked', '203.0.113.9'), ('settingsChanged', NULL);",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let db = WriteContainer::new(&db);
    let settings = db.select::<SecuritySettings>().one().unwrap().unwrap();
    // Pairing kept to this computer by address is kept to its clients.
    assert_eq!(settings.pairing_opened_by, PairingOpener::LocalClients);
    assert!(settings.alert_new_client);
    assert_eq!(settings.revoke_inactive_after_days, Some(30));
    let kinds: Vec<SecurityEventKind> = db
        .select::<SecurityEvent>()
        .order("id")
        .all()
        .unwrap()
        .into_iter()
        .map(|event| event.kind)
        .collect();
    assert_eq!(
        kinds,
        [
            SecurityEventKind::PairingOpened,
            SecurityEventKind::PairingRefused,
            SecurityEventKind::SettingsChanged,
        ]
    );
    assert!(
        db.container()
            .connection()
            .execute_batch("SELECT 1 FROM allowed_networks")
            .is_err()
    );
}

#[test]
fn a_tasks_follower_moves_into_its_column() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "actor_task_follower_column")
        .unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO intelligence_agents (id, agent_name, agent_color) VALUES (1, 'Scout', 'blue');
             INSERT INTO clients (id, key, client_name) VALUES (1, 'k', 'Mac');
             INSERT INTO actors (id, client_id, identifier, product, scope, modes, models)
                 VALUES (1, 1, 'code', 'Claude Code', '/src', '[]', '[]');
             INSERT INTO actor_tasks (id, actor_id, title) VALUES (1, 1, 'Fix'), (2, 1, 'Ship');
             INSERT INTO actor_task_followers (task_id, agent_id) VALUES (1, 1);",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let followers: Vec<(i64, Option<i64>)> = WriteContainer::new(&db)
        .select::<ActorSession>()
        .order("id")
        .all()
        .unwrap()
        .into_iter()
        .map(|task| (task.id, task.follower.map(|follower| follower.agent_id)))
        .collect();
    assert_eq!(followers, [(1, Some(1)), (2, None)]);
}

#[test]
fn proposals_go_and_a_reply_quoting_one_stands_alone() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "proposals_removed")
        .unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO intelligence_agents (id, agent_name, agent_color) VALUES (1, 'Scout', 'blue');
             INSERT INTO intelligence_agent_messages (id, agent_id, sender, message_content) VALUES
                 (1, 1, 'backgroundAgent', '{\"kind\":\"proposal\",\"action\":\"File it\",\"justification\":\"\",\"code\":\"\",\"status\":\"pending\",\"result\":null,\"error\":null}'),
                 (2, 1, 'agent', '{\"kind\":\"text\",\"text\":\"Hi\"}');
             INSERT INTO intelligence_agent_messages (id, agent_id, sender, message_content, reply_to)
                 VALUES (3, 1, 'user', '{\"kind\":\"text\",\"text\":\"Yes\"}', 1);",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let left: Vec<(i64, Option<i64>)> = WriteContainer::new(&db)
        .select::<IntelligenceAgentMessage>()
        .order("id")
        .all()
        .unwrap()
        .into_iter()
        .map(|message| (message.id, message.reply_to))
        .collect();
    assert_eq!(left, [(2, None), (3, None)]);
}

#[test]
fn storage_settings_start_at_the_old_constants_in_one_row() {
    let db = migrated();
    let read = WriteContainer::new(&db);
    let settings = read.select::<StorageSettings>().one().unwrap().unwrap();
    assert_eq!(
        (
            settings.thread_retention_days,
            settings.actor_session_retention_days,
            settings.memory_limit,
        ),
        (14, 7, 20)
    );
    assert!(
        db.connection()
            .execute_batch("INSERT INTO storage_settings (id) VALUES (2)")
            .is_err()
    );
}

#[test]
fn a_contacts_methods_become_rows_that_change_the_contact() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "contact_methods")
        .unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO workspaces (id, title) VALUES (1, 'w');
             INSERT INTO contacts (id, workspace_id, first_name, last_name, methods, deleted)
                 VALUES (1, 1, 'Ada', 'Lovelace',
                     '[{\"kind\":\"phone\",\"value\":\"1\"},{\"kind\":\"email\",\"value\":\"a@b\"}]',
                     FALSE),
                        (2, 1, 'Charles', 'Babbage', '[]', TRUE);",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    // Each method is a row, in its order, and the deleted contact is kept.
    let methods: Vec<(i64, String, String)> = db
        .connection()
        .prepare("SELECT contact_id, kind, value FROM contact_methods ORDER BY id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        methods,
        [
            (1, "phone".to_string(), "1".to_string()),
            (1, "email".to_string(), "a@b".to_string())
        ]
    );
    let deleted: bool = db
        .connection()
        .query_row("SELECT deleted FROM contacts WHERE id = 2", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(deleted);

    // Adding, changing or removing a method changes its contact.
    for change in [
        "INSERT INTO contact_methods (contact_id, kind, value) VALUES (1, 'email', 'c@d')",
        "UPDATE contact_methods SET value = '2' WHERE id = 1",
        "DELETE FROM contact_methods WHERE id = 1",
    ] {
        db.connection()
            .execute("UPDATE contacts SET modified = 0 WHERE id = 1", [])
            .unwrap();
        db.connection().execute(change, []).unwrap();
        let modified: i64 = db
            .connection()
            .query_row("SELECT modified FROM contacts WHERE id = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(modified > 0, "{change}");
    }
}

#[test]
fn the_profile_is_one_row_with_a_name_and_bio_never_empty_and_a_photo_set_whole() {
    let db = migrated();
    let read = WriteContainer::new(&db);
    let profile = read.select::<Profile>().one().unwrap().unwrap();
    assert_eq!(
        (profile.name, profile.bio, profile.photo),
        (None, None, Photo::None)
    );
    for refused in [
        "UPDATE profile SET name = ''",
        "UPDATE profile SET bio = ''",
        "INSERT INTO profile (id) VALUES (2)",
        "UPDATE profile SET photo_data = 'AAAA'",
        "UPDATE profile SET photo_media_type = 'image/png'",
    ] {
        assert!(db.connection().execute_batch(refused).is_err());
    }
}

#[test]
fn thread_messages_leave_json_and_threads_share_their_prompts() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "thread_columns_prompts_and_forks")
        .unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            r#"INSERT INTO intelligence_threads (id, system_prompt, remote_id, remote_cursor)
                 VALUES (1, 'Same', 'resp', 2), (2, 'Same', 'orphan', NULL), (3, 'Other', NULL, NULL);
             INSERT INTO intelligence_thread_messages (id, thread_id, description, kind, payload) VALUES
                 (1, 1, 'Hi', 'user-message', '{"UserMessage":{"text":"Hi","images":[{"media_type":"image/png","data":"AAAA"}]}}'),
                 (2, 1, 'Yes', 'assistant-message', '{"AssistantMessage":{"text":"Yes"}}'),
                 (3, 1, 'Hm', 'thinking', '{"Thinking":{"Standard":{"thought":"Hm","signature":"sig","in_content":true}}}'),
                 (4, 1, '', 'thinking', '{"Thinking":{"Redacted":{"trace":"opaque","summary":null}}}'),
                 (5, 1, '', 'tool-call-request', '{"ToolCallRequest":{"call_identifier":"c","tool_name":"code","arguments":"{}"}}'),
                 (6, 1, '', 'tool-call-response', '{"ToolCallResponse":{"call_identifier":"c","response":{"Failure":"boom"}}}'),
                 (7, 1, '', 'tool-call-response', '{"ToolCallResponse":{"call_identifier":"d","response":{"Success":"ok"}}}');"#,
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let db = WriteContainer::new(&db);
    let threads: Vec<IntelligenceThread> =
        db.select().order("intelligence_threads.id").all().unwrap();
    assert_eq!(threads[0].system_prompt, "Same");
    assert_eq!(threads[0].prompt_id, threads[1].prompt_id);
    assert_ne!(threads[0].prompt_id, threads[2].prompt_id);
    assert_eq!(threads[0].remote.as_ref().unwrap().remote_cursor, 2);
    assert_eq!(threads[1].remote, None);
    assert_eq!(threads[0].origin, ThreadOrigin::Root);

    let messages: Vec<IntelligenceThreadMessage> = db.select().order("id").all().unwrap();
    let items: Vec<ThreadItem> = messages.into_iter().map(|message| message.item).collect();
    assert_eq!(
        items,
        [
            ThreadItem::UserMessage {
                text: "Hi".to_string()
            },
            ThreadItem::AssistantMessage {
                text: "Yes".to_string()
            },
            ThreadItem::Thought {
                text: "Hm".to_string(),
                signature: Some("sig".to_string()),
                in_content: true
            },
            ThreadItem::RedactedThought {
                trace: "opaque".to_string(),
                summary: None
            },
            ThreadItem::ToolCallRequest {
                call_identifier: "c".to_string(),
                tool_name: "code".to_string(),
                arguments: "{}".to_string()
            },
            ThreadItem::ToolCallResponse {
                call_identifier: "c".to_string(),
                succeeded: false,
                text: "boom".to_string()
            },
            ThreadItem::ToolCallResponse {
                call_identifier: "d".to_string(),
                succeeded: true,
                text: "ok".to_string()
            },
        ]
    );
    let image: IntelligenceThreadImage = db.select().one().unwrap().unwrap();
    assert_eq!(
        (image.message_id, image.data.to_base64().as_str()),
        (1, "AAAA")
    );
}

#[test]
fn agent_and_actor_messages_leave_json_and_images_become_bytes() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "message_columns_and_attachments")
        .unwrap();
    db.register_migration_functions().unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            r#"INSERT INTO intelligence_agents (id, agent_name, agent_color) VALUES (1, 'Ada', 'blue');
             INSERT INTO intelligence_agent_messages (id, agent_id, sender, message_content, attachment_count) VALUES
                 (1, 1, 'user', '{"kind":"text","text":"Hi"}', 1),
                 (2, 1, 'agent', '{"kind":"codeExecution","title":"T","code":"C","outcome":{"status":"failed","error":"E"}}', 0),
                 (3, 1, 'agent', '{"kind":"choice","question":"Q","options":["A","B"],"context":null,"selected":1}', 0),
                 (4, 1, 'user', '{"kind":"forward","note":"","item":{"kind":"journal","workspace_id":2,"id":3,"title":"Day"}}', 0),
                 (5, 1, 'agent', '{"kind":"askedAgent","agent":{"id":9,"name":"Bo"},"question":"Q","reply":{"status":"unanswered","reason":"R"}}', 0),
                 (6, 1, 'backgroundAgent', '{"kind":"woke","by":{"kind":"assignedTask","workspace_id":2,"task_id":4,"title":"Fix"}}', 0),
                 (7, 1, 'agent', '{"kind":"compressedContext"}', 0);
             INSERT INTO intelligence_agent_attachments (message_id, media_type, data)
                 VALUES (1, 'image/png', 'AAAA');
             INSERT INTO clients (id, key, client_name) VALUES (1, 'k', 'c');
             INSERT INTO actors (id, client_id, identifier, product, scope, modes, models)
                 VALUES (1, 1, 'a', 'p', 's', '[]', '[]');
             INSERT INTO actor_tasks (id, actor_id, title) VALUES (1, 1, 't');
             INSERT INTO actor_task_messages (id, task_id, content, attachment_count) VALUES
                 (1, 1, '{"sender":"actor","kind":"toolUse","tool":"Edit","summary":"s","ended_at":null}', 1),
                 (2, 1, '{"sender":"actor","kind":"question","questions":[{"header":null,"question":"Q","options":[],"multi_select":false,"allows_other":true}]}', 0),
                 (3, 1, '{"sender":"user","kind":"answers","request_id":2,"answers":[{"selected":[],"other":"O"}]}', 0),
                 (4, 1, '{"sender":"actor","kind":"permission","subject":{"kind":"tool","name":"n","input":{"a":1}},"reason":null,"decisions":["allow"]}', 0),
                 (5, 1, '{"sender":"user","kind":"permission","request_id":4,"decision":"allow","message":null}', 0),
                 (6, 1, '{"sender":"actor","kind":"plan","plan":"P"}', 0),
                 (7, 1, '{"sender":"user","kind":"plan","request_id":6,"approved":false,"feedback":"F"}', 0);
             INSERT INTO actor_task_attachments (message_id, media_type, data)
                 VALUES (1, 'image/png', 'BBBB'), (1, 'image/png', 'not base64!');"#,
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let db = WriteContainer::new(&db);
    let agent: Vec<IntelligenceAgentMessage> = db.select().order("id").all().unwrap();
    assert_eq!(agent[0].attachment_count, 1);
    assert!(matches!(
        &agent[1].message_content,
        AgentMessageContent::CodeExecution(run)
            if matches!(&run.outcome, AgentCodeOutcome::Failed { error } if error == "E")
    ));
    assert!(matches!(
        &agent[2].message_content,
        AgentMessageContent::Choice(choice)
            if choice.options == ["A", "B"] && choice.selected == Some(1)
    ));
    assert!(matches!(
        &agent[3].message_content,
        AgentMessageContent::Forward { item, .. } if item.id == 3 && item.title == "Day"
    ));
    assert_eq!(agent[3].excerpt, "Day");
    assert_eq!(agent[4].excerpt, "Asked Bo: Q");
    assert_eq!(agent[5].excerpt, "“Fix” was assigned to you");
    assert_eq!(agent[6].excerpt, "Context rebuilt");

    let actor: Vec<ActorSessionMessage> = db.select().order("id").all().unwrap();
    assert_eq!(
        actor[0].attachment_count, 1,
        "an undecodable image is dropped"
    );
    assert!(matches!(
        &actor[0].content,
        ActorSessionMessageContent::Actor(ActorMessageContent::ToolUse {
            category: ActorToolKind::Other,
            ..
        })
    ));
    assert!(actor[1].answered);
    assert!(matches!(
        &actor[3].content,
        ActorSessionMessageContent::Actor(ActorMessageContent::Permission(request))
            if request.decisions == [ActorPermissionDecision::Allow]
    ));
    assert!(matches!(
        &actor[6].content,
        ActorSessionMessageContent::User(ActorUserContent::Plan { approved: false, feedback: Some(feedback), .. })
            if feedback == "F"
    ));
}

#[test]
fn rewinds_two_tables_become_one_and_cascade_with_their_item() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "unified_rewind")
        .unwrap();
    db.register_migration_functions().unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO workspaces (id, title) VALUES (1, 'w');
             INSERT INTO tasks (id, workspace_id, title) VALUES (1, 1, 'Plan');
             INSERT INTO documents (id, workspace_id, title, contents) VALUES (1, 1, 'Doc', 'Text');
             UPDATE tasks SET title = 'Plan B' WHERE id = 1;",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let view = WriteContainer::new(&db);
    let versions: Vec<Version> = view.select().order("id").all().unwrap();
    assert_eq!(versions.len(), 2);
    assert!(matches!(
        versions[0].item,
        VersionedItem::Task { task_id: 1, .. }
    ));
    assert_eq!(versions[0].title, "Plan B");
    assert_eq!(versions[1].item, VersionedItem::Document { document_id: 1 });
    assert_eq!(versions[1].body, "Text");

    let both = db.connection().execute(
        "INSERT INTO versions (task_id, document_id, workspace_id, title, body, status, change, author)
         VALUES (1, 1, 1, '', '', 'incomplete', 'edited', 'user')",
        [],
    );
    assert!(both.is_err(), "a version is of one item");
    let document_with_status = db.connection().execute(
        "INSERT INTO versions (document_id, workspace_id, title, body, status, change, author)
         VALUES (1, 1, '', '', 'incomplete', 'edited', 'user')",
        [],
    );
    assert!(document_with_status.is_err(), "only a task keeps a status");

    db.connection()
        .execute("DELETE FROM documents WHERE id = 1", [])
        .unwrap();
    let left: i64 = db
        .connection()
        .query_row("SELECT COUNT(*) FROM versions", [], |row| row.get(0))
        .unwrap();
    assert_eq!(left, 1, "a document's versions go with it");
}

#[test]
fn polymorphic_references_become_foreign_keys_held_to_a_workspace() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "typed_references")
        .unwrap();
    db.register_migration_functions().unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO workspaces (id, title) VALUES (1, 'One'), (2, 'Two');
             INSERT INTO tasks (id, workspace_id, title) VALUES (1, 1, 'T'), (2, 2, 'Other');
             INSERT INTO articles (id, url, title, snapshot) VALUES (1, 'u', 'A',
                 '{\"state\":\"captured\",\"final_url\":\"u\",\"fetched_at\":1,\"truncated\":false}');
             INSERT INTO workspace_articles (id, workspace_id, article_id) VALUES (1, 1, 1);
             INSERT INTO tags (id, workspace_id, entity, name)
                 VALUES (1, 1, 'task', 'a'), (2, 1, 'article', 'b'), (3, 2, 'task', 'c');
             INSERT INTO tagged (tag_id, entity_id) VALUES (1, 1), (2, 1);
             INSERT INTO metadata (entity_kind, entity_id, key, value)
                 VALUES ('tasks', 1, 'k', 'v'), ('tasks', 99, 'orphan', 'v');
             INSERT INTO extensions (id, name, version, script, script_identifier)
                 VALUES (1, 'e', 0, '', 'com.x');
             INSERT INTO intelligence_agents (id, agent_name, agent_color) VALUES (1, 'A', 'blue');
             INSERT INTO storage_entries (sandbox, key, value)
                 VALUES ('extension:com.x', 'k', '1'), ('agent:1', 'k', '2'),
                        ('extension:gone', 'k', '3');
             INSERT INTO saved_task_views (id, workspace_id, name, list, sort, descending,
                     priorities, sizes)
                 VALUES (1, 1, 'V', 'inbox', 'created', 0, '[\"high\",\"low\"]', '[3]');",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let count = |sql: &str| -> i64 {
        db.connection()
            .query_row(sql, [], |row| row.get(0))
            .unwrap()
    };
    assert_eq!(
        count("SELECT COUNT(*) FROM task_tags WHERE tag_id = 1 AND task_id = 1"),
        1
    );
    assert_eq!(
        count("SELECT COUNT(*) FROM metadata"),
        1,
        "an orphan doesn't survive"
    );
    assert_eq!(
        count("SELECT COUNT(*) FROM storage_entries"),
        2,
        "nor does an orphaned store"
    );

    let view = WriteContainer::new(&db);
    let saved: SavedTaskView = view.select().one().unwrap().unwrap();
    assert_eq!(saved.priorities.0, [TaskPriority::Low, TaskPriority::High]);
    assert_eq!(saved.sizes.0.len(), 1);

    // A tag can't reach across workspaces.
    let refused = |sql: &str| db.connection().execute(sql, []).is_err();
    assert!(refused(
        "INSERT INTO task_tags (workspace_id, tag_id, task_id) VALUES (1, 3, 1)"
    ));
    assert!(refused(
        "INSERT INTO task_tags (workspace_id, tag_id, task_id) VALUES (2, 3, 1)"
    ));

    // Deleting a task takes its tags.
    db.connection()
        .execute_batch("DELETE FROM tasks WHERE id = 1;")
        .unwrap();
    assert_eq!(count("SELECT COUNT(*) FROM task_tags"), 0);
}

#[test]
fn actor_tasks_become_actor_sessions_and_keep_their_rows_links_and_history() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "actor_tasks_become_actor_sessions")
        .unwrap();
    db.register_migration_functions().unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO clients (id, key, client_name) VALUES (1, 'k', 'Mac');
             INSERT INTO actors (id, client_id, identifier, product, scope, modes, models)
                 VALUES (1, 1, 'code', 'Claude Code', '/src', '[]', '[]');
             INSERT INTO actor_tasks (id, actor_id, title) VALUES (1, 1, 'Fix the build');
             INSERT INTO actor_task_messages (id, task_id, sender, kind, text)
                 VALUES (1, 1, 'actor', 'text', 'See cohesion://actor-task/1 for it');
             INSERT INTO activities (id, title, actor_task_id) VALUES (1, 'Fix', 1);
             INSERT INTO intelligence_agents (id, agent_name, agent_color, context_tokens)
                 VALUES (1, 'Ada', 'blue', 50);
             INSERT INTO intelligence_agent_messages (id, agent_id, sender, kind, wake, actor_task_id)
                 VALUES (1, 1, 'agent', 'woke', 'actorTask', 1);
             INSERT INTO workspaces (id, title) VALUES (1, 'Home');
             INSERT INTO tasks (id, workspace_id, title, description)
                 VALUES (1, 1, 'Follow up', 'From [the fix](cohesion://actor-task/1), not cohesion://task/1/1.');
             UPDATE storage_settings SET actor_task_retention_days = 3;",
        )
        .unwrap();
    let versions = |db: &Container| -> i64 {
        db.connection()
            .query_row("SELECT count(*) FROM versions", [], |row| row.get(0))
            .unwrap()
    };
    let versions_before = versions(&db);
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let text = |sql: &str| -> String {
        db.connection()
            .query_row(sql, [], |row| row.get(0))
            .unwrap()
    };
    let number = |sql: &str| -> i64 {
        db.connection()
            .query_row(sql, [], |row| row.get(0))
            .unwrap()
    };
    // The session is the row it was, its retention carried over and its search kept.
    assert_eq!(
        text("SELECT title FROM actor_sessions WHERE id = 1"),
        "Fix the build"
    );
    assert_eq!(
        number("SELECT actor_session_retention_days FROM storage_settings"),
        3
    );
    assert_eq!(
        number("SELECT count(*) FROM actor_sessions_fts WHERE actor_sessions_fts MATCH 'build'"),
        1
    );
    // Links move, and only actor sessions' links; Rewind doesn't count that as an edit.
    assert_eq!(
        text("SELECT description FROM tasks WHERE id = 1"),
        "From [the fix](cohesion://actor-session/1), not cohesion://task/1/1."
    );
    assert_eq!(
        text("SELECT text FROM actor_session_messages WHERE id = 1"),
        "See cohesion://actor-session/1 for it"
    );
    assert_eq!(versions(&db), versions_before);
    // The wake's word moves past its check, and no thread carries the old API on.
    assert_eq!(
        text("SELECT wake FROM intelligence_agent_messages WHERE actor_session_id = 1"),
        "actorSession"
    );
    assert_eq!(
        number("SELECT count(*) FROM intelligence_agents WHERE context_tokens IS NULL"),
        1
    );
    // The triggers came back under the new names, as the rules they were.
    db.connection()
        .execute_batch("UPDATE tasks SET description = 'New' WHERE id = 1")
        .unwrap();
    assert_eq!(
        text("SELECT body FROM versions WHERE task_id = 1 ORDER BY id DESC LIMIT 1"),
        "New"
    );
    assert!(
        db.connection()
            .execute_batch("UPDATE actor_sessions SET status = 'nonsense' WHERE id = 1")
            .is_err()
    );
    db.connection()
        .execute_batch("DELETE FROM actor_sessions WHERE id = 1")
        .unwrap();
    assert_eq!(number("SELECT count(*) FROM actor_session_messages"), 0);
    assert_eq!(number("SELECT count(*) FROM activities"), 0);
}

#[test]
fn dropping_a_feeds_parser_keeps_the_feed() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "feeds_drop_parser")
        .unwrap();
    db.register_migration_functions().unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO feeds (id, url, title, sort_order, full_page_policy, parser,
                    status_state, status_at, failure_kind, failure_text)
                VALUES (1, 'https://a.example/feed', 'A', 2, 'always', 'readability',
                    'failed', 5, 'unreachable', 'down');
             INSERT INTO articles (id, feed_id, url, title, contents, snapshot_state, snapshot_at,
                    final_url, truncated)
                VALUES (1, 1, 'https://a.example/1', 'One', 'Text', 'captured', 5,
                    'https://a.example/1', FALSE);",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let feed = WriteContainer::new(&db)
        .select::<Feed>()
        .filter_eq(Feed::ID, 1)
        .one()
        .unwrap()
        .unwrap();
    assert_eq!(feed.url, "https://a.example/feed");
    assert_eq!(feed.full_page_policy, FeedFullPagePolicy::Always);
    assert!(matches!(feed.status, FeedStatus::Failed { at: 5, .. }));
    let articles: i64 = db
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM articles WHERE feed_id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(articles, 1);
}

// Marks take the capacity set when the daemon deploys, so it rebuilds no thread.
#[test]
fn thread_marks_take_the_memory_capacity_set_at_the_migration() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "agent_thread_mark_memory_limit")
        .unwrap();
    db.register_migration_functions().unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "UPDATE storage_settings SET memory_limit = 35;
             INSERT INTO intelligence_agents (id, agent_name, agent_color)
                VALUES (1, 'Scout', 'blue');
             INSERT INTO thread_prompts (id, prompt) VALUES (1, 'You are Scout.');
             INSERT INTO intelligence_threads (id, prompt_id) VALUES (1, 1);
             INSERT INTO intelligence_endpoints (id, name, provider_identifier, dialect, base_url,
                    cache_minutes)
                VALUES (1, 'Local', 'local', 'messages', 'http://localhost', 5);
             INSERT INTO endpoint_models (id, endpoint_id, model_identifier)
                VALUES (1, 1, 'model');
             INSERT INTO agent_thread_marks (id, agent_id, thread_id, items, endpoint_model_id,
                    heard_through, memories, last_call_at, prompt_tokens, cache_seconds,
                    extended_from)
                VALUES (1, 1, 1, 4, 1, 9, '[]', 1000, 1200, 3600, 2);",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let mark = WriteContainer::new(&db)
        .select::<AgentThreadMark>()
        .one()
        .unwrap()
        .unwrap();
    assert_eq!(mark.memory_limit, 35);
    assert_eq!(
        (
            mark.agent_id,
            mark.thread_id,
            mark.items,
            mark.extended_from
        ),
        (1, 1, 4, 2)
    );
    assert_eq!(
        (mark.heard_through, mark.last_call_at, mark.cache_seconds),
        (9, 1000, 3600)
    );
}

#[test]
fn articles_only_from_feeds_drops_saving_and_keeps_the_reader_and_task_tags() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "articles_only_from_feeds")
        .unwrap();
    db.register_migration_functions().unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO workspaces (id, title) VALUES (1, 'One'), (2, 'Two');
             INSERT INTO feeds (id, url, title, status_state, status_at, added)
                 VALUES (1, 'https://a.example/feed', 'A', 'refreshed', 1, 0);
             INSERT INTO articles (id, feed_id, read, url, title, contents, snapshot_state,
                     snapshot_at, final_url, truncated)
                 VALUES (1, 1, TRUE, 'https://a.example/1', 'Orchard', 'apples', 'captured', 1,
                     'https://a.example/1', FALSE),
                 (2, NULL, FALSE, 'https://b.example/1', 'Hand', 'hand', 'captured', 1,
                     'https://b.example/1', FALSE);
             INSERT INTO article_folders (id, workspace_id, title) VALUES (1, 1, 'Folder');
             INSERT INTO workspace_articles (workspace_id, article_id, article_folder_id)
                 VALUES (1, 1, 1), (1, 2, NULL);
             INSERT INTO tags (id, workspace_id, entity, name)
                 VALUES (1, 1, 'task', 'urgent'), (2, 1, 'article', 'urgent'),
                     (3, 2, 'task', 'other');
             INSERT INTO article_tags (workspace_id, tag_id, article_id) VALUES (1, 2, 1);
             INSERT INTO tasks (id, workspace_id, title) VALUES (1, 1, 'T');
             INSERT INTO task_tags (workspace_id, tag_id, task_id) VALUES (1, 1, 1);
             INSERT INTO saved_task_views (id, workspace_id, name, list, sort, descending)
                 VALUES (1, 1, 'V', 'inbox', 'created', 0);
             INSERT INTO saved_task_view_tags (workspace_id, view_id, tag_id, tag_match)
                 VALUES (1, 1, 1, 'with');
             INSERT INTO extensions (id, name, version, script, script_identifier)
                 VALUES (1, 'e', 0, '', 'com.x');
             INSERT INTO extension_hooks (extension_id, hook)
                 VALUES (1, 'article:save'), (1, 'article:tagged'), (1, 'article:read');",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let count = |sql: &str| -> i64 {
        db.connection()
            .query_row(sql, [], |row| row.get(0))
            .unwrap()
    };
    assert_eq!(
        count(
            "SELECT COUNT(*) FROM sqlite_master WHERE name IN
                ('workspace_articles', 'article_folders', 'article_tags')"
        ),
        0
    );

    // The reader keeps what a feed brought in, read state and search included;
    // a page saved by hand goes.
    let articles: Vec<Article> = WriteContainer::new(&db).select::<Article>().all().unwrap();
    assert_eq!(articles.len(), 1);
    assert_eq!((articles[0].feed_id, articles[0].read), (1, true));
    let corpus =
        || count("SELECT COUNT(*) FROM feed_corpus_fts WHERE feed_corpus_fts MATCH 'apples'");
    assert_eq!(corpus(), 1);
    assert_eq!(
        count("SELECT COUNT(*) FROM search_entries WHERE entity_kind = 'article'"),
        0
    );
    db.connection()
        .execute_batch("UPDATE articles SET deleted = TRUE WHERE id = 1")
        .unwrap();
    assert_eq!(corpus(), 0);
    db.connection()
        .execute_batch("UPDATE articles SET deleted = FALSE WHERE id = 1")
        .unwrap();
    assert_eq!(corpus(), 1);
    assert!(
        db.connection()
            .execute(
                "INSERT INTO articles (url, title, snapshot_state, snapshot_at, failure_kind)
                 VALUES ('u', 't', 'failed', 1, 'tooLarge')",
                [],
            )
            .is_err(),
        "an article comes from a feed"
    );

    // Hooks on what's gone go; the rest stay.
    assert_eq!(count("SELECT COUNT(*) FROM extension_hooks"), 1);

    // Task tags come through, one name per workspace, held to it.
    let names: Vec<String> = WriteContainer::new(&db)
        .select::<Tag>()
        .order("id")
        .all()
        .unwrap()
        .into_iter()
        .map(|tag| tag.name)
        .collect();
    assert_eq!(names, ["urgent", "other"]);
    let refused = |sql: &str| db.connection().execute(sql, []).is_err();
    assert!(refused(
        "INSERT INTO tags (workspace_id, name) VALUES (1, 'URGENT')"
    ));
    assert!(refused(
        "INSERT INTO task_tags (workspace_id, tag_id, task_id) VALUES (1, 3, 1)"
    ));
    assert!(refused(
        "INSERT INTO saved_task_view_tags (workspace_id, view_id, tag_id, tag_match)
         VALUES (1, 1, 3, 'with')"
    ));

    // Deleting a tag still takes it off its tasks and views.
    assert_eq!(count("SELECT COUNT(*) FROM task_tags"), 1);
    assert_eq!(count("SELECT COUNT(*) FROM saved_task_view_tags"), 1);
    db.connection()
        .execute_batch("DELETE FROM tags WHERE id = 1")
        .unwrap();
    assert_eq!(count("SELECT COUNT(*) FROM task_tags"), 0);
    assert_eq!(count("SELECT COUNT(*) FROM saved_task_view_tags"), 0);

    // Deleting a feed's row takes its articles.
    db.connection()
        .execute_batch("DELETE FROM feeds WHERE id = 1")
        .unwrap();
    assert_eq!(count("SELECT COUNT(*) FROM articles"), 0);
}

#[test]
fn a_sessions_activities_become_its_first_holding_what_the_newest_says() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "one_activity_per_actor_session")
        .unwrap();
    db.register_migration_functions().unwrap();
    // As `migrate` runs them: the later ones rebuild tables with foreign keys off.
    db.connection()
        .pragma_update(None, "foreign_keys", false)
        .unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO clients (id, key, client_name) VALUES (1, 'k', 'Mac');
             INSERT INTO actors (id, client_id, identifier, product, scope, modes, models)
                 VALUES (1, 1, 'code', 'Claude Code', '/src', '[]', '[]');
             INSERT INTO actor_sessions (id, actor_id, title)
                 VALUES (1, 1, 'Fix'), (2, 1, 'Ship'), (3, 1, 'Read');
             INSERT INTO activities (id, title, actor_session_id, status, progress, started_at,
                     ended_at) VALUES
                 (1, 'Fix', 1, 'succeeded', NULL, 10, 20),
                 (2, 'Fix it', 1, 'failed', NULL, 30, 40),
                 (3, 'Fix it now', 1, 'running', 'Building', 50, NULL),
                 (4, 'Ship', 2, 'cancelled', NULL, 60, 70),
                 (5, 'Read', 3, 'cancelled', NULL, 80, 90),
                 (6, 'Read more', 3, 'succeeded', NULL, 100, 110);",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let kept: Vec<_> = WriteContainer::new(&db)
        .select::<Activity>()
        .order("id")
        .all()
        .unwrap()
        .into_iter()
        .map(|activity| {
            (
                activity.id,
                activity.title,
                activity.status,
                activity.progress,
                activity.started_at,
                activity.ended_at,
            )
        })
        .collect();
    assert_eq!(
        kept,
        [
            (
                1,
                "Fix it now".to_string(),
                ActivityStatus::Running,
                Some("Building".to_string()),
                10,
                None
            ),
            (
                4,
                "Ship".to_string(),
                ActivityStatus::Cancelled,
                None,
                60,
                Some(70)
            ),
            (
                5,
                "Read more".to_string(),
                ActivityStatus::Succeeded,
                None,
                80,
                Some(110)
            ),
        ]
    );
    let unread = |session: i64| -> bool {
        db.connection()
            .query_row(
                "SELECT unread FROM actor_sessions WHERE id = ?",
                [session],
                |row| row.get(0),
            )
            .unwrap()
    };
    // Merging marks no session unread.
    assert!(!unread(3));
    // One activity per session, open or not.
    assert!(
        db.connection()
            .execute_batch(
                "INSERT INTO activities (title, actor_session_id, status, ended_at)
                 VALUES ('Ship', 2, 'succeeded', 5)"
            )
            .is_err()
    );
    // Ending it still marks its session unread.
    db.connection()
        .execute_batch("UPDATE activities SET status = 'succeeded', ended_at = 9 WHERE id = 1")
        .unwrap();
    assert!(unread(1));
}

#[test]
fn documents_start_in_the_inbox_and_file_only_into_a_live_folder_of_their_workspace() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "document_folders")
        .unwrap();
    // As `migrate` runs them: these later migrations rebuild tables, which
    // needs foreign keys off.
    db.connection()
        .execute_batch("PRAGMA foreign_keys = OFF")
        .unwrap();
    db.register_migration_functions().unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO workspaces (id, title) VALUES (1, 'Home'), (2, 'Work');
             INSERT INTO documents (id, workspace_id, title, contents)
                 VALUES (1, 1, 'Plan', 'text');
             INSERT INTO documents (id, workspace_id, title, contents, document_type,
                     journal_date)
                 VALUES (2, 1, '2026-11-09', '', 'journal', '2026-11-09');",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let folder_of = |id: i64| -> Option<i64> {
        db.connection()
            .query_row(
                "SELECT folder_id FROM documents WHERE id = ?",
                [id],
                |row| row.get(0),
            )
            .unwrap()
    };
    assert_eq!(folder_of(1), None);
    db.connection()
        .execute_batch(
            "INSERT INTO document_folders (id, workspace_id, title) VALUES (1, 1, 'Notes');
             INSERT INTO document_folders (id, workspace_id, parent_id, title)
                 VALUES (2, 1, 1, 'Old');
             INSERT INTO document_folders (id, workspace_id, title, deleted)
                 VALUES (3, 1, 'Gone', TRUE);
             INSERT INTO document_folders (id, workspace_id, title) VALUES (4, 2, 'Work');",
        )
        .unwrap();
    let file = |document: i64, folder: i64| {
        db.connection().execute(
            "UPDATE documents SET folder_id = ? WHERE id = ?",
            [folder, document],
        )
    };
    file(1, 2).unwrap();
    assert_eq!(folder_of(1), Some(2));
    // Another workspace's folder, a deleted one, or any folder for a journal.
    assert!(file(1, 4).is_err());
    assert!(file(1, 3).is_err());
    assert!(file(2, 1).is_err());
    // A folder's parent is of its own workspace, and never itself.
    let nest = |sql: &str| db.connection().execute(sql, []);
    assert!(nest("UPDATE document_folders SET parent_id = 4 WHERE id = 1").is_err());
    assert!(nest("UPDATE document_folders SET parent_id = 1 WHERE id = 1").is_err());
}

#[test]
fn backlinks_are_filled_from_every_live_documents_links() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "backlinks")
        .unwrap();
    // As `migrate` runs them: the later ones rebuild tables with foreign keys off.
    db.connection()
        .pragma_update(None, "foreign_keys", false)
        .unwrap();
    db.register_migration_functions().unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO workspaces (id, title) VALUES (3, 'Work');
             INSERT INTO tasks (id, workspace_id, title) VALUES (42, 3, 'Fix login');
             INSERT INTO documents (id, workspace_id, title, contents) VALUES
                 (7, 3, 'Notes', '[Fix](cohesion://task/3/42) and [again](cohesion://task/42)

[Me](cohesion://document/3/7), see [the plan](cohesion://document/8).

```
[Code](cohesion://agent/1)
```'),
                 (8, 3, 'Plan', 'No links.');
             INSERT INTO documents (id, workspace_id, deleted, title, contents) VALUES
                 (9, 3, TRUE, 'Gone', '[Fix](cohesion://task/3/42)');",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let db = WriteContainer::new(&db);
    let rows: Vec<(LinkKind, i64, LinkKind, i64, String)> = db
        .select::<Backlink>()
        .order("id")
        .all()
        .unwrap()
        .into_iter()
        .map(|b| {
            (
                b.source_kind,
                b.source_id,
                b.target_kind,
                b.target_id,
                b.link,
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            (
                LinkKind::Document,
                7,
                LinkKind::Task,
                42,
                "cohesion://task/3/42".to_string()
            ),
            (
                LinkKind::Document,
                7,
                LinkKind::Document,
                8,
                "cohesion://document/8".to_string()
            ),
        ]
    );

    // A document removed for good takes its backlinks with it.
    db.delete::<Document>()
        .filter_eq(Document::ID, 7)
        .execute()
        .unwrap();
    assert!(db.select::<Backlink>().all().unwrap().is_empty());
}

#[test]
fn task_backlinks_are_filled_from_every_live_tasks_description() {
    let mut db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "task_backlinks")
        .unwrap();
    db.connection()
        .pragma_update(None, "foreign_keys", false)
        .unwrap();
    db.register_migration_functions().unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO workspaces (id, title) VALUES (3, 'Work');
             INSERT INTO documents (id, workspace_id, title, contents) VALUES (8, 3, 'Plan', '');
             INSERT INTO tasks (id, workspace_id, title, description) VALUES
                 (42, 3, 'Fix login', '[The plan](cohesion://document/3/8)

See [me](cohesion://task/42) and `[code](cohesion://task/3/43)`.'),
                 (43, 3, 'Ship it', 'No links.');
             INSERT INTO tasks (id, workspace_id, deleted, title, description) VALUES
                 (44, 3, TRUE, 'Gone', '[The plan](cohesion://document/8)');",
        )
        .unwrap();
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db.migrate().unwrap();

    let db = WriteContainer::new(&db);
    let rows: Vec<(LinkKind, i64, LinkKind, i64, String)> = db
        .select::<Backlink>()
        .order("id")
        .all()
        .unwrap()
        .into_iter()
        .map(|b| {
            (
                b.source_kind,
                b.source_id,
                b.target_kind,
                b.target_id,
                b.link,
            )
        })
        .collect();
    assert_eq!(
        rows,
        [(
            LinkKind::Task,
            42,
            LinkKind::Document,
            8,
            "cohesion://document/3/8".to_string()
        )]
    );

    // A task removed for good takes its backlinks with it.
    db.delete::<Task>()
        .filter_eq(Task::ID, 42)
        .execute()
        .unwrap();
    assert!(db.select::<Backlink>().all().unwrap().is_empty());
}

#[test]
fn databases_keep_their_values_to_their_own_rows_and_properties_and_rows_take_their_documents() {
    let db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == "databases")
        .unwrap();
    // As `migrate` runs them: the later ones rebuild tables with foreign keys
    // off. `document_tables` replaced these tables, so this runs no further.
    db.connection()
        .pragma_update(None, "foreign_keys", false)
        .unwrap();
    db.register_migration_functions().unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .execute_batch(
            "INSERT INTO workspaces (id, title) VALUES (1, 'Home');
             INSERT INTO documents (id, workspace_id, title, contents) VALUES (1, 1, 'Plan', 'apples');",
        )
        .unwrap();
    db.connection().execute_batch(MIGRATIONS[at].sql).unwrap();
    db.connection()
        .pragma_update(None, "foreign_keys", true)
        .unwrap();

    // An ordinary document stays searchable; a row's document never is.
    let run = |sql: &str| db.connection().execute_batch(sql);
    run(
        "INSERT INTO databases (id, workspace_id, title) VALUES (1, 1, 'Books'), (2, 1, 'Films');
         INSERT INTO database_properties (id, database_id, name, data_type)
             VALUES (1, 1, 'Title', 'text'), (2, 2, 'Title', 'text');
         INSERT INTO documents (id, workspace_id, title, contents, document_type)
             VALUES (2, 1, 'Dune', 'apples', 'databaseRow');
         INSERT INTO database_rows (id, database_id, document_id) VALUES (1, 1, 2), (2, 2, NULL);
         INSERT INTO database_row_values (database_id, row_id, property_id, text_value)
             VALUES (1, 1, 1, 'Dune');",
    )
    .unwrap();
    assert_eq!(search(&db, "apples"), [("document".to_string(), 1)]);
    run("UPDATE documents SET contents = 'pears' WHERE id = 2").unwrap();
    assert!(search(&db, "pears").is_empty());

    // A value is of its own database's row and property, once each, always
    // with its text.
    assert!(
        run(
            "INSERT INTO database_row_values (database_id, row_id, property_id, text_value)
             VALUES (1, 1, 2, 'x')"
        )
        .is_err()
    );
    assert!(
        run(
            "INSERT INTO database_row_values (database_id, row_id, property_id, text_value)
             VALUES (1, 2, 1, 'x')"
        )
        .is_err()
    );
    assert!(
        run(
            "INSERT INTO database_row_values (database_id, row_id, property_id, text_value)
             VALUES (1, 1, 1, 'again')"
        )
        .is_err()
    );
    assert!(
        run(
            "INSERT INTO database_row_values (database_id, row_id, property_id, text_value)
             VALUES (2, 2, 2, NULL)"
        )
        .is_err()
    );
    assert!(run("UPDATE database_properties SET data_type = 'money' WHERE id = 1").is_err());

    // A row's document is never soft-deleted, and goes with its row.
    assert!(run("UPDATE documents SET deleted = TRUE WHERE id = 2").is_err());
    let count = |sql: &str| -> i64 {
        db.connection()
            .query_row(sql, [], |row| row.get(0))
            .unwrap()
    };
    run("DELETE FROM databases WHERE id = 1").unwrap();
    assert_eq!(count("SELECT count(*) FROM documents WHERE id = 2"), 0);
    assert_eq!(
        count("SELECT count(*) FROM database_rows WHERE database_id = 1"),
        0
    );
    assert_eq!(count("SELECT count(*) FROM database_row_values"), 0);
    assert_eq!(count("SELECT count(*) FROM documents WHERE id = 1"), 1);
}

/// Runs every migration before `name` on a new database, with the
/// migration functions and foreign keys off, as `migrate` runs them, and
/// returns it with its `user_version` there, to be filled and migrated on.
fn migrated_before(name: &str) -> Container {
    let db = Container::in_memory().unwrap();
    let at = MIGRATIONS
        .iter()
        .position(|migration| migration.name == name)
        .unwrap();
    db.connection()
        .pragma_update(None, "foreign_keys", false)
        .unwrap();
    db.register_migration_functions().unwrap();
    for migration in &MIGRATIONS[..at] {
        db.connection().execute_batch(migration.sql).unwrap();
    }
    db.connection()
        .pragma_update(None, "user_version", at as i64)
        .unwrap();
    db
}

#[test]
fn databases_become_document_folders_whose_rows_are_documents() {
    let mut db = migrated_before("document_tables");
    // Books: a formula first, then Pages, then Name, its first text property
    // set by hand. Its rows: one without a document named by Name, one whose
    // untitled document Name names, one whose blank Name leaves its
    // document's title, and an empty one with outdated formulas. Films: one
    // empty row with an untitled document. A document links to Books.
    db.connection()
        .execute_batch(
            "INSERT INTO workspaces (id, title) VALUES (1, 'Home');
             INSERT INTO document_folders (id, workspace_id, title, sort_order)
                 VALUES (1, 1, 'Notes', 1);
             INSERT INTO documents (id, workspace_id, title, contents)
                 VALUES (1, 1, 'Plan', '[Books](cohesion://database/1/1)');
             INSERT INTO documents (id, workspace_id, title, contents, document_type)
                 VALUES (2, 1, '', 'apples', 'databaseRow'),
                        (3, 1, 'Old notes', 'pears', 'databaseRow'),
                        (4, 1, '', '', 'databaseRow');
             INSERT INTO databases (id, workspace_id, title) VALUES (1, 1, 'films'), (2, 1, 'Books');
             INSERT INTO database_properties
                     (id, database_id, name, data_type, sort_order, formula_source, formula_error)
                 VALUES (1, 2, 'Shout', 'text', 0, '(row) => row.Name', 'TypeError: no'),
                        (2, 2, 'Pages', 'integer', 1, NULL, NULL),
                        (3, 2, 'Name', 'text', 2, NULL, NULL),
                        (4, 1, 'Title', 'text', 0, NULL, NULL);
             INSERT INTO database_rows (id, database_id, document_id, formula_stale)
                 VALUES (1, 2, NULL, 0), (2, 2, 2, 0), (3, 2, 3, 0), (4, 2, NULL, 2),
                        (5, 1, 4, 0);
             INSERT INTO database_row_values
                     (id, database_id, row_id, property_id, text_value, integer_value, real_value)
                 VALUES (1, 2, 1, 1, 'DUNE', NULL, NULL),
                        (2, 2, 1, 2, '412', 412, 412.0),
                        (3, 2, 1, 3, 'Dune', NULL, NULL),
                        (4, 2, 2, 3, 'Emma', NULL, NULL),
                        (5, 2, 3, 3, '  ', NULL, NULL);
             INSERT INTO backlinks (source_kind, source_id, target_kind, target_id, link)
                 VALUES ('document', 1, 'database', 2, 'cohesion://database/1/2'),
                        ('document', 2, 'document', 1, 'cohesion://document/1/1'),
                        ('document', 1, 'contact', 7, 'cohesion://contact/1/7');",
        )
        .unwrap();
    db.migrate().unwrap();
    db.connection()
        .pragma_update(None, "foreign_keys", true)
        .unwrap();
    let run = |sql: &str| db.connection().execute_batch(sql);
    let rows = |sql: &str| -> Vec<Vec<String>> {
        let mut statement = db.connection().prepare(sql).unwrap();
        let width = statement.column_count();
        statement
            .query_map([], |row| {
                (0..width)
                    .map(|i| {
                        row.get::<_, rusqlite::types::Value>(i)
                            .map(|value| match value {
                                rusqlite::types::Value::Null => "NULL".to_string(),
                                rusqlite::types::Value::Integer(n) => n.to_string(),
                                rusqlite::types::Value::Real(n) => n.to_string(),
                                rusqlite::types::Value::Text(text) => text,
                                rusqlite::types::Value::Blob(_) => "blob".to_string(),
                            })
                    })
                    .collect()
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };

    // Each database is a top-level folder after the others, ordered by
    // title.
    assert_eq!(
        rows("SELECT id, title, parent_id, sort_order FROM document_folders ORDER BY id"),
        [
            ["1", "Notes", "NULL", "1"],
            ["2", "films", "NULL", "3"],
            ["3", "Books", "NULL", "2"],
        ]
    );
    // Each row is an ordinary document in it, titled by the rule; rows
    // without one have new, empty ones.
    assert_eq!(
        rows(
            "SELECT id, title, contents, document_type, folder_id, formula_stale
             FROM documents ORDER BY id"
        ),
        [
            [
                "1",
                "Plan",
                "[Books](cohesion://database/1/1)",
                "normal",
                "NULL",
                "0"
            ],
            ["2", "Emma", "apples", "normal", "3", "0"],
            ["3", "Old notes", "pears", "normal", "3", "0"],
            ["4", "Untitled", "", "normal", "2", "0"],
            ["5", "Dune", "", "normal", "3", "0"],
            ["6", "Untitled", "", "normal", "3", "2"],
        ]
    );
    // Properties and values keep their ids, now by folder and document;
    // the text column that named the rows keeps its values. A formula is
    // computed when what it reads changes; a property set by hand has no
    // update mode.
    assert_eq!(
        rows(
            "SELECT id, folder_id, name, sort_order, formula_source, formula_update_mode,
                 formula_error
             FROM document_folder_properties ORDER BY id"
        ),
        [
            [
                "1",
                "3",
                "Shout",
                "0",
                "(row) => row.Name",
                "whenChanged",
                "TypeError: no"
            ],
            ["2", "3", "Pages", "1", "NULL", "NULL", "NULL"],
            ["3", "3", "Name", "2", "NULL", "NULL", "NULL"],
            ["4", "2", "Title", "0", "NULL", "NULL", "NULL"],
        ]
    );
    assert_eq!(
        rows(
            "SELECT id, folder_id, document_id, property_id, text_value, integer_value
             FROM document_values ORDER BY id"
        ),
        [
            ["1", "3", "5", "1", "DUNE", "NULL"],
            ["2", "3", "5", "2", "412", "412"],
            ["3", "3", "5", "3", "Dune", "NULL"],
            ["4", "3", "2", "3", "Emma", "NULL"],
            ["5", "3", "3", "3", "  ", "NULL"],
        ]
    );
    // The old tables are gone, and so are links to a database; a link to a
    // contact, which `contacts_search_and_links` admitted, is kept.
    assert!(
        rows(
            "SELECT name FROM sqlite_master WHERE name IN
             ('databases', 'database_properties', 'database_rows', 'database_row_values',
              'database_folders', 'database_row_documents')"
        )
        .is_empty()
    );
    assert_eq!(
        rows("SELECT source_id, target_kind, target_id FROM backlinks ORDER BY source_id"),
        [["1", "contact", "7"], ["2", "document", "1"]]
    );

    // Former rows' documents are searchable, new ones by their titles.
    assert_eq!(search(&db, "apples"), [("document".to_string(), 2)]);
    assert_eq!(search(&db, "pears"), [("document".to_string(), 3)]);
    assert_eq!(search(&db, "Dune"), [("document".to_string(), 5)]);

    // A value's document and property are of its folder: it can't name
    // another's, and a document can't leave its folder with values.
    assert!(
        run(
            "INSERT INTO document_values (folder_id, document_id, property_id, text_value)
             VALUES (3, 4, 3, 'x')"
        )
        .is_err()
    );
    assert!(
        run(
            "INSERT INTO document_values (folder_id, document_id, property_id, text_value)
             VALUES (2, 4, 3, 'x')"
        )
        .is_err()
    );
    assert!(run("UPDATE documents SET folder_id = NULL WHERE id = 5").is_err());
    run("DELETE FROM document_values WHERE document_id = 5").unwrap();
    run("UPDATE documents SET folder_id = NULL WHERE id = 5").unwrap();
    // The Inbox never has properties.
    assert!(
        run(
            "INSERT INTO document_folder_properties (folder_id, name, data_type)
             VALUES (NULL, 'Pages', 'integer')"
        )
        .is_err()
    );
    // An update mode belongs to a formula: a property set by hand can't
    // have one, a formula can't go without one, and it is one of the two.
    assert!(run("UPDATE document_folder_properties SET formula_update_mode = 'continuously' WHERE id = 2").is_err());
    assert!(
        run("UPDATE document_folder_properties SET formula_update_mode = NULL WHERE id = 1")
            .is_err()
    );
    assert!(
        run("UPDATE document_folder_properties SET formula_update_mode = 'hourly' WHERE id = 1")
            .is_err()
    );
    run("UPDATE document_folder_properties SET formula_update_mode = 'continuously' WHERE id = 1")
        .unwrap();
    // A former row's document is soft-deleted like any, and removing it
    // for good takes its values.
    run("UPDATE documents SET deleted = TRUE WHERE id = 2").unwrap();
    run("DELETE FROM documents WHERE id = 2").unwrap();
    assert_eq!(
        rows("SELECT count(*) FROM document_values WHERE document_id = 2"),
        [["0"]]
    );

    // Only a choice property has options, named uniquely ignoring case, and
    // a choice is never a formula.
    run("UPDATE document_folder_properties SET data_type = 'choice' WHERE id = 4").unwrap();
    assert!(
        run("UPDATE document_folder_properties SET data_type = 'choice' WHERE id = 1").is_err()
    );
    run("INSERT INTO document_property_options (id, property_id, name) VALUES (1, 4, 'Read')")
        .unwrap();
    run(
        "INSERT INTO document_property_options (id, property_id, name, color)
         VALUES (2, 4, 'Unread', 'blue')",
    )
    .unwrap();
    assert!(
        run("INSERT INTO document_property_options (property_id, name) VALUES (4, 'READ')")
            .is_err()
    );
    assert!(
        run("INSERT INTO document_property_options (property_id, name) VALUES (3, 'Read')")
            .is_err()
    );
    assert!(
        run(
            "INSERT INTO document_property_options (property_id, name, color)
             VALUES (4, 'Later', 'mauve')"
        )
        .is_err()
    );
    // A property with options stays a choice until they go.
    assert!(run("UPDATE document_folder_properties SET data_type = 'text' WHERE id = 4").is_err());
    // A value names an option of its own property: deleting the option
    // takes the value.
    run(
        "INSERT INTO document_values (folder_id, document_id, property_id, text_value, option_id)
         VALUES (2, 4, 4, '', 1)",
    )
    .unwrap();
    run(
        "INSERT INTO document_folder_properties (id, folder_id, name, data_type)
         VALUES (5, 2, 'Shelf', 'choice')",
    )
    .unwrap();
    assert!(
        run("UPDATE document_values SET property_id = 5 WHERE document_id = 4 AND property_id = 4")
            .is_err()
    );
    assert!(
        run("INSERT INTO document_values (folder_id, document_id, property_id, text_value, option_id)
             VALUES (2, 4, 5, '', 1)")
        .is_err()
    );
    run("DELETE FROM document_property_options WHERE id = 1").unwrap();
    assert_eq!(
        rows("SELECT count(*) FROM document_values WHERE property_id = 4"),
        [["0"]]
    );
    // Deleting the property takes its options.
    run("DELETE FROM document_folder_properties WHERE id = 4").unwrap();
    assert_eq!(
        rows("SELECT count(*) FROM document_property_options"),
        [["0"]]
    );
}

#[test]
fn folders_with_properties_become_tables_and_the_rest_lists() {
    let mut db = migrated_before("document_folder_representation");
    db.connection()
        .execute_batch(
            "INSERT INTO workspaces (id, title) VALUES (1, 'Home');
             INSERT INTO document_folders (id, workspace_id, title, sort_order)
                 VALUES (1, 1, 'Notes', 1), (2, 1, 'Books', 2);
             INSERT INTO document_folder_properties (id, folder_id, name, data_type)
                 VALUES (1, 2, 'Pages', 'integer');",
        )
        .unwrap();
    db.migrate().unwrap();
    let representations: Vec<(i64, String)> = db
        .connection()
        .prepare("SELECT id, representation FROM document_folders ORDER BY id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        representations,
        [(1, "list".to_string()), (2, "table".to_string())]
    );
    // Only the two representations are stored.
    assert!(
        db.connection()
            .execute(
                "UPDATE document_folders SET representation = 'board' WHERE id = 1",
                [],
            )
            .is_err()
    );
}

#[test]
fn folders_already_there_are_indexed_by_title_unless_deleted() {
    let mut db = migrated_before("folders_search");
    db.connection()
        .execute_batch(
            "INSERT INTO workspaces (id, title) VALUES (1, 'w');
             INSERT INTO task_folders (id, workspace_id, title, sort_order)
                 VALUES (1, 1, 'Garden chores', 1);
             INSERT INTO task_folders (id, workspace_id, title, sort_order, deleted)
                 VALUES (2, 1, 'Garden gone', 2, TRUE);
             INSERT INTO document_folders (id, workspace_id, title, sort_order)
                 VALUES (1, 1, 'Garden plans', 1);",
        )
        .unwrap();
    db.migrate().unwrap();

    assert_eq!(
        search(&db, "garden"),
        [
            ("documentFolder".to_string(), 1),
            ("taskFolder".to_string(), 1)
        ]
    );
}
