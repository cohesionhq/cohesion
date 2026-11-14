//! End-to-end tests: real scripts against an in-memory database.

use std::borrow::Cow;
use std::sync::Arc;

use cohesion_core::LocalIdentity;
use cohesion_core::model::{
    ActivityStatus, AgentCodeExecution, AgentCodeOutcome, AgentMessageContent, ConnectorDefinition,
    ConnectorResource, ConnectorResourceFilters, Credential, ExtensionManifest,
    ExtensionPermissionKind, HookKind, IntelligenceAgentColor, IntelligenceAgentMessageSender,
    IntelligenceEffort, MetadataKey, RegistryResult, ResourceContentKind, ResourceKindDescription,
    ServiceDescription, ServiceFunctionManifest, ServiceManifest,
};
use cohesion_core::protocol::{ClientCapabilities, ServerError, Vocabulary};
use cohesion_db::{
    Container, CreateAgentMessageCommand, CreateExtensionCommand, CreateIntelligenceAgentCommand,
    CreateIntelligenceCostEventCommand, CreateIntelligenceThreadCommand, CreateTaskCommand,
    CreateWorkspaceCommand, DatabaseContext, DatabaseEventKind, DatabaseEventTrace,
    DeleteResourceCommand, EnsureJournalCommand, GetExtensionCommand, GetExtensionQuery,
    GetResourceCommand, IdentifyLocalClientCommand, ImportResourceCommand, InstallExtensionCommand,
    ListWorkspacesCommand, SandboxKey, Secrets, SetExtensionPermissionCommand,
    SyncCapabilitiesCommand, UpsertConnectorAccountCommand, WriteArea,
};

use crate::globals::{ForegroundEffect, Mount};
use crate::sandbox::{Sandbox, SandboxOptions, ScriptArgument};
use crate::{
    ActorAccess, ScriptEngine, ScriptError, ScriptOutput, ScriptStop, ServiceAccess, ServiceCall,
};

/// The sandbox key of extension `identifier`.
fn extension(identifier: &str) -> SandboxKey {
    SandboxKey::Extension(identifier.to_string())
}

fn database() -> DatabaseContext {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    DatabaseContext::new(container, Secrets::new(), ())
}

/// Runs `script` against `db` on a fresh engine.
fn run(db: &DatabaseContext, script: &str) -> Result<ScriptOutput, ScriptError> {
    let engine = ScriptEngine::new(db.read_write());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    runtime.block_on(engine.general_purpose_run(script.to_string()))
}

fn json(db: &DatabaseContext, script: &str) -> serde_json::Value {
    let ScriptOutput(text) = run(db, script).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn create_workspace(db: &DatabaseContext, title: &str) -> i64 {
    db.send_blocking(CreateWorkspaceCommand {
        title: title.to_string(),
        emoji: None,
    })
    .unwrap()
    .id
}

#[test]
fn task_list_options() {
    let db = database();
    create_workspace(&db, "W");
    run(
        &db,
        "const w = cohesion.workspaces()[0]; \
         w.createTask({ title: 'open' }); w.createTask({ title: 'done' }).status = TaskStatus.completed;",
    )
    .unwrap();
    let titles = |call: &str| {
        json(
            &db,
            &format!("cohesion.workspaces()[0].{call}.map(t => t.title).sort()"),
        )
    };
    // Left out, `undefined` and `null` all mean "no filter": incomplete only.
    for call in [
        "tasks()",
        "tasks(undefined)",
        "tasks({})",
        "tasks({ status: undefined })",
        "tasks({ status: null })",
    ] {
        assert_eq!(titles(call), serde_json::json!(["open"]), "{call}");
    }
    assert_eq!(
        titles("tasks({ status: 'completed' })"),
        serde_json::json!(["done"])
    );
    assert_eq!(
        titles("tasks({ status: [TaskStatus.incomplete, 'completed'] })"),
        serde_json::json!(["done", "open"])
    );
    assert!(matches!(
        run(&db, "cohesion.workspaces()[0].tasks({ status: 'nope' })"),
        Err(ScriptError::Exception { message, .. }) if message.contains("nope")
    ));
}

#[test]
fn create_task_files_into_a_passed_folder() {
    let db = database();
    create_workspace(&db, "W");
    create_workspace(&db, "Other");
    assert_eq!(
        json(
            &db,
            "const w = cohesion.workspace('W'); \
             const folder = w.createTaskFolder({ title: 'Errands' }); \
             const task = w.createTask({ title: 'milk', folder }); \
             [task.taskFolderId === folder.id, w.createTask({ title: 'dishes' }).taskFolderId]",
        ),
        serde_json::json!([true, null])
    );
    // A folder from another workspace, or a value that is not a folder, throws.
    for script in [
        "const f = cohesion.workspace('Other').createTaskFolder({ title: 'X' }); \
         cohesion.workspace('W').createTask({ title: 't', folder: f })",
        "cohesion.workspace('W').createTask({ title: 't', folder: { id: 1 } })",
    ] {
        assert!(run(&db, script).is_err(), "{script}");
    }
}

#[test]
fn contacts_and_their_methods_save_each_assignment() {
    let db = database();
    create_workspace(&db, "W");
    assert_eq!(
        json(
            &db,
            "const w = cohesion.workspace('W'); \
             const c = w.createContact({ firstName: 'Ada', lastName: 'Lovelace', \
                 methods: [{ kind: ContactMethodKind.email, value: 'ada@example.com' }] }); \
             c.middleName = 'King'; \
             c.notes = 'Wrote the first program.'; \
             const phone = c.addMethod('phone', '+44 20 7946 0000'); \
             c.methods[0].value = 'ada@work.example'; \
             phone.kind = ContactMethodKind.email; \
             const unset = c.photo; \
             c.photo = { mediaType: 'image/png', data: 'AAAA' }; \
             const read = w.contact(c.id); \
             [read.middleName, read.notes, c.methods.length, \
              read.methods.map(m => [m.kind, m.value]), read.methods[0] instanceof ContactMethod, \
              unset, read.photo]",
        ),
        serde_json::json!([
            "King",
            "Wrote the first program.",
            2,
            [["email", "ada@work.example"], ["email", "+44 20 7946 0000"]],
            true,
            null,
            { "mediaType": "image/png", "data": "AAAA" }
        ])
    );
    // An unknown kind, an empty value or an empty name throws and saves
    // nothing.
    for script in [
        "cohesion.workspace('W').contacts()[0].addMethod('fax', '1')",
        "cohesion.workspace('W').contacts()[0].methods[0].value = ''",
        "cohesion.workspace('W').contacts()[0].lastName = ''",
        "cohesion.workspace('W').contacts()[0].photo = { mediaType: 'image/tiff', data: 'AAAA' }",
        "cohesion.workspace('W').createContact({ firstName: 'G', lastName: 'H', \
             methods: [{ kind: 'email', value: '' }] })",
    ] {
        assert!(run(&db, script).is_err(), "{script}");
    }
    assert_eq!(
        json(
            &db,
            "const w = cohesion.workspace('W'); const c = w.contacts()[0]; \
             c.methods[1].delete(); \
             const kept = [c.lastName, w.contact(c.id).methods.length, w.contacts().length, \
                 c.photo.mediaType]; \
             c.photo = null; kept.push(w.contact(c.id).photo); \
             c.delete(); [...kept, w.contacts().length]",
        ),
        serde_json::json!(["Lovelace", 1, 1, "image/png", null, 0])
    );
}

#[test]
fn calendars_and_events_are_shared_and_colored_by_the_palette() {
    let db = database();
    create_workspace(&db, "W");
    assert_eq!(
        json(
            &db,
            "const store = cohesion.createCalendarStore('Home'); \
             const cal = cohesion.createCalendar({ storeId: store.id, title: 'Family', \
                 color: 'teal' }); \
             const e = cal.createEvent({ title: 'Dentist', location: 'High St', \
                 timing: { kind: 'timed', startsAt: 1000, endsAt: 4600, timeZone: 'UTC' }, \
                 invitees: [{ email: 'ada@example.com' }], color: '#d50000' }); \
             cal.selectedColor = '#1e88e5'; \
             e.update({ title: 'Dentist', \
                 timing: { kind: 'allDay', startDate: '1970-01-01', endDate: '1970-01-02' } }); \
             const hit = cohesion.workspace('W').search('dentist')[0]; \
             const own = cohesion.calendarStores()[0]; own.rename('Personal'); \
             [store.accountId, store.namedByUser, own.title, own.colorSupport, \
              cohesion.calendars()[0].selectedColor, cohesion.calendars()[0].declaredColor, \
              e.timing, e.invitees, e.selectedColor, \
              cal.events({ from: 0, to: 3600 }).length, \
              hit.open() instanceof CalendarEvent, cohesion.calendarEvents({ from: 0, to: 1 })[0].title]",
        ),
        serde_json::json!([
            null,
            true,
            "Personal",
            "none",
            "blue",
            null,
            { "kind": "allDay", "startDate": "1970-01-01", "endDate": "1970-01-02" },
            [],
            null,
            1,
            true,
            "Dentist"
        ])
    );
    // A color that isn't one, an empty title, or an event that ends before
    // it starts throws and saves nothing.
    for script in [
        "cohesion.calendars()[0].selectedColor = 'mauve'",
        "cohesion.calendars()[0].createEvent({ title: '', \
             timing: { kind: 'allDay', startDate: '2026-01-01', endDate: '2026-01-01' } })",
        "cohesion.calendars()[0].createEvent({ title: 'x', \
             timing: { kind: 'timed', startsAt: 10, endsAt: 0, timeZone: 'UTC' } })",
        "cohesion.calendars()[0].createEvent({ title: 'x', timing: { kind: 'sometime' } })",
    ] {
        assert!(run(&db, script).is_err(), "{script}");
    }
}

#[test]
fn task_priority_reads_and_writes_by_name() {
    let db = database();
    create_workspace(&db, "W");
    assert_eq!(
        json(
            &db,
            "const w = cohesion.workspace('W'); \
             const plain = w.createTask({ title: 'plain' }); \
             const task = w.createTask({ title: 't', priority: TaskPriority.high }); \
             const created = task.priority; \
             task.priority = 'urgent'; \
             [plain.priority, created, w.tasks().find(t => t.id === task.id).priority]",
        ),
        serde_json::json!(["none", "high", "urgent"])
    );
    for script in [
        "cohesion.workspace('W').tasks()[0].priority = 'soon'",
        "cohesion.workspace('W').createTask({ title: 't', priority: 'soon' })",
    ] {
        assert!(run(&db, script).is_err(), "{script}");
    }
}

#[test]
fn task_size_reads_writes_and_filters_by_number() {
    let db = database();
    create_workspace(&db, "W");
    assert_eq!(
        json(
            &db,
            "const w = cohesion.workspace('W'); \
             const plain = w.createTask({ title: 'plain' }); \
             const task = w.createTask({ title: 't', size: 2 }); \
             const created = task.size; \
             task.size = 5; \
             const big = w.tasks({ size: 5 }).map(t => t.title); \
             const either = w.tasks({ size: [2, 5] }).map(t => t.title); \
             task.size = null; \
             [plain.size, created, big, either, w.tasks().find(t => t.id === task.id).size]",
        ),
        serde_json::json!([null, 2, ["t"], ["t"], null])
    );
    for script in [
        "cohesion.workspace('W').tasks()[0].size = 6",
        "cohesion.workspace('W').tasks()[0].size = 2.5",
        "cohesion.workspace('W').tasks()[0].size = 'small'",
        "cohesion.workspace('W').createTask({ title: 't', size: 0 })",
        "cohesion.workspace('W').tasks({ size: 9 })",
    ] {
        assert!(run(&db, script).is_err(), "{script}");
    }
}

#[test]
fn task_assignee_reads_writes_and_reaches_a_hook() {
    let db = database();
    let workspace_id = create_workspace(&db, "W");
    let agent_id = db
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
        .unwrap()
        .id;
    let read = json(
        &db,
        &format!(
            "const w = cohesion.workspace('W'); \
             const plain = w.createTask({{ title: 'plain' }}); \
             const task = w.createTask({{ title: 't', assignee: {{ kind: 'user' }} }}); \
             const created = task.assignee; \
             task.assignee = {{ kind: 'agent', agentId: {agent_id} }}; \
             const agent = w.tasks().find(t => t.id === task.id).assignee; \
             plain.assignee = {{ kind: 'user' }}; \
             plain.assignee = null; \
             const contact = w.createContact({{ firstName: 'Bo', lastName: 'Lee' }}); \
             plain.assignee = {{ kind: 'contact', contactId: contact.id }}; \
             const assignedContact = plain.assignee; \
             plain.assignee = null; \
             [plain.assignee, created, agent, task.id, assignedContact, contact.id]"
        ),
    );
    assert_eq!(
        read,
        serde_json::json!([
            null,
            { "kind": "user" },
            { "kind": "agent", "agentId": agent_id },
            read[3],
            { "kind": "contact", "contactId": read[5] },
            read[5]
        ])
    );
    let task_id = read[3].as_i64().unwrap();
    for script in [
        "cohesion.workspace('W').tasks()[0].assignee = { kind: 'team' }".to_string(),
        "cohesion.workspace('W').tasks()[0].assignee = 'user'".to_string(),
        format!(
            "cohesion.workspace('W').tasks()[0].assignee = {{ kind: 'agent', agentId: {} }}",
            agent_id + 1
        ),
        "cohesion.workspace('W').tasks()[0].assignee = { kind: 'contact', contactId: 999 }"
            .to_string(),
    ] {
        assert!(run(&db, &script).is_err(), "{script}");
    }

    // `task:assigned` hands its function the task, assignee and all.
    let sandbox = Sandbox::new(SandboxOptions::STANDARD, db.read_write(), &[], None).unwrap();
    sandbox
        .evaluate("function onTaskAssigned(task) { return task.assignee.agentId; }".to_string())
        .unwrap();
    let ScriptOutput(text) = sandbox
        .call_with_arguments(
            "onTaskAssigned",
            &[ScriptArgument::Task {
                workspace_id,
                task_id,
            }],
        )
        .unwrap();
    assert_eq!(text, agent_id.to_string());
}

#[test]
fn task_dates_set_independently() {
    let db = database();
    create_workspace(&db, "W");
    // Each setter keeps the other date; `null` clears one. Text that is not
    // a real `YYYY-MM-DD` day throws and leaves the task as it was.
    assert_eq!(
        json(
            &db,
            "const w = cohesion.workspace('W'); \
             const task = w.createTask({ title: 't', dueDate: '2026-10-20' }); \
             task.startDate = '2026-10-14'; \
             const both = [task.startDate, task.dueDate]; \
             task.dueDate = null; \
             const rejected = ['2026-02-30', '2026-1-5', 'tomorrow'].map(day => { \
                 try { task.startDate = day; return 'saved'; } catch (e) { return 'threw'; } \
             }); \
             const stored = w.tasks().find(t => t.id === task.id); \
             [both, [stored.startDate, stored.dueDate], rejected]",
        ),
        serde_json::json!([
            ["2026-10-14", "2026-10-20"],
            ["2026-10-14", null],
            ["threw", "threw", "threw"]
        ])
    );
}

#[test]
fn tasks_filter_by_day_range() {
    let db = database();
    create_workspace(&db, "W");
    // Bounds are exclusive and either may be left out; an undated task never
    // matches a range; a malformed bound matches nothing rather than
    // comparing as text.
    assert_eq!(
        json(
            &db,
            "const w = cohesion.workspace('W'); \
             ['2026-10-19', '2026-10-20', '2026-10-21', null] \
                 .forEach((dueDate, i) => w.createTask({ title: `t${i}`, dueDate })); \
             const titles = dueDate => w.tasks({ dueDate }).map(t => t.title).sort(); \
             [ \
                 titles('2026-10-20'), \
                 titles(['2026-10-19', '2026-10-21']), \
                 titles({ after: '2026-10-19', before: '2026-10-21' }), \
                 titles({ before: '2026-10-21' }), \
                 titles({ after: '2026-1-5' }), \
             ]",
        ),
        serde_json::json!([["t1"], ["t0", "t2"], ["t1"], ["t0", "t1"], []])
    );
}

#[test]
fn tasks_sort_by_the_sort_option() {
    let db = database();
    create_workspace(&db, "W");
    // Default is creation order; a date sort keeps undated tasks last even
    // descending; folder listings take the same options; an unknown sort
    // throws.
    assert_eq!(
        json(
            &db,
            "const w = cohesion.workspace('W'); \
             const f = w.createTaskFolder({ title: 'F' }); \
             [['b', '2026-10-20'], ['C', null], ['a', '2026-10-01']] \
                 .forEach(([title, dueDate]) => f.create({ title, dueDate })); \
             const titles = tasks => tasks.map(t => t.title); \
             let unknown = false; \
             try { w.tasks({ sort: 'effort' }); } catch (e) { unknown = e instanceof TypeError; } \
             [ \
                 titles(w.tasks()), \
                 titles(w.tasks({ sort: TaskSort.title })), \
                 titles(w.tasks({ sort: TaskSort.dueDate, descending: true })), \
                 titles(cohesion.tasks({ sort: 'dueDate' })), \
                 titles(f.tasks({ sort: TaskSort.title, descending: true })), \
                 unknown, \
             ]",
        ),
        serde_json::json!([
            ["b", "C", "a"],
            ["a", "b", "C"],
            ["b", "a", "C"],
            ["a", "b", "C"],
            ["C", "b", "a"],
            true
        ])
    );
}

#[test]
fn journal_is_per_date_and_defaults_to_today() {
    let db = database();
    create_workspace(&db, "W");
    // An explicit date gets that day's entry, the same one each time; no
    // date is a different (today's) entry; a malformed date throws.
    assert_eq!(
        json(
            &db,
            "const w = cohesion.workspace('W'); \
             const dated = w.journal('2020-01-02'); \
             let threw = false; \
             try { w.journal('2020-1-2'); } catch (e) { threw = true; } \
             [dated.title, w.journal('2020-01-02').id === dated.id, \
              w.journal().id !== dated.id, threw]",
        ),
        serde_json::json!(["2020-01-02", true, true, true])
    );
}

#[test]
fn read_only_sandbox_reads_journals_with_get_journal() {
    let db = database();
    let workspace_id = create_workspace(&db, "W");
    db.send_blocking(EnsureJournalCommand {
        workspace_id,
        date: "2020-01-02".to_string(),
    })
    .unwrap();
    let sandbox = Sandbox::new(
        SandboxOptions::STANDARD,
        db.read_only(),
        &[Mount::Cohesion],
        None,
    )
    .unwrap();

    // `getJournal` reads, null for an empty day; `journal` creates, so it
    // throws even for a day that exists.
    let ScriptOutput(text) = sandbox
        .evaluate(
            "const w = cohesion.workspace('W');\n\
             let threw = false;\n\
             try { w.journal('2020-01-02'); } catch (e) { threw = true; }\n\
             [w.getJournal('2020-01-02').title, w.getJournal('2020-01-03'), threw]"
                .to_string(),
        )
        .unwrap();
    assert_eq!(text, r#"["2020-01-02",null,true]"#);
}

#[test]
fn deadline_reports_the_time_limit() {
    let options = SandboxOptions {
        time_limit: std::time::Duration::ZERO,
        ..SandboxOptions::STANDARD
    };
    let sandbox = Sandbox::new(options, database().read_write(), &[], None).unwrap();
    let Err(ScriptError::Exception { message, .. }) = sandbox.evaluate("while (true) {}".into())
    else {
        panic!("an endless loop should be stopped");
    };
    assert!(message.contains("time limit"), "{message}");
}

#[test]
fn a_stop_interrupts_a_running_script_and_leaves_the_next_alone() {
    let sandbox =
        Sandbox::new(SandboxOptions::STANDARD, database().read_write(), &[], None).unwrap();
    let stop = ScriptStop::default();
    let stopper = std::thread::spawn({
        let stop = stop.clone();
        move || stop.stop()
    });
    let stopped = sandbox.stoppable(stop.clone(), |sandbox| {
        sandbox.evaluate("while (true) {}".into())
    });
    stopper.join().unwrap();
    assert!(matches!(stopped, Err(ScriptError::Stopped)), "{stopped:?}");

    // Stopped already, a script fails before it starts; outside the scope,
    // the sandbox runs on as before.
    let again = sandbox.stoppable(stop, |sandbox| sandbox.evaluate("1".into()));
    assert!(matches!(again, Err(ScriptError::Stopped)), "{again:?}");
    let ScriptOutput(text) = sandbox.evaluate("1 + 1".into()).unwrap();
    assert_eq!(text, "2");
}

#[test]
fn panicking_job_fails_alone() {
    let db = database();
    create_workspace(&db, "W");
    let engine = ScriptEngine::new(db.read_write());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let result = runtime.block_on(engine.general_purpose::<()>(|_| panic!("boom")));
    assert!(
        matches!(&result, Err(ScriptError::Panicked(message)) if message == "boom"),
        "{result:?}"
    );
    let ScriptOutput(text) = runtime
        .block_on(engine.general_purpose_run("cohesion.workspaces().length".to_string()))
        .unwrap();
    assert_eq!(text, "1");
}

// Shutting the engine down ends its worker threads: a script after it is
// cancelled rather than run, and the threads don't outlive the engine.
#[test]
fn a_shut_down_engine_runs_nothing() {
    let db = database();
    create_workspace(&db, "W");
    let engine = ScriptEngine::new(db.read_write());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let ScriptOutput(text) = runtime
        .block_on(engine.general_purpose_run("1 + 1".to_string()))
        .unwrap();
    assert_eq!(text, "2");
    runtime.block_on(engine.shutdown());
    let after = runtime.block_on(engine.general_purpose_run("1 + 1".to_string()));
    assert!(matches!(after, Err(ScriptError::Cancelled)), "{after:?}");
}

#[test]
fn scripts_do_not_share_prototypes() {
    let db = database();
    create_workspace(&db, "W");
    let engine = ScriptEngine::new(db.read_write());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let first = "Workspace.prototype.leak = 1; \
                 Object.getPrototypeOf(Workspace.prototype).leak2 = 1; 0";
    runtime
        .block_on(engine.general_purpose_run(first.to_string()))
        .unwrap();
    let second = "const w = cohesion.workspaces()[0]; [typeof w.leak, typeof w.leak2]";
    let ScriptOutput(text) = runtime
        .block_on(engine.general_purpose_run(second.to_string()))
        .unwrap();
    assert_eq!(text, r#"["undefined","undefined"]"#);
}

#[test]
fn scope_runs_only_after_its_script_evaluates() {
    let db = database();
    let engine = ScriptEngine::new(db.read_write());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    assert!(!engine.is_scope_running(&extension("ext")));

    let broken = engine.spawn_scope(
        extension("broken"),
        "throw 'no'".to_string(),
        Cow::Borrowed(&[]),
        db.read_write(),
    );
    assert!(runtime.block_on(broken).is_err());
    assert!(!engine.is_scope_running(&extension("broken")));

    let ext = engine.spawn_scope(
        extension("ext"),
        "0".to_string(),
        Cow::Borrowed(&[]),
        db.read_write(),
    );
    runtime.block_on(ext).unwrap();
    assert!(engine.is_scope_running(&extension("ext")));
}

#[test]
fn call_with_arguments_inflates_entities() {
    let db = database();
    let workspace_id = create_workspace(&db, "Home");
    let task_id = db
        .send_blocking(CreateTaskCommand {
            workspace_id,
            title: "Water plants".to_string(),
            folder_id: None,
            priority: None,
            size: None,
            assignee: None,
            description: None,
            start_date: None,
            due_date: None,
            tag_ids: Vec::new(),
        })
        .unwrap()
        .id;
    let sandbox = Sandbox::new(SandboxOptions::STANDARD, db.read_write(), &[], None).unwrap();
    sandbox
        .evaluate(
            "function onTask(task) { return [task instanceof Task, task.title]; }".to_string(),
        )
        .unwrap();

    let ScriptOutput(text) = sandbox
        .call_with_arguments(
            "onTask",
            &[ScriptArgument::Task {
                workspace_id,
                task_id,
            }],
        )
        .unwrap();
    assert_eq!(text, r#"[true,"Water plants"]"#);

    let missing = sandbox.call_with_arguments(
        "onTask",
        &[ScriptArgument::Task {
            workspace_id,
            task_id: task_id + 1,
        }],
    );
    assert!(matches!(missing, Err(ScriptError::LoadError(_))));

    let undefined = sandbox.call("onTaskCreate");
    assert!(matches!(undefined, Err(ScriptError::MissingFunction(name)) if name == "onTaskCreate"));
}

#[test]
fn agent_messages_arrive_with_their_content_parsed_by_kind() {
    let db = database();
    let agent_id = db
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
        .unwrap()
        .id;
    let execution = AgentCodeExecution {
        title: "Count tasks".to_string(),
        code: "1 + 1".to_string(),
        outcome: AgentCodeOutcome::Succeeded {
            result: "2".to_string(),
        },
    };
    let message_id = db
        .send_blocking(CreateAgentMessageCommand {
            agent_id,
            run_id: None,
            run_description: None,
            sender: IntelligenceAgentMessageSender::Agent,
            content: AgentMessageContent::CodeExecution(execution),
            reply_to: None,
        })
        .unwrap()
        .id;
    let sandbox = Sandbox::new(SandboxOptions::STANDARD, db.read_only(), &[], None).unwrap();
    sandbox
        .evaluate(
            "function onAgentMessage(message) {
               const content = message.content;
               switch (content.kind) {
                 case 'codeExecution':
                   return [message.agentId, message.sender, content.title, content.outcome.status, content.outcome.result];
               }
             }"
            .to_string(),
        )
        .unwrap();

    let ScriptOutput(text) = sandbox
        .call_with_arguments(
            "onAgentMessage",
            &[ScriptArgument::AgentMessage { message_id }],
        )
        .unwrap();
    assert_eq!(
        text,
        format!(r#"[{agent_id},"agent","Count tasks","succeeded","2"]"#)
    );
}

#[test]
fn agents_read_under_a_read_only_sandbox() {
    let db = database();
    let agent_id = db
        .send_blocking(CreateIntelligenceAgentCommand {
            name: Some("Scout".to_string()),
            color: IntelligenceAgentColor::Teal,
            emoji: Some("🦊".to_string()),
            personality: None,
            verbosity: None,
            role: None,
            directive: None,
            team_id: None,
        })
        .unwrap()
        .id;
    let sandbox = Sandbox::new(
        SandboxOptions::STANDARD,
        db.read_only(),
        &[Mount::Cohesion],
        None,
    )
    .unwrap();
    let ScriptOutput(text) = sandbox
        .evaluate(format!(
            "const a = cohesion.agent({agent_id});\n\
             [a instanceof Agent, a.name, a.emoji, a.color, a.activity, a.unread,\n\
              cohesion.agents().map(x => x.id), JSON.parse(JSON.stringify(a)).emoji]"
        ))
        .unwrap();
    let read: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        read,
        serde_json::json!([true, "Scout", "🦊", "teal", null, false, [agent_id], "🦊"])
    );
}

#[test]
fn a_script_organizes_agents_into_teams_but_a_read_only_one_cannot() {
    let db = database();
    let agent_id = db
        .send_blocking(CreateIntelligenceAgentCommand {
            name: Some("Scout".to_string()),
            color: IntelligenceAgentColor::Teal,
            emoji: None,
            personality: None,
            verbosity: None,
            role: None,
            directive: None,
            team_id: None,
        })
        .unwrap()
        .id;
    let organized = json(
        &db,
        &format!(
            "const team = cohesion.createTeam('Research');\n\
             const scout = cohesion.agent({agent_id});\n\
             scout.setTeam(team.id);\n\
             team.rename('Field research');\n\
             const members = cohesion.team(team.id).agents().map(a => a.name);\n\
             [team instanceof Team, cohesion.teams().map(t => t.title), scout.teamId === team.id,\n\
              members]"
        ),
    );
    assert_eq!(
        organized,
        serde_json::json!([true, ["Field research"], true, ["Scout"]])
    );

    let sandbox = Sandbox::new(
        SandboxOptions::STANDARD,
        db.read_only(),
        &[Mount::Cohesion],
        None,
    )
    .unwrap();
    let ScriptOutput(text) = sandbox
        .evaluate(
            "const team = cohesion.teams()[0];\n\
             const scout = team.agents()[0];\n\
             [() => cohesion.createTeam('Ops'), () => scout.setTeam(null),\n\
              () => team.rename('Ops'), () => team.delete()].map(write => {\n\
                 try { write(); return 'wrote'; }\n\
                 catch (e) { return e instanceof Error && e.message.includes('read-only access'); }\n\
             })"
            .to_string(),
        )
        .unwrap();
    assert_eq!(text, "[true,true,true,true]");
}

#[test]
fn a_missing_agent_throws_a_reference_error() {
    let db = database();
    let thrown = json(
        &db,
        "try { cohesion.agent(999); 'found' } catch (e) { e instanceof ReferenceError }",
    );
    assert_eq!(thrown, serde_json::json!(true));
}

#[test]
fn vocabulary_words_an_activity_status_as_hello_does() {
    let db = database();
    let label = json(
        &db,
        "cohesion.vocabulary().activity_statuses.find(w => w.kind === 'paused')?.label",
    );
    let paused = Vocabulary::new()
        .activity_statuses
        .into_iter()
        .find(|word| word.kind == ActivityStatus::Paused)
        .unwrap();
    assert_eq!(label, serde_json::json!(paused.label));
}

#[test]
fn registry_results_take_the_query_and_read_source_urls() {
    let db = database();
    let sandbox = Sandbox::new(SandboxOptions::STANDARD, db.read_only(), &[], None).unwrap();
    sandbox
        .evaluate(
            "function registryResults(query) {
               return [{
                 manifest: { identifier: 'com.example.' + query, name: query, version: 1,
                             permissions: ['network'], hooks: [] },
                 sourceURL: 'https://example.com/' + query + '.js',
               }];
             }"
            .to_string(),
        )
        .unwrap();

    let results: Vec<RegistryResult> = sandbox
        .call_returning_with_arguments(
            "registryResults",
            &[ScriptArgument::String("stamp".to_string())],
        )
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].manifest.identifier, "com.example.stamp");
    assert_eq!(results[0].manifest.author, None);
    assert_eq!(results[0].source_url, "https://example.com/stamp.js");
}

#[test]
fn json_arguments_arrive_as_objects_without_absent_keys() {
    let db = database();
    let sandbox = Sandbox::new(SandboxOptions::STANDARD, db.read_only(), &[], None).unwrap();
    sandbox
        .evaluate(
            "function f(x) { return [x.organizationUnitId, x.search === undefined]; }".to_string(),
        )
        .unwrap();

    let filters = ConnectorResourceFilters {
        organization_unit_id: Some("u1".to_string()),
        search: None,
    };
    let result: (String, bool) = sandbox
        .call_returning_with_arguments(
            "f",
            &[ScriptArgument::Json(serde_json::to_value(filters).unwrap())],
        )
        .unwrap();
    assert_eq!(result, ("u1".to_string(), true));
}

#[test]
fn resource_argument_inflates_and_workspace_lists_resources() {
    let db = database();
    let workspace_id = create_workspace(&db, "Home");
    let extension = db
        .send_blocking(CreateExtensionCommand {
            script: String::new(),
            manifest: ExtensionManifest {
                identifier: "com.example".to_string(),
                name: "Example".to_string(),
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
    let account = db
        .send_blocking(UpsertConnectorAccountCommand {
            extension_id: extension.id,
            connector: ConnectorDefinition {
                identifier: "com.example".to_string(),
                icon: None,
                version: 1,
                description: ServiceDescription {
                    sign_in_hint: None,
                    title: "Example".to_string(),
                    unit: "Workspace".to_string(),
                    kinds: vec![ResourceKindDescription {
                        identifier: "page".to_string(),
                        title: "Page".to_string(),
                        plural: "Pages".to_string(),
                        contents: "Markdown".to_string(),
                    }],
                    metadata_keys: vec![],
                },
            },
            identifier: "acct".to_string(),
            description: "Example Co.".to_string(),
            credential: Credential {
                authorization: "token".to_string(),
                refresh: None,
                expires_at: None,
            },
        })
        .unwrap();
    let resource_id = db
        .send_blocking(ImportResourceCommand {
            workspace_id,
            account_id: account.id,
            resource: ConnectorResource {
                identifier: "p1".to_string(),
                last_updated: 1,
                title: "Roadmap".to_string(),
                contents: "# Next quarter".to_string(),
                content_kind: ResourceContentKind::Markdown,
                kind_identifier: "page".to_string(),
                metadata: [
                    (MetadataKey::Author, "Ada".to_string()),
                    (MetadataKey::DateCreated, "2026-10-24T09:30:00Z".to_string()),
                ]
                .into(),
            },
        })
        .unwrap()
        .id;

    let sandbox = Sandbox::new(SandboxOptions::STANDARD, db.read_only(), &[], None).unwrap();
    sandbox
        .evaluate(
            "function onResourceImport(resource) { \
             return [resource instanceof Resource, resource.kind, resource.contentKind === 'markdown']; }"
                .to_string(),
        )
        .unwrap();
    let ScriptOutput(text) = sandbox
        .call_with_arguments(
            "onResourceImport",
            &[ScriptArgument::Resource {
                workspace_id,
                resource_id,
            }],
        )
        .unwrap();
    assert_eq!(text, r#"[true,"com.example.page",true]"#);

    let workspace = format!("cohesion.workspace({workspace_id})");
    for filter in ["{ kind: 'com.example.page' }", "{ search: 'ROAD' }"] {
        assert_eq!(
            json(
                &db,
                &format!("{workspace}.resources({filter}).map(a => a.title)")
            ),
            serde_json::json!(["Roadmap"]),
            "{filter}"
        );
    }
    assert_eq!(
        json(
            &db,
            &format!("{workspace}.resources({{ kind: 'com.example.other' }}).length")
        ),
        serde_json::json!(0)
    );
    assert_eq!(
        json(
            &db,
            &format!("{workspace}.search('roadmap')[0].open() instanceof Resource")
        ),
        serde_json::json!(true)
    );
    assert_eq!(
        json(
            &db,
            &format!(
                "const r = {workspace}.resources()[0]; \
                 let threw = false; try {{ r.get('bogus'); }} catch (e) {{ threw = e instanceof TypeError; }} \
                 [r.get(MetadataKey.author), r.get('dateDue'), threw, \
                  MetadataKey.dateCreated, r.get('dateCreated')]"
            )
        ),
        serde_json::json!(["Ada", null, true, "dateCreated", "2026-10-24T09:30:00Z"])
    );

    // A removed resource, whose row is gone, still reaches a hook whole.
    let resource = db
        .send_blocking(GetResourceCommand {
            workspace_id,
            resource_id,
        })
        .unwrap();
    db.send_blocking(DeleteResourceCommand {
        workspace_id,
        resource_id,
    })
    .unwrap();
    sandbox
        .evaluate(
            "function onResourceDelete(resource) { \
             return [resource instanceof Resource, resource.title]; }"
                .to_string(),
        )
        .unwrap();
    let ScriptOutput(text) = sandbox
        .call_with_arguments(
            "onResourceDelete",
            &[ScriptArgument::DeletedResource { resource }],
        )
        .unwrap();
    assert_eq!(text, r#"[true,"Roadmap"]"#);
}

#[test]
fn read_only_sandbox_reads_articles_but_never_fetches_one() {
    let db = database();
    follow_a_feed(&db);
    let sandbox = Sandbox::new(
        SandboxOptions::STANDARD,
        db.read_only(),
        &[Mount::Cohesion],
        None,
    )
    .unwrap();

    // The write gate throws before the method fetches a page.
    let ScriptOutput(text) = sandbox
        .evaluate(
            "const article = cohesion.feeds()[0].articles()[0];\n\
             let refused;\n\
             try { article.resnapshot(); refused = 'wrote'; }\n\
             catch (e) { refused = e instanceof Error && e.message.includes('read-only access'); }\n\
             [article.snapshot.state, refused]"
                .to_string(),
        )
        .unwrap();
    assert_eq!(text, r#"["captured",true]"#);
}

#[test]
fn a_sandbox_without_article_fetch_never_fetches_an_article() {
    let db = database();
    follow_a_feed(&db);
    // Writable and online, as an extension granted `cohesion:write` and
    // `network`: still no `ArticleFetch`, which no permission mounts.
    let sandbox = Sandbox::new(
        SandboxOptions::STANDARD,
        db.read_write(),
        &[Mount::Cohesion, Mount::Network],
        None,
    )
    .unwrap();

    // It throws before fetching.
    let ScriptOutput(text) = sandbox
        .evaluate(
            "const article = cohesion.feeds()[0].articles()[0];\n\
             try { article.resnapshot(); 'fetched'; }\n\
             catch (e) { e instanceof Error && e.message.includes('isn’t available to extensions'); }"
            .to_string(),
        )
        .unwrap();
    assert_eq!(text, "true");
}

/// Follows a feed with one item about sourdough.
fn follow_a_feed(db: &DatabaseContext) {
    use cohesion_core::model::{ArticleCapture, FeedFullPagePolicy, FeedItemCapture};
    db.send_blocking(cohesion_db::CreateFeedCommand {
        url: "https://bakery.example/feed.xml".to_string(),
        title: "Bakery".to_string(),
        full_page_policy: FeedFullPagePolicy::WhenExcerpt,
        items: vec![FeedItemCapture {
            url: "https://bakery.example/rye".to_string(),
            title: Some("Rye".to_string()),
            author: None,
            published: None,
            fetched: Ok(ArticleCapture {
                final_url: "https://bakery.example/rye".to_string(),
                page_title: None,
                author: None,
                published: None,
                text: "sourdough rye".to_string(),
                truncated: false,
            }),
        }],
    })
    .unwrap();
}

#[test]
fn the_corpus_is_searched_apart_from_any_workspace() {
    let db = database();
    create_workspace(&db, "Home");
    follow_a_feed(&db);
    let text = json(
        &db,
        "const home = cohesion.workspaces()[0];\n\
         const item = cohesion.searchCorpus('sourdough', { feedIds: [cohesion.feeds()[0].id] })[0];\n\
         const unread = item.read;\n\
         item.markRead();\n\
         [home.search('sourdough').length, cohesion.feeds()[0].articles().length, unread,\n\
          item.read, item.feed().title]",
    );
    assert_eq!(text, serde_json::json!([0, 1, false, true, "Bakery"]));
}

#[test]
fn read_only_sandbox_searches_the_corpus_but_marks_nothing() {
    let db = database();
    follow_a_feed(&db);
    let sandbox = Sandbox::new(
        SandboxOptions::STANDARD,
        db.read_only(),
        &[Mount::Cohesion],
        None,
    )
    .unwrap();
    let ScriptOutput(text) = sandbox
        .evaluate(
            "const item = cohesion.searchCorpus('rye')[0];\n\
             const refused = [\n\
                 () => item.markRead(),\n\
                 () => item.markUnread(),\n\
             ].map(write => {\n\
                 try { write(); return 'wrote'; }\n\
                 catch (e) { return e.message.includes('read-only access'); }\n\
             });\n\
             [item.title, refused, cohesion.searchCorpus('rye')[0].read]"
                .to_string(),
        )
        .unwrap();
    assert_eq!(text, r#"["Rye",[true,true],false]"#);
}

#[test]
fn tasks_are_tagged_by_name_and_filtered_by_tag() {
    let db = database();
    create_workspace(&db, "W");
    assert_eq!(
        json(
            &db,
            "const w = cohesion.workspace('W'); \
             const a = w.createTask({ title: 'a' }); \
             const b = w.createTask({ title: 'b' }); \
             a.tag('Errand'); a.tag('errand'); a.tag('home'); b.tag('HOME'); \
             const names = a.tags().map(t => t.name); \
             a.untag('ERRAND'); a.untag('nothing'); \
             const left = a.tags().map(t => t.name); \
             const home = w.tasks({ tag: 'home' }).map(t => t.title); \
             w.tags().find(t => t.name === 'home').delete(); \
             [names, left, home, a.tags().length, w.tags().length, w.tasks({ tag: 'home' }).length]",
        ),
        serde_json::json!([["Errand", "home"], ["home"], ["a", "b"], 0, 1, 0])
    );
}

#[test]
fn read_only_sandbox_cannot_tag() {
    let db = database();
    let workspace_id = create_workspace(&db, "Home");
    create_task(&db, workspace_id, "Water plants");
    let sandbox = Sandbox::new(
        SandboxOptions::STANDARD,
        db.read_only(),
        &[Mount::Cohesion],
        None,
    )
    .unwrap();
    let ScriptOutput(text) = sandbox
        .evaluate(
            "const ws = cohesion.workspaces()[0];\n\
             const task = ws.tasks()[0];\n\
             [() => task.tag('x'), () => task.untag('x'), () => ws.createTag('task', 'x')].map(write => {\n\
                 try { write(); return 'wrote'; }\n\
                 catch (e) { return e instanceof Error && e.message.includes('read-only access'); }\n\
             })"
            .to_string(),
        )
        .unwrap();
    assert_eq!(text, "[true,true,true]");
}

#[test]
fn read_only_sandbox_reads_but_cannot_write() {
    let db = database();
    let workspace_id = create_workspace(&db, "Home");
    let task_id = db
        .send_blocking(CreateTaskCommand {
            workspace_id,
            title: "Water plants".to_string(),
            folder_id: None,
            priority: None,
            size: None,
            assignee: None,
            description: None,
            start_date: None,
            due_date: None,
            tag_ids: Vec::new(),
        })
        .unwrap()
        .id;
    let sandbox = Sandbox::new(
        SandboxOptions::STANDARD,
        db.read_only(),
        &[Mount::Cohesion],
        None,
    )
    .unwrap();

    let ScriptOutput(text) = sandbox
        .evaluate(
            "const ws = cohesion.workspaces()[0];\n\
             const task = ws.tasks()[0];\n\
             const refused = [\n\
                 () => ws.rename('x'),\n\
                 () => ws.createTask({ title: 't' }),\n\
                 () => task.setMetadata('k', 'v'),\n\
                 () => { task.priority = 'high'; },\n\
                 () => { task.description = 'x'; },\n\
                 () => { task.dueDate = '2026-10-20'; },\n\
             ].map(write => {\n\
                 try { write(); return 'wrote'; }\n\
                 catch (e) { return e instanceof Error && e.message.includes('read-only access'); }\n\
             });\n\
             [ws.title, ws.metadata('k'), task.title, refused]"
                .to_string(),
        )
        .unwrap();
    assert_eq!(
        text,
        r#"["Home",null,"Water plants",[true,true,true,true,true,true]]"#
    );
    assert_eq!(
        db.send_blocking(ListWorkspacesCommand::default()).unwrap()[0].title,
        "Home"
    );

    sandbox
        .evaluate("function onTask(task) { return task.title; }".to_string())
        .unwrap();
    let ScriptOutput(text) = sandbox
        .call_with_arguments(
            "onTask",
            &[ScriptArgument::Task {
                workspace_id,
                task_id,
            }],
        )
        .unwrap();
    assert_eq!(text, r#""Water plants""#);
}

#[test]
fn api_mounts_without_cohesion() {
    let sandbox = Sandbox::new(
        SandboxOptions::STANDARD,
        database().read_write(),
        &[Mount::Intelligence],
        None,
    )
    .unwrap();
    let ScriptOutput(text) = sandbox
        .evaluate(
            "[typeof cohesion, typeof EndpointModel, typeof Workspace, \
              IntelligenceProvider.anthropic, NetworkMethod.post]"
                .to_string(),
        )
        .unwrap();
    assert_eq!(
        text,
        r#"["undefined","function","function","anthropic","POST"]"#
    );
}

/// A sandbox writing only intelligence settings, as an extension granted
/// `intelligence` without `cohesion:write`, sets endpoints up but cannot
/// change the user's data; one writing only the user's data is the reverse.
#[test]
fn write_areas_split_intelligence_settings_from_data() {
    let db = database();
    db.send_blocking(CreateWorkspaceCommand {
        title: "Home".to_string(),
        emoji: None,
    })
    .unwrap();
    let mounts = &[Mount::Cohesion, Mount::Intelligence];
    let settings = Sandbox::new(
        SandboxOptions::STANDARD,
        db.writing([WriteArea::IntelligenceSettings]),
        mounts,
        None,
    )
    .unwrap();
    let ScriptOutput(text) = settings
        .evaluate(
            "const endpoint = intelligence.createEndpoint({ kind: 'custom', identifier: 'ollama', \
               baseURL: 'http://localhost:11434/v1', dialect: Dialect.chatCompletions }); \
             const model = endpoint.addModel({ identifier: 'llama3', contextSize: 8192 }); \
             const added = endpoint.models().map((m) => m.modelIdentifier); \
             model.remove(); \
             const removed = endpoint.models().length; \
             endpoint.delete(); \
             let task; \
             try { cohesion.workspace('Home').createTask({ title: 'no' }); } \
             catch (e) { task = e.message.includes('`cohesion:write`'); } \
             [added, removed, intelligence.endpoints().length, task]"
                .to_string(),
        )
        .unwrap();
    assert_eq!(text, r#"[["llama3"],0,0,true]"#);

    let data = Sandbox::new(
        SandboxOptions::STANDARD,
        db.writing([WriteArea::UserData]),
        mounts,
        None,
    )
    .unwrap();
    let ScriptOutput(text) = data
        .evaluate(
            "cohesion.workspace('Home').createTask({ title: 'yes' }); \
             try { intelligence.createEndpoint({ kind: 'custom', identifier: 'ollama', \
               baseURL: 'http://localhost:11434/v1', dialect: Dialect.chatCompletions }); false } \
             catch (e) { e.message.includes('`intelligence`') }"
                .to_string(),
        )
        .unwrap();
    assert_eq!(text, "true");
}

/// Records one call of `model` with `tokens` of each kind, costing `cost`
/// dollars for each.
fn record_cost(db: &DatabaseContext, model: &str, tokens: i64, cost: f64) {
    let thread = db
        .send_blocking(CreateIntelligenceThreadCommand {
            system_prompt: String::new(),
        })
        .unwrap();
    db.send_blocking(CreateIntelligenceCostEventCommand {
        thread_id: thread.id,
        provider_identifier: "anthropic".to_string(),
        model_identifier: model.to_string(),
        input_tokens: tokens,
        output_tokens: tokens,
        cache_read_tokens: tokens,
        cache_write_tokens: tokens,
        total_cost: cost * 4.0,
        input_cost: Some(cost),
        output_cost: Some(cost),
        cache_read_cost: Some(cost),
        cache_write_cost: Some(cost),
    })
    .unwrap();
}

#[test]
fn usage_totals_every_models_calls() {
    let db = database();
    record_cost(&db, "haiku", 10, 0.25);
    record_cost(&db, "haiku", 20, 0.25);
    record_cost(&db, "opus", 5, 1.0);
    let sandbox = Sandbox::new(
        SandboxOptions::STANDARD,
        db.read_only(),
        &[Mount::Usage],
        None,
    )
    .unwrap();

    let ScriptOutput(text) = sandbox
        .evaluate(
            "const total = usage.total();\n\
             [total instanceof Usage, total.calls, total.inputTokens, total.cacheWriteTokens, \
              total.cost, usage.byModel().map(m => [m.model, m.calls]), \
              usage.total({ days: '2000-01-01' }).calls]"
                .to_string(),
        )
        .unwrap();
    assert_eq!(text, r#"[true,3,35,35,6,[["opus",1],["haiku",2]],0]"#);
}

/// The isolated worker, which reads scripts no one has trusted yet, holds
/// far less than the pools that run trusted ones.
#[test]
fn the_isolated_worker_holds_less_memory() {
    let engine = ScriptEngine::new(database().read_write());
    let script = "'x'.repeat(10 * 1024 * 1024).length".to_string();
    let ScriptOutput(length) = block_on(engine.general_purpose_run(script.clone())).unwrap();
    assert_eq!(length, "10485760");
    let isolated = block_on(engine.isolated(move |sandbox| sandbox.evaluate(script)));
    assert!(
        isolated.is_err(),
        "10 MB should not fit in the isolated worker"
    );
}

#[test]
fn usage_is_absent_without_its_mount() {
    let sandbox = Sandbox::new(
        SandboxOptions::STANDARD,
        database().read_write(),
        &[Mount::Cohesion],
        None,
    )
    .unwrap();
    let ScriptOutput(text) = sandbox.evaluate("typeof usage".to_string()).unwrap();
    assert_eq!(text, r#""undefined""#);
}

#[test]
fn typescript_types_are_stripped() {
    let sandbox =
        Sandbox::new(SandboxOptions::STANDARD, database().read_write(), &[], None).unwrap();
    let ScriptOutput(text) = sandbox
        .evaluate(
            "interface Point { x: number }\n\
             type Pair = [number, number];\n\
             function sum<T extends Point>(p: T, extra?: number): number {\n\
                 return p.x + (extra ?? 0);\n\
             }\n\
             const pair = [1, 2] as Pair;\n\
             sum({ x: 3 } satisfies Point, pair[1]!)"
                .to_string(),
        )
        .unwrap();
    assert_eq!(text, "5");
}

#[test]
fn stripped_typescript_runs_as_written() {
    let sandbox =
        Sandbox::new(SandboxOptions::STANDARD, database().read_write(), &[], None).unwrap();
    let cases = [
        // An erased statement doesn't turn the next line into a call.
        ("const a = 1\ntype T = number\n(2)", "2"),
        // A return type on its own line.
        ("const f = (a: number):\n  number => a + 1;\nf(1)", "2"),
        (
            "abstract class A<T> {\n\
               private readonly items: T[] = [];\n\
               declare size: number;\n\
               abstract name(): string;\n\
               add(item: T): this;\n\
               add(item: T): this { this.items.push(item); return this; }\n\
               get count(): number { return this.items.length; }\n\
             }\n\
             class B extends A<number> implements Named { name() { return \"b\"; } }\n\
             interface Named { name(): string }\n\
             new B().add(1).add(2).count",
            "2",
        ),
        (
            "const pick = <T, K extends keyof T>(from: T, key: K): T[K] => from[key];\n\
             const sizes = new Map<string, number>([[\"a\", 1]]);\n\
             pick({ a: sizes.get(\"a\")! < 2 ? `${1 as number}` : \"\" }, \"a\")",
            "\"1\"",
        ),
    ];
    for (script, expected) in cases {
        let ScriptOutput(text) = sandbox.evaluate(script.to_string()).unwrap();
        assert_eq!(text, expected, "{script}");
    }
    let refused = sandbox.evaluate("enum Colour { Red }".to_string());
    assert!(matches!(refused, Err(ScriptError::Exception { .. })));
}

fn create_task(db: &DatabaseContext, workspace_id: i64, title: &str) -> i64 {
    db.send_blocking(CreateTaskCommand {
        workspace_id,
        title: title.to_string(),
        folder_id: None,
        priority: None,
        size: None,
        assignee: None,
        description: None,
        start_date: None,
        due_date: None,
        tag_ids: Vec::new(),
    })
    .unwrap()
    .id
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(future)
}

/// Which globals a script sees, as `typeof` each.
const SEEN_GLOBALS: &str = "[typeof cohesion, typeof agent, typeof network, \
                             typeof log, typeof intelligence]";

/// Runs `script` as a turn of agent `agent_id`'s that an event started,
/// continuing `trace`.
trait EventTurns {
    fn event_turn_run(
        &self,
        agent_id: i64,
        trace: DatabaseEventTrace,
        script: String,
    ) -> impl std::future::Future<
        Output = Result<(ScriptOutput, crate::Effects<ForegroundEffect>), ScriptError>,
    >;
}

impl EventTurns for ScriptEngine {
    fn event_turn_run(
        &self,
        agent_id: i64,
        trace: DatabaseEventTrace,
        script: String,
    ) -> impl std::future::Future<
        Output = Result<(ScriptOutput, crate::Effects<ForegroundEffect>), ScriptError>,
    > {
        self.traced_foreground_agent_run(
            agent_id,
            script,
            ScriptStop::default(),
            trace.started_by(cohesion_core::model::Initiative::Event),
        )
    }
}

#[test]
fn foreground_agent_sees_cohesion_and_agent_only() {
    let db = database();
    create_workspace(&db, "Home");
    let engine = ScriptEngine::new(db.read_write());
    let script = format!("cohesion.workspace('Home').createTask({{ title: 't' }}); {SEEN_GLOBALS}");
    let (ScriptOutput(text), effects) =
        block_on(engine.foreground_agent_run(1, script, ScriptStop::default())).unwrap();
    assert_eq!(
        text,
        r#"["object","object","object","undefined","undefined"]"#
    );
    assert!(effects.into_parts().0.is_empty());
}

#[test]
fn a_turn_an_event_started_reads_and_has_no_network() {
    let db = database();
    create_workspace(&db, "Home");
    let engine = ScriptEngine::new(db.read_write());
    let script = format!(
        "const wrote = (() => {{ \
             try {{ cohesion.workspace('Home').createTask({{ title: 't' }}); return true; }} \
             catch (e) {{ return false; }} \
         }})(); \
         [wrote, cohesion.workspace('Home').title, \
          {SEEN_GLOBALS}.map(t => t === 'object')]"
    );
    let (ScriptOutput(text), _) =
        block_on(engine.event_turn_run(1, DatabaseEventTrace::default(), script)).unwrap();
    assert_eq!(text, r#"[false,"Home",[true,true,false,false,false]]"#);
    assert_eq!(
        json(&db, "cohesion.workspace('Home').tasks().length"),
        serde_json::json!(0)
    );
}

#[test]
fn a_cue_only_reads() {
    let db = database();
    create_workspace(&db, "Home");
    let engine = ScriptEngine::new(db.read_write());
    let probe = "(() => { \
             try { cohesion.workspace('Home').createTask({ title: 't' }); return true; } \
             catch (e) { return false; } \
         })()";
    let cue = block_on(engine.evaluate_cue(
        1,
        DatabaseEventTrace::default(),
        format!("() => [{probe}, typeof network, typeof agent, typeof actors, typeof connectors]"),
        Vec::new(),
    ))
    .unwrap();
    assert_eq!(
        cue.payload.0,
        r#"[false,"undefined","undefined","undefined","undefined"]"#
    );
}

#[test]
fn agent_effects_are_collected_in_call_order() {
    let engine = ScriptEngine::new(database().read_write());
    let (_, foreground) = block_on(engine.foreground_agent_run(
        7,
        "agent.cue({ name: ' Triage ', hook: 'task:create', code: '(session) => true', instructions: 'triage it', effort: IntelligenceEffort.low }); \
         agent.cue({ name: 'Follow the session', hook: 'actor:moved', code: '(session) => true', instructions: 'tell the user', effort: 'high' }); \
         agent.removeCue('Follow up');"
            .to_string(), ScriptStop::default(),
    ))
    .unwrap();
    assert_eq!(
        foreground.into_parts().0,
        vec![
            ForegroundEffect::Cue {
                name: "Triage".to_string(),
                hook: HookKind::TaskCreate,
                code: "(session) => true".to_string(),
                instructions: "triage it".to_string(),
                effort: IntelligenceEffort::Low,
            },
            ForegroundEffect::Cue {
                name: "Follow the session".to_string(),
                hook: HookKind::ActorMoved,
                code: "(session) => true".to_string(),
                instructions: "tell the user".to_string(),
                effort: IntelligenceEffort::High,
            },
            ForegroundEffect::RemoveCue("Follow up".to_string()),
        ]
    );
}

#[test]
fn agent_effects_are_dropped_when_the_script_fails() {
    let sandbox = Sandbox::new(
        SandboxOptions::STANDARD,
        database().read_write(),
        &[Mount::Agent],
        None,
    )
    .unwrap();
    let failed = sandbox.collecting_effects::<ForegroundEffect, _>(|sandbox| {
        sandbox.evaluate("agent.removeCue('Triage'); throw new Error('no')".to_string())
    });
    assert!(failed.is_err());
    let (_, effects) = sandbox
        .collecting_effects::<ForegroundEffect, _>(|sandbox| sandbox.evaluate("0".to_string()))
        .unwrap();
    assert!(effects.into_parts().0.is_empty());
    // Outside a collecting job, the globals throw rather than record.
    assert!(
        sandbox
            .evaluate("agent.removeCue('x')".to_string())
            .is_err()
    );
}

#[test]
fn a_panicking_job_leaves_no_trace_on_the_sandbox() {
    let sandbox =
        Sandbox::new(SandboxOptions::STANDARD, database().read_write(), &[], None).unwrap();
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        sandbox.traced(
            DatabaseEventTrace::default(),
            |sandbox| -> Result<(), ScriptError> {
                assert!(sandbox.has_trace());
                panic!("the job panics")
            },
        )
    }));
    assert!(panicked.is_err());
    assert!(!sandbox.has_trace());
}

#[test]
fn agent_cue_requires_a_hook_and_a_name() {
    let engine = ScriptEngine::new(database().read_write());
    let (ScriptOutput(text), effects) = block_on(
        engine.foreground_agent_run(
            1,
            "[['Watch', 'cohesion:write'], ['Watch', 'network'], ['Watch', 'hook:task:create'], \
              ['  ', 'task:create']].map(([name, hook]) => { \
             try { agent.cue({ name, hook, code: '() => true', instructions: '', effort: 'low' }); return 'set'; } \
             catch (e) { return e instanceof TypeError; } \
         })"
            .to_string(), ScriptStop::default(),
        ),
    )
    .unwrap();
    assert_eq!(text, "[true,true,true,true]");
    assert!(effects.into_parts().0.is_empty());
}

#[test]
fn evaluate_cue_calls_the_function_with_the_subject() {
    let db = database();
    let workspace_id = create_workspace(&db, "Home");
    let task_id = create_task(&db, workspace_id, "Water plants");
    let engine = ScriptEngine::new(db.read_write());
    let evaluate = |code: &str| {
        block_on(engine.evaluate_cue(
            1,
            DatabaseEventTrace::default(),
            code.to_string(),
            vec![ScriptArgument::Task {
                workspace_id,
                task_id,
            }],
        ))
    };

    let fired = evaluate("(task) => task.title === 'Water plants'").unwrap();
    assert!(fired.fired);
    let ScriptOutput(subject) = fired.subject;
    let subject: serde_json::Value = serde_json::from_str(&subject).unwrap();
    assert_eq!(subject["title"], "Water plants");

    assert!(
        !evaluate("(task) => task.title === 'Feed cat'")
            .unwrap()
            .fired
    );
    assert!(evaluate("(task: Task) => task.title.length").unwrap().fired);
    assert!(!evaluate("() => 0").unwrap().fired);

    for not_a_function in ["1 + 1", "function named() { return true; }"] {
        assert!(
            matches!(evaluate(not_a_function), Err(ScriptError::Exception { .. })),
            "{not_a_function}"
        );
    }

    let launch = block_on(engine.evaluate_cue(
        1,
        DatabaseEventTrace::default(),
        "() => true".to_string(),
        Vec::new(),
    ))
    .unwrap();
    assert!(launch.fired);
    assert_eq!(launch.subject, ScriptOutput("null".to_string()));
}

#[test]
fn evaluate_cue_payload_is_what_the_function_returned() {
    let db = database();
    let workspace_id = create_workspace(&db, "Home");
    let task_id = create_task(&db, workspace_id, "Water plants");
    let engine = ScriptEngine::new(db.read_write());
    let evaluate = |code: &str| {
        block_on(engine.evaluate_cue(
            1,
            DatabaseEventTrace::default(),
            code.to_string(),
            vec![ScriptArgument::Task {
                workspace_id,
                task_id,
            }],
        ))
        .unwrap()
    };
    let json = |ScriptOutput(text): &ScriptOutput| -> serde_json::Value {
        serde_json::from_str(text).unwrap()
    };

    let filtered = evaluate("(task) => ({ title: task.title, urgent: false })");
    assert!(filtered.fired);
    assert_eq!(
        json(&filtered.payload),
        serde_json::json!({ "title": "Water plants", "urgent": false })
    );
    assert_eq!(json(&filtered.subject)["title"], "Water plants");

    let title = evaluate("(task) => task.title");
    assert!(title.fired);
    assert_eq!(json(&title.payload), serde_json::json!("Water plants"));

    let whole = evaluate("() => true");
    assert!(whole.fired);
    assert_eq!(whole.payload, whole.subject);

    for falsy in ["() => false", "() => ''", "() => null", "() => undefined"] {
        assert!(!evaluate(falsy).fired, "{falsy}");
    }
}

/// Keeps the trace of every event the database raises; clones share the list.
#[derive(Clone, Default)]
struct RecordedTraces(std::sync::Arc<std::sync::Mutex<Vec<cohesion_db::DatabaseEventTrace>>>);
impl std::fmt::Debug for RecordedTraces {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecordedTraces")
    }
}
impl cohesion_db::DatabaseEventListener for RecordedTraces {
    fn send(&self, event: cohesion_db::DatabaseEvent) {
        self.0.lock().unwrap().push(event.trace);
    }
}

#[test]
fn foreground_agent_writes_are_traced_to_the_agent() {
    let traces = RecordedTraces::default();
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    let db = DatabaseContext::new(container, Secrets::new(), traces.clone());
    create_workspace(&db, "Home");
    let engine = ScriptEngine::new(db.read_write());
    let create = "cohesion.workspace('Home').createTask({ title: 't' })";
    _ = block_on(engine.foreground_agent_run(7, create.to_string(), ScriptStop::default()))
        .unwrap();
    block_on(engine.general_purpose_run(create.to_string())).unwrap();

    let traces = traces.0.lock().unwrap();
    let [workspace, agent, general] = &traces[..] else {
        panic!("expected three events, got {}", traces.len());
    };
    assert!(!workspace.is_from_agent(7));
    assert!(agent.is_from_agent(7));
    assert!(!agent.is_from_agent(8));
    assert!(!agent.is_from_extension("agent:7"));
    assert!(!general.is_from_agent(7));
}

#[test]
fn a_jobs_notifications_continue_the_waking_trace() {
    let traces = RecordedTraces::default();
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    let db = DatabaseContext::new(container, Secrets::new(), traces.clone());
    create_workspace(&db, "Home");
    let engine = ScriptEngine::new(db.read_write());
    let woke = DatabaseEventTrace::default()
        .event(DatabaseEventKind::WorkspaceCreated { workspace_id: 1 });
    let notify = "notifications.send('Rent', 'Due today')";
    _ = block_on(engine.event_turn_run(7, woke.trace.clone(), notify.to_string())).unwrap();
    let refused = block_on(engine.event_turn_run(
        7,
        DatabaseEventTrace::default(),
        "cohesion.workspace('Home').createTask({ title: 't' })".to_string(),
    ));
    assert!(refused.is_err(), "a job still can't write the user's data");

    let traces = traces.0.lock().unwrap();
    let [_workspace, notification] = &traces[..] else {
        panic!("expected two events, got {}", traces.len());
    };
    assert!(notification.is_from_agent(7));
    assert_eq!(notification.depth(), woke.trace.depth() + 1);
}

/// Routes `services` calls to the scopes spawned on its engine, as the
/// daemon does but without its checks, recording each call's name and
/// whether its caller was read-only.
struct ScopedServices {
    db: DatabaseContext,
    engine: ScriptEngine,
    calls: std::sync::Mutex<Vec<(String, bool)>>,
}
impl ServiceAccess for ScopedServices {
    fn call(&self, call: ServiceCall) -> Result<serde_json::Value, ServerError> {
        self.calls
            .lock()
            .unwrap()
            .push((call.name.clone(), call.read_only));
        let identifier = self
            .db
            .send_blocking(GetExtensionCommand {
                query: GetExtensionQuery::Id(call.extension_id),
            })
            .unwrap()
            .script_identifier;
        let scope = self
            .engine
            .scope(&SandboxKey::Extension(identifier))
            .unwrap();
        let arguments = vec![
            ScriptArgument::String(call.name),
            ScriptArgument::Json(call.args),
        ];
        block_on(scope.run(move |sandbox| {
            sandbox.call_returning_with_arguments("serviceRequest", &arguments)
        }))
        .map_err(|e| ServerError::validation(None, e.to_string()))
    }
}

/// An engine whose `services` reach the scopes spawned on it.
fn serving_engine(db: &DatabaseContext) -> (ScriptEngine, Arc<ScopedServices>) {
    let engine = ScriptEngine::new(db.read_write());
    let services = Arc::new(ScopedServices {
        db: db.clone(),
        engine: engine.clone(),
        calls: Default::default(),
    });
    (engine.with_services(services.clone()), services)
}

/// Installs `identifier` listing `permission`, declaring each of
/// `functions` (a name, whether it takes an argument, whether it is
/// read-only), or no `service` block for `None`. Grants nothing; answers
/// its id.
fn install_service(
    db: &DatabaseContext,
    identifier: &str,
    permission: ExtensionPermissionKind,
    functions: Option<&[(&str, bool, bool)]>,
) -> i64 {
    let service = functions.map(|functions| ServiceManifest {
        description: None,
        functions: functions
            .iter()
            .map(|&(name, takes_args, read_only)| {
                let function = ServiceFunctionManifest {
                    description: None,
                    args: takes_args.then(|| "unknown".to_string()),
                    returns: None,
                    read_only: Some(read_only),
                };
                (name.to_string(), function)
            })
            .collect(),
    });
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
            permissions: vec![permission, ExtensionPermissionKind::Services],
            hooks: vec![],
            service,
        },
        allow_overwrite: false,
    })
    .unwrap()
    .id
}

fn grant(db: &DatabaseContext, extension_id: i64, permission: ExtensionPermissionKind) {
    db.send_blocking(SetExtensionPermissionCommand {
        extension_id,
        permission,
        enabled: true,
    })
    .unwrap();
}

/// Spawns the scope of extension `identifier`, with `services` mounted,
/// running `body` against `database`.
fn spawn_scope(
    engine: &ScriptEngine,
    identifier: &str,
    database: cohesion_db::GuardedDatabaseContext,
    body: &str,
) {
    block_on(engine.spawn_scope(
        extension(identifier),
        body.to_string(),
        Cow::Owned(vec![Mount::Services]),
        database.for_extension(Arc::new(identifier.to_string())),
    ))
    .unwrap();
}

/// Installs `identifier` as a service granted `service`, as
/// [`install_service`] does, and spawns its read-write scope.
fn spawn_service(
    db: &DatabaseContext,
    engine: &ScriptEngine,
    identifier: &str,
    functions: &[(&str, bool, bool)],
    body: &str,
) {
    use ExtensionPermissionKind::Service;
    let id = install_service(db, identifier, Service, Some(functions));
    grant(db, id, Service);
    spawn_scope(engine, identifier, db.read_write(), body);
}

const COUNTER: &str = "let count = 0; \
    function serviceRequest(name, args) { \
        if (name === 'add') count += args.by; \
        return { count, args }; \
    }";

/// `call`'s result, or `"refused"` when it throws.
fn refused(call: &str) -> String {
    format!("(() => {{ try {{ return {call}; }} catch (e) {{ return 'refused'; }} }})()")
}

#[test]
fn a_service_function_answers_what_its_service_returns() {
    let db = database();
    let (engine, _) = serving_engine(&db);
    spawn_service(
        &db,
        &engine,
        "com.example.counter",
        &[("add", true, false), ("read", false, true)],
        COUNTER,
    );
    let script = "const counter = services.use('com.example.counter'); \
                  [counter.add({ by: 2 }).count, counter.add({ by: 3 }).count, \
                   counter.read('ignored').args, Object.keys(counter), typeof counter.reset, \
                   Object.isFrozen(counter)]";
    let ScriptOutput(text) = block_on(engine.general_purpose_run(script.to_string())).unwrap();
    assert_eq!(text, r#"[2,5,null,["add","read"],"undefined",true]"#);
}

#[test]
fn read_only_callers_see_every_function_and_call_as_read_only() {
    let db = database();
    let (engine, services) = serving_engine(&db);
    spawn_service(
        &db,
        &engine,
        "com.example.counter",
        &[("add", true, false), ("read", false, true)],
        COUNTER,
    );
    let script = "[Object.keys(services.use('com.example.counter')), \
                   services.use('com.example.counter').read().count]";
    let (ScriptOutput(text), _) =
        block_on(engine.event_turn_run(1, DatabaseEventTrace::default(), script.to_string()))
            .unwrap();
    assert_eq!(text, r#"[["add","read"],0]"#);
    _ = block_on(engine.foreground_agent_run(
        1,
        "services.use('com.example.counter').add({ by: 1 })".to_string(),
        ScriptStop::default(),
    ))
    .unwrap();

    // An extension without `cohesion:write` calls as read-only too.
    spawn_scope(
        &engine,
        "com.example.reader",
        db.read_only(),
        "function go() { return services.use('com.example.counter').read().count; }",
    );
    let count: i64 = block_on(
        engine
            .scope(&extension("com.example.reader"))
            .unwrap()
            .run(|sandbox| sandbox.call_returning("go")),
    )
    .unwrap();
    assert_eq!(count, 1);
    assert_eq!(
        *services.calls.lock().unwrap(),
        [
            ("read".to_string(), true),
            ("add".to_string(), false),
            ("read".to_string(), true)
        ]
    );
}

#[test]
fn only_a_callable_service_can_be_used() {
    use ExtensionPermissionKind::{Service, ServiceIsolated};
    let db = database();
    let (engine, services) = serving_engine(&db);
    let functions: &[_] = &[("read", false, true)];
    let isolated = install_service(
        &db,
        "com.example.isolated",
        ServiceIsolated,
        Some(functions),
    );
    grant(&db, isolated, ServiceIsolated);
    install_service(&db, "com.example.disabled", Service, Some(functions));
    let undeclared = install_service(&db, "com.example.undeclared", Service, None);
    grant(&db, undeclared, Service);

    let uses = ["missing", "isolated", "disabled", "undeclared"]
        .map(|name| refused(&format!("services.use('com.example.{name}')")));
    let script = format!("[{}]", uses.join(", "));
    let ScriptOutput(text) = block_on(engine.general_purpose_run(script)).unwrap();
    assert_eq!(text, r#"["refused","refused","refused","refused"]"#);
    assert!(services.calls.lock().unwrap().is_empty());
}

#[test]
fn a_cue_calls_services_as_read_only() {
    let db = database();
    let (engine, services) = serving_engine(&db);
    spawn_service(
        &db,
        &engine,
        "com.example.counter",
        &[("add", true, false), ("read", false, true)],
        COUNTER,
    );
    let cue = "() => services.use('com.example.counter').read().count".to_string();
    let evaluated =
        block_on(engine.evaluate_cue(1, DatabaseEventTrace::default(), cue, Vec::new())).unwrap();
    assert_eq!(evaluated.payload, ScriptOutput("0".to_string()));
    assert_eq!(
        *services.calls.lock().unwrap(),
        [("read".to_string(), true)]
    );
}

#[test]
fn a_service_cannot_call_back_into_itself() {
    let db = database();
    let (engine, _) = serving_engine(&db);
    spawn_service(
        &db,
        &engine,
        "com.example.echo",
        &[("again", false, false)],
        &format!(
            "function serviceRequest() {{ return {}; }}",
            refused("services.use('com.example.echo').again()")
        ),
    );
    spawn_service(
        &db,
        &engine,
        "com.example.ping",
        &[("go", false, false), ("leaf", false, false)],
        "function serviceRequest(name) { \
             return name === 'go' ? services.use('com.example.pong').back() : 'leaf'; \
         }",
    );
    spawn_service(
        &db,
        &engine,
        "com.example.pong",
        &[("back", false, false), ("direct", false, false)],
        &format!(
            "function serviceRequest(name) {{ \
                 return name === 'back' ? {} : services.use('com.example.ping').leaf(); \
             }}",
            refused("services.use('com.example.ping').go()")
        ),
    );
    // Once a call returns, its service may be called again.
    let script = "[services.use('com.example.echo').again(), \
                   services.use('com.example.ping').go(), \
                   services.use('com.example.pong').direct()]";
    let ScriptOutput(text) = block_on(engine.general_purpose_run(script.to_string())).unwrap();
    assert_eq!(text, r#"["refused","refused","leaf"]"#);
}

#[test]
fn a_throwing_service_keeps_serving() {
    let db = database();
    let (engine, _) = serving_engine(&db);
    spawn_service(
        &db,
        &engine,
        "com.example.flaky",
        &[("fail", false, false), ("ok", false, false)],
        "function serviceRequest(name) { if (name === 'fail') throw new Error('no'); return 'ok'; }",
    );
    let script = "const flaky = services.use('com.example.flaky'); \
                  [(() => { try { flaky.fail(); } catch (e) { return e instanceof Error; } })(), \
                   flaky.ok()]";
    let ScriptOutput(text) = block_on(engine.general_purpose_run(script.to_string())).unwrap();
    assert_eq!(text, r#"[true,"ok"]"#);
}

/// Installs `identifier` granting nothing, so its scope's `cohesion` would
/// be read-only, and spawns that scope with only `storage` mounted.
fn spawn_storage_scope(db: &DatabaseContext, engine: &ScriptEngine, identifier: &str) {
    block_on(
        engine.spawn_scope(
            extension(identifier),
            String::new(),
            Cow::Owned(vec![Mount::Storage]),
            db.read_only()
                .for_extension(Arc::new(identifier.to_string())),
        ),
    )
    .unwrap();
}

/// Evaluates `script` in the running scope `identifier`, as JSON text.
fn in_scope(engine: &ScriptEngine, identifier: &str, script: &str) -> String {
    let script = script.to_string();
    let ScriptOutput(text) = block_on(
        engine
            .scope(&extension(identifier))
            .unwrap()
            .run(move |sandbox| sandbox.evaluate(script)),
    )
    .unwrap();
    text
}

#[test]
fn an_extension_without_permissions_keeps_its_own_storage_across_restarts() {
    let db = database();
    let engine = ScriptEngine::new(db.read_write());
    install_service(&db, "com.example.one", ExtensionPermissionKind::Timer, None);
    install_service(&db, "com.example.two", ExtensionPermissionKind::Timer, None);
    spawn_storage_scope(&db, &engine, "com.example.one");
    spawn_storage_scope(&db, &engine, "com.example.two");

    assert_eq!(
        in_scope(
            &engine,
            "com.example.one",
            "storage.set('cursor', { page: 2, ids: [1, 2] }); storage.set('flag', true); \
             storage.set('gone', 'soon'); storage.remove('gone'); storage.set('flag', null); \
             [storage.get('cursor'), storage.get('gone'), storage.keys()]",
        ),
        r#"[{"page":2,"ids":[1,2]},null,["cursor"]]"#
    );
    assert_eq!(
        in_scope(
            &engine,
            "com.example.two",
            "[storage.get('cursor'), storage.keys()]"
        ),
        "[null,[]]"
    );
    assert_eq!(
        in_scope(
            &engine,
            "com.example.one",
            "try { storage.set('f', () => 1); 'stored' } catch (e) { e instanceof TypeError }",
        ),
        "true"
    );

    engine.shutdown_scope(&extension("com.example.one"));
    spawn_storage_scope(&db, &engine, "com.example.one");
    assert_eq!(
        in_scope(&engine, "com.example.one", "storage.get('cursor').page"),
        "2"
    );
}

#[test]
fn an_agent_keeps_one_storage_across_its_conversation_runs_and_cues() {
    let db = database();
    let engine = ScriptEngine::new(db.read_write());
    let agent = |name: &str| {
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
    };
    let (scout, other) = (agent("Scout"), agent("Other"));

    let evaluation = block_on(engine.evaluate_cue(
        scout,
        DatabaseEventTrace::default(),
        "() => { storage.set('fired', (storage.get('fired') ?? 0) + 1); return true; }".to_string(),
        Vec::new(),
    ))
    .unwrap();
    assert!(evaluation.fired);
    _ = block_on(engine.event_turn_run(
        scout,
        DatabaseEventTrace::default(),
        "storage.set('fired', storage.get('fired') + 1)".to_string(),
    ))
    .unwrap();
    let (ScriptOutput(text), _) = block_on(engine.foreground_agent_run(
        scout,
        "[storage.get('fired'), storage.keys()]".to_string(),
        ScriptStop::default(),
    ))
    .unwrap();
    assert_eq!(text, r#"[2,["fired"]]"#);

    let (ScriptOutput(text), _) = block_on(engine.foreground_agent_run(
        other,
        "storage.get('fired')".to_string(),
        ScriptStop::default(),
    ))
    .unwrap();
    assert_eq!(text, "null");
    assert_eq!(
        json(&db, &refused("storage.get('fired')")),
        serde_json::json!("refused")
    );
}

#[test]
fn an_extension_named_like_an_agent_cannot_reach_its_storage_or_scope() {
    let db = database();
    let engine = ScriptEngine::new(db.read_write());
    let agent_id = db
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
        .unwrap()
        .id;
    _ = block_on(engine.foreground_agent_run(
        agent_id,
        "storage.set('secret', 42)".to_string(),
        ScriptStop::default(),
    ))
    .unwrap();

    let impostor = format!("agent:{agent_id}");
    install_service(&db, &impostor, ExtensionPermissionKind::Timer, None);
    spawn_storage_scope(&db, &engine, &impostor);
    assert_eq!(
        in_scope(
            &engine,
            &impostor,
            "storage.set('agent:1:secret', 1); [storage.get('secret'), storage.keys()]",
        ),
        r#"[null,["agent:1:secret"]]"#
    );
    assert!(!engine.is_scope_running(&SandboxKey::Agent(agent_id)));

    let (ScriptOutput(text), _) = block_on(engine.foreground_agent_run(
        agent_id,
        "[storage.get('secret'), storage.keys()]".to_string(),
        ScriptStop::default(),
    ))
    .unwrap();
    assert_eq!(text, r#"[42,["secret"]]"#);
}

/// A sandbox with `network` and `seconds` to run in.
fn network_sandbox(seconds: u64) -> Sandbox {
    let options = SandboxOptions {
        time_limit: std::time::Duration::from_secs(seconds),
        ..SandboxOptions::STANDARD
    };
    Sandbox::new(options, database().read_only(), &[Mount::Network], None).unwrap()
}

/// Accepts one connection on a local port and answers `answer` to it (nothing
/// for `None`, which holds the connection open), returning the address.
fn listen_once(answer: Option<Vec<u8>>) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0u8; 1024];
        _ = stream.read(&mut request);
        match answer {
            Some(bytes) => _ = stream.write_all(&bytes),
            None => std::thread::sleep(std::time::Duration::from_secs(30)),
        }
    });
    format!("http://{address}/")
}

#[test]
fn fetch_gives_up_at_the_scripts_deadline_not_its_own_timeout() {
    // The listener never answers; the fetch's own limit is 20 seconds.
    let url = listen_once(None);
    // Building the sandbox migrates a database, which isn't what's being timed.
    let sandbox = network_sandbox(1);
    let started = std::time::Instant::now();
    let result = sandbox.evaluate(format!("network.fetch({{ url: '{url}', method: 'GET' }})"));
    assert!(result.is_err());
    assert!(started.elapsed() < std::time::Duration::from_secs(15));
}

#[test]
fn fetch_refuses_a_body_over_the_cap() {
    let size = 32 * 1024 * 1024 + 1;
    let mut answer = format!("HTTP/1.1 200 OK\r\nContent-Length: {size}\r\n\r\n").into_bytes();
    answer.resize(answer.len() + size, b'x');
    let url = listen_once(Some(answer));
    let Err(ScriptError::Exception { message, .. }) =
        network_sandbox(60).evaluate(format!("network.fetch({{ url: '{url}', method: 'GET' }})"))
    else {
        panic!("a body over the cap should throw");
    };
    assert!(message.contains("32 MiB"), "{message}");

    let url = listen_once(Some(
        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nhi".to_vec(),
    ));
    let ScriptOutput(text) = network_sandbox(60)
        .evaluate(format!(
            "const r = network.fetch({{ url: '{url}', method: 'GET' }}); [r.code, r.body]"
        ))
        .unwrap();
    assert_eq!(text, r#"[200,"hi"]"#);
}

/// Accepts one connection, answers 200 and returns what the request was.
fn listen_and_record() -> (String, std::sync::mpsc::Receiver<String>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut chunk = [0u8; 1024];
        loop {
            let read = stream.read(&mut chunk).unwrap();
            request.extend_from_slice(&chunk[..read]);
            let text = String::from_utf8_lossy(&request).to_string();
            if let Some((head, body)) = text.split_once("\r\n\r\n") {
                let length = head
                    .lines()
                    .find_map(|l| {
                        l.to_lowercase()
                            .strip_prefix("content-length:")
                            .map(str::to_owned)
                    })
                    .and_then(|v| v.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if body.len() >= length {
                    break;
                }
            }
            if read == 0 {
                break;
            }
        }
        _ = stream.write_all(b"HTTP/1.1 207 Multi-Status\r\nContent-Length: 2\r\n\r\nok");
        _ = sender.send(String::from_utf8_lossy(&request).to_string());
    });
    (format!("http://{address}/"), receiver)
}

#[test]
fn fetch_sends_a_custom_method_with_a_text_body_and_its_content_type() {
    let (url, request) = listen_and_record();
    let ScriptOutput(text) = network_sandbox(60)
        .evaluate(format!(
            "const r = network.fetch({{ url: '{url}', method: 'PROPFIND', \
             headers: {{ 'Content-Type': 'application/xml; charset=utf-8', Depth: '1' }}, \
             textBody: '<d:propfind xmlns:d=\"DAV:\"/>' }}); [r.code, r.body]"
        ))
        .unwrap();
    assert_eq!(text, r#"[207,"ok"]"#);
    let request = request.recv().unwrap();
    assert!(request.starts_with("PROPFIND / HTTP/1.1"), "{request}");
    let lower = request.to_lowercase();
    assert!(
        lower.contains("content-type: application/xml; charset=utf-8"),
        "{request}"
    );
    assert!(lower.contains("depth: 1"), "{request}");
    assert!(
        request.ends_with("<d:propfind xmlns:d=\"DAV:\"/>"),
        "{request}"
    );
}

#[test]
fn fetch_still_sends_json_and_form_bodies_with_the_enum_methods() {
    let (url, request) = listen_and_record();
    network_sandbox(60)
        .evaluate(format!(
            "network.fetch({{ url: '{url}', method: NetworkMethod.post, jsonBody: {{ a: 1 }} }})"
        ))
        .unwrap();
    let request = request.recv().unwrap();
    assert!(request.starts_with("POST / HTTP/1.1"), "{request}");
    assert!(
        request
            .to_lowercase()
            .contains("content-type: application/json"),
        "{request}"
    );
    assert!(request.ends_with(r#"{"a":1}"#), "{request}");

    let (url, request) = listen_and_record();
    network_sandbox(60)
        .evaluate(format!(
            "network.fetch({{ url: '{url}', method: 'PUT', formBody: {{ k: 'v w' }} }})"
        ))
        .unwrap();
    let request = request.recv().unwrap();
    assert!(request.starts_with("PUT / HTTP/1.1"), "{request}");
    assert!(request.ends_with("k=v+w"), "{request}");
}

#[test]
fn fetch_rejects_an_invalid_method_and_more_than_one_body() {
    let sandbox = network_sandbox(60);
    let Err(ScriptError::Exception { message, .. }) =
        sandbox.evaluate("network.fetch({ url: 'http://127.0.0.1:9/', method: 'GET /x' })".into())
    else {
        panic!("an invalid method should throw");
    };
    assert!(message.contains("not a valid HTTP method"), "{message}");
    let Err(ScriptError::Exception { message, .. }) = sandbox.evaluate(
        "network.fetch({ url: 'http://127.0.0.1:9/', method: 'POST', textBody: 'a', jsonBody: {} })"
            .into(),
    ) else {
        panic!("two bodies should throw");
    };
    assert!(message.contains("only one of"), "{message}");
}

#[test]
fn storage_caps_a_value_and_a_whole_store() {
    let db = database();
    let engine = ScriptEngine::new(db.read_write());
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
    let run = |script: &str| {
        block_on(engine.event_turn_run(agent.id, DatabaseEventTrace::default(), script.to_string()))
    };

    let (ScriptOutput(text), _) = run("storage.set('ok', 'x'.repeat(1000000)); \
         try { storage.set('big', 'x'.repeat(1100000)); 'stored' } catch (e) { e.message }")
    .unwrap();
    assert!(text.contains("at most 1 MiB"), "{text}");

    // Fifteen more near-1 MiB values fill the 16 MiB store; replacing one
    // still fits, adding another does not.
    let (ScriptOutput(text), _) = run("const v = 'x'.repeat(1000000); \
         for (let i = 0; i < 15; i++) storage.set('k' + i, v); \
         storage.set('k0', v); \
         try { storage.set('over', v); 'stored' } catch (e) { e.message }")
    .unwrap();
    assert!(text.contains("Storage is full"), "{text}");
    let (ScriptOutput(text), _) = run(
        "storage.remove('k1'); storage.set('over', 'x'.repeat(1000000)); storage.keys().length",
    )
    .unwrap();
    assert_eq!(text, "16");
}

#[test]
fn task_folder_lookup_finds_only_live_folders_in_its_workspace() {
    let db = database();
    create_workspace(&db, "Home");
    create_workspace(&db, "Work");
    let found = json(
        &db,
        r#"
        const home = cohesion.workspace("Home");
        const work = cohesion.workspace("Work");
        const folder = home.createTaskFolder({ title: "Errands" });
        const missing = (lookup) => {
            try { lookup(); return "found"; }
            catch (e) { return e instanceof ReferenceError; }
        };
        const title = home.taskFolder(folder.id).title;
        const elsewhere = missing(() => work.taskFolder(folder.id));
        folder.delete();
        [title, elsewhere, missing(() => home.taskFolder(folder.id))]
        "#,
    );
    assert_eq!(found, serde_json::json!(["Errands", true, true]));
}

#[test]
fn a_script_colors_a_task_folder_from_the_agents_palette() {
    let db = database();
    create_workspace(&db, "Home");
    let result = json(
        &db,
        r#"
        const home = cohesion.workspace("Home");
        const folder = home.createTaskFolder({ title: "Errands" });
        const before = folder.color;
        folder.color = "teal";
        const set = [folder.color, home.taskFolder(folder.id).color];
        let refused = false;
        try { folder.color = "beige"; } catch (e) { refused = e instanceof TypeError; }
        folder.color = null;
        [before, set, refused, home.taskFolder(folder.id).color]
        "#,
    );
    assert_eq!(
        result,
        serde_json::json!([null, ["teal", "teal"], true, null])
    );
}

#[test]
fn a_script_gives_a_task_folder_a_symbol_from_the_set() {
    let db = database();
    create_workspace(&db, "Home");
    let result = json(
        &db,
        r#"
        const home = cohesion.workspace("Home");
        const folder = home.createTaskFolder({ title: "Errands" });
        const before = folder.symbol;
        folder.symbol = "cart";
        const set = [folder.symbol, home.taskFolder(folder.id).symbol];
        let refused = false;
        try { folder.symbol = "unicorn"; } catch (e) { refused = e instanceof TypeError; }
        folder.symbol = null;
        [before, set, refused, home.taskFolder(folder.id).symbol]
        "#,
    );
    assert_eq!(
        result,
        serde_json::json!([null, ["cart", "cart"], true, null])
    );
}

#[test]
fn task_and_document_lookups_find_any_status_in_their_workspace_only() {
    let db = database();
    create_workspace(&db, "Home");
    create_workspace(&db, "Work");
    let found = json(
        &db,
        r#"
        const home = cohesion.workspace("Home");
        const work = cohesion.workspace("Work");
        const task = home.createTask({ title: "Call mum" });
        task.complete();
        const doc = home.createDocument("Groceries", "Eggs");
        const entry = home.journal("2026-10-20");
        const missing = (lookup) => {
            try { lookup(); return "found"; }
            catch (e) { return e instanceof ReferenceError; }
        };
        [
            home.task(task.id).status,
            home.document(doc.id).contents,
            home.document(entry.id).journalDate,
            missing(() => work.task(task.id)),
            missing(() => work.document(doc.id)),
        ]
        "#,
    );
    assert_eq!(
        found,
        serde_json::json!(["completed", "Eggs", "2026-10-20", true, true])
    );
}

/// Records actor sessions and messages as `ActorSessions` would, without a
/// client to call.
struct RecordingActors {
    db: DatabaseContext,
}
impl ActorAccess for RecordingActors {
    fn start(
        &self,
        request: cohesion_core::protocol::StartActorSessionRequest,
        _deadline: std::time::Instant,
    ) -> Result<cohesion_core::model::ActorSession, ServerError> {
        let (session, _) = self
            .db
            .send_blocking(cohesion_db::CreateActorSessionCommand {
                actor_id: request.actor_id,
                prompt: request.prompt,
                title: request.title,
                mode: request.mode,
                model: request.model,
                effort: request.effort,
                attachments: Vec::new(),
            })
            .map_err(|e| ServerError::validation(None, format!("{e:?}")))?;
        Ok(session)
    }

    fn send(
        &self,
        session_id: i64,
        content: cohesion_core::model::ActorUserContent,
        _deadline: std::time::Instant,
    ) -> Result<cohesion_core::model::ActorSessionMessage, ServerError> {
        self.db
            .send_blocking(cohesion_db::AddActorSessionMessageCommand {
                session_id,
                content: cohesion_core::model::ActorSessionMessageContent::User(content),
                attachments: Vec::new(),
            })
            .map_err(|e| ServerError::validation(None, format!("{e:?}")))
    }

    fn stop(
        &self,
        session_id: i64,
        _deadline: std::time::Instant,
    ) -> Result<cohesion_core::model::ActorSession, ServerError> {
        self.db
            .send_blocking(cohesion_db::GetActorSessionCommand { id: session_id })
            .map_err(|e| ServerError::validation(None, format!("{e:?}")))
    }

    fn resolve(
        &self,
        session_id: i64,
        _deadline: std::time::Instant,
    ) -> Result<cohesion_core::model::ActorSession, ServerError> {
        self.db
            .send_blocking(cohesion_db::SetActorSessionStatusCommand {
                session_id,
                moved: cohesion_core::model::ActorSessionMove::Resolved,
            })
            .map_err(|e| ServerError::validation(None, format!("{e:?}")))
    }

    fn usage(&self, _client_id: i64, _identifier: &str) -> cohesion_core::protocol::ActorUsage {
        cohesion_core::protocol::ActorUsage::default()
    }
}

/// A client offering one actor; answers the actor's id.
fn offer_actor(db: &DatabaseContext) -> i64 {
    let client = db
        .send_blocking(IdentifyLocalClientCommand {
            identity: LocalIdentity {
                key: "MacBook".to_string(),
                name: "MacBook".to_string(),
            },
        })
        .unwrap();
    db.send_blocking(SyncCapabilitiesCommand {
        client_id: client.id,
        offers: ClientCapabilities {
            actors: vec![cohesion_core::model::ActorDefinition {
                identifier: "cohesion".to_string(),
                product: "Claude Code".to_string(),
                scope: "/src/cohesion".to_string(),
                modes: Vec::new(),
                models: Vec::new(),
            }],
        },
    })
    .unwrap();
    db.send_blocking(cohesion_db::ListActorsCommand).unwrap()[0].id
}

#[test]
fn agent_turns_and_runs_dispatch_to_actors_and_a_cue_cannot() {
    let db = database();
    let actor_id = offer_actor(&db);
    let engine = ScriptEngine::new(db.read_write())
        .with_actors(Arc::new(RecordingActors { db: db.clone() }));

    let foreground = format!(
        "const session = actors.dispatch({actor_id}, {{ prompt: 'Fix the build' }}); \
         actors.send(session.id, 'Also the docs'); \
         [actors.list()[0].title, session.title, session.status, \
          actors.messages(session.id).map(m => m.content.sender + ':' + m.content.text)]"
    );
    let (ScriptOutput(text), _) =
        block_on(engine.foreground_agent_run(1, foreground, ScriptStop::default())).unwrap();
    assert_eq!(
        text,
        r#"["Claude Code on MacBook","Fix the build","working",["user:Fix the build","user:Also the docs"]]"#
    );

    // A title given names the session in place of the prompt's first line.
    let titled = format!(
        "actors.dispatch({actor_id}, {{ prompt: 'Run every test', title: 'Test run' }}).title"
    );
    let (ScriptOutput(text), _) =
        block_on(engine.foreground_agent_run(1, titled, ScriptStop::default())).unwrap();
    assert_eq!(text, r#""Test run""#);

    // A job's database is read-only, and it dispatches anyway.
    let background = format!("actors.dispatch({actor_id}, {{ prompt: 'Run the tests' }}).title");
    let (ScriptOutput(text), _) =
        block_on(engine.event_turn_run(1, DatabaseEventTrace::default(), background)).unwrap();
    assert_eq!(text, r#""Run the tests""#);

    // A turn marks a session read for the user, as opening it does; a
    // job, read-only, may not.
    let mark = "(() => { \
                const session = actors.sessions()[0]; \
                try { session.markRead(); } catch (e) { return 'refused'; } \
                return [session.unread, session.readAt !== null, actors.session(session.id).readAt !== null]; \
                })()";
    let (ScriptOutput(text), _) =
        block_on(engine.event_turn_run(1, DatabaseEventTrace::default(), mark.to_string()))
            .unwrap();
    assert_eq!(text, r#""refused""#);
    let (ScriptOutput(text), _) =
        block_on(engine.foreground_agent_run(1, mark.to_string(), ScriptStop::default())).unwrap();
    assert_eq!(text, "[false,true,true]");

    // A cue decides whether to wake with no side effect, reading the session
    // it moved as an `ActorSession`.
    let session = db
        .send_blocking(cohesion_db::ListActorSessionsCommand { actor_id: None })
        .unwrap()
        .remove(0);
    let cue = block_on(engine.evaluate_cue(
        1,
        DatabaseEventTrace::default(),
        "(session) => [typeof actors, session instanceof ActorSession, session.title]".to_string(),
        vec![ScriptArgument::ActorSession {
            actor_session: session,
            moved: cohesion_core::model::ActorSessionMove::Finished,
        }],
    ))
    .unwrap();
    assert_eq!(cue.payload.0, r#"["undefined",true,"Run the tests"]"#);

    // A turn resolves a session for the user, which leaves the list; a job,
    // read-only, may not.
    let resolve = "(() => { \
                   const session = actors.sessions()[0]; \
                   try { session.resolve(); } catch (e) { return 'refused'; } \
                   return [session.status, actors.session(session.id).status, \
                   actors.sessions().some(t => t.id === session.id)]; \
                   })()";
    let (ScriptOutput(text), _) =
        block_on(engine.event_turn_run(1, DatabaseEventTrace::default(), resolve.to_string()))
            .unwrap();
    assert_eq!(text, r#""refused""#);
    let (ScriptOutput(text), _) =
        block_on(engine.foreground_agent_run(1, resolve.to_string(), ScriptStop::default()))
            .unwrap();
    assert_eq!(text, r#"["resolved","resolved",false]"#);
}

#[test]
fn every_turn_follows_what_it_dispatches() {
    let db = database();
    let actor_id = offer_actor(&db);
    let engine = ScriptEngine::new(db.read_write())
        .with_actors(Arc::new(RecordingActors { db: db.clone() }));

    let script = format!(
        "const first = actors.dispatch({actor_id}, {{ prompt: 'Fix the build' }}); \
         const second = actors.dispatch({actor_id}, {{ prompt: 'Update the docs' }}); \
         [first.status, second.title, typeof actors.run, typeof actors.wait]"
    );
    let (ScriptOutput(text), effects) =
        block_on(engine.foreground_agent_run(1, script, ScriptStop::default())).unwrap();
    assert_eq!(
        text,
        r#"["working","Update the docs","undefined","undefined"]"#
    );
    let ids: Vec<i64> = db
        .send_blocking(cohesion_db::ListActorSessionsCommand { actor_id: None })
        .unwrap()
        .into_iter()
        .map(|session| session.id)
        .collect();
    let (requests, _) = effects.into_parts();
    assert_eq!(requests.len(), 2);
    for id in ids {
        assert!(requests.contains(&ForegroundEffect::Follow(id)));
    }

    // A turn an event started follows what it dispatches too: the follow
    // keeps who started the chain.
    let dispatch = format!("actors.dispatch({actor_id}, {{ prompt: 'Lint' }}).title");
    let (ScriptOutput(text), effects) =
        block_on(engine.event_turn_run(1, DatabaseEventTrace::default(), dispatch)).unwrap();
    assert_eq!(text, r#""Lint""#);
    assert_eq!(effects.into_parts().0.len(), 1);
}

#[test]
fn agent_scripts_attach_images_that_their_runs_send_and_others_cannot() {
    let db = database();
    let actor_id = offer_actor(&db);
    let engine = ScriptEngine::new(db.read_write())
        .with_actors(Arc::new(RecordingActors { db: db.clone() }));
    let session = format!("actors.dispatch({actor_id}, {{ prompt: 'Take a screenshot' }}).id");
    let (ScriptOutput(session_id), _) =
        block_on(engine.foreground_agent_run(1, session, ScriptStop::default())).unwrap();
    let shot = |data: &str| cohesion_core::protocol::ImageUpload {
        media_type: "image/png".to_string(),
        data: data.to_string(),
    };
    let message = db
        .send_blocking(cohesion_db::AddActorSessionMessageCommand {
            session_id: session_id.parse().unwrap(),
            content: cohesion_core::model::ActorSessionMessageContent::Actor(
                cohesion_core::model::ActorMessageContent::Text {
                    text: "Here it is".to_string(),
                },
            ),
            attachments: vec![shot("c2hvdDE="), shot("c2hvdDI=")],
        })
        .unwrap();
    let data = |images: Vec<cohesion_core::protocol::ImageUpload>| -> Vec<String> {
        images.into_iter().map(|image| image.data).collect()
    };

    // A turn attaches what it fetched or made, and an actor's images by
    // their message, in the order attached.
    let script = format!(
        "images.attach({{ mediaType: 'image/gif', data: 'R0lG' }}); \
         images.attachFromActorSession({})",
        message.id
    );
    let (_, effects) =
        block_on(engine.foreground_agent_run(1, script.clone(), ScriptStop::default())).unwrap();
    assert_eq!(
        data(effects.into_parts().1),
        ["R0lG", "c2hvdDE=", "c2hvdDI="]
    );
    // So does a job.
    let (_, effects) =
        block_on(engine.event_turn_run(1, DatabaseEventTrace::default(), script.clone())).unwrap();
    assert_eq!(effects.into_parts().1.len(), 3);

    // What isn't a message's image throws, attaching nothing more.
    for refused in [
        "images.attach({ mediaType: 'image/svg+xml', data: 'PHN2Zz4=' })",
        "images.attach({ mediaType: 'image/png', data: 'not base64!' })",
        "images.attachFromActorSession(999)",
        "for (let i = 0; i < 11; i++) images.attach({ mediaType: 'image/png', data: 'AAAA' })",
    ] {
        let script = format!("try {{ {refused}; 'attached' }} catch (e) {{ 'refused' }}");
        let (ScriptOutput(text), effects) =
            block_on(engine.foreground_agent_run(1, script, ScriptStop::default())).unwrap();
        assert_eq!(text, "\"refused\"", "{refused}");
        assert!(effects.into_parts().1.len() <= 10, "{refused}");
    }

    // A script that throws attaches nothing; nor can a general script or an
    // extension's, which have no `images`.
    let thrown = "images.attach({ mediaType: 'image/png', data: 'AAAA' }); throw new Error('no')";
    assert!(
        block_on(engine.foreground_agent_run(1, thrown.to_string(), ScriptStop::default()))
            .is_err()
    );
    let ScriptOutput(text) = run(&db, "typeof images").unwrap();
    assert_eq!(text, "\"undefined\"");
}

/// A sandbox with `cohesion` that may sleep, and `seconds` to run in.
fn sleeping_sandbox(seconds: u64) -> Sandbox {
    let options = SandboxOptions {
        time_limit: std::time::Duration::from_secs(seconds),
        ..SandboxOptions::STANDARD
    };
    Sandbox::new(
        options,
        database().read_only(),
        &[Mount::Cohesion, Mount::Sleep],
        None,
    )
    .unwrap()
}

#[test]
fn sleep_pauses_then_returns() {
    let db = database();
    let started = std::time::Instant::now();
    assert_eq!(
        json(
            &db,
            "[cohesion.sleep(0.05), cohesion.sleep(0), cohesion.sleep(0.01)]"
        ),
        serde_json::json!([null, null, null])
    );
    assert!(started.elapsed() >= std::time::Duration::from_millis(60));
}

#[test]
fn sleep_refuses_what_is_not_0_to_30_seconds_or_would_pass_the_deadline() {
    let sandbox = sleeping_sandbox(60);
    let ScriptOutput(text) = sandbox
        .evaluate(
            "[30.5, 31, -1, -0.001, NaN, Infinity, -Infinity].map(s => { \
                 try { cohesion.sleep(s); return 'slept'; } \
                 catch (e) { return e instanceof RangeError; } \
             })"
            .into(),
        )
        .unwrap();
    assert_eq!(text, "[true,true,true,true,true,true,true]");

    // A pause that would outlast the script throws before it starts.
    let sandbox = sleeping_sandbox(1);
    let started = std::time::Instant::now();
    let ScriptOutput(text) = sandbox
        .evaluate("try { cohesion.sleep(5); 'slept' } catch (e) { e instanceof RangeError }".into())
        .unwrap();
    assert_eq!(text, "true");
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
}

#[test]
fn a_stop_cuts_a_sleep_short() {
    let sandbox = sleeping_sandbox(60);
    let stop = ScriptStop::default();
    let stopper = std::thread::spawn({
        let stop = stop.clone();
        move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            stop.stop();
        }
    });
    let started = std::time::Instant::now();
    let stopped = sandbox.stoppable(stop, |sandbox| {
        sandbox.evaluate("cohesion.sleep(30)".into())
    });
    stopper.join().unwrap();
    assert!(matches!(stopped, Err(ScriptError::Stopped)), "{stopped:?}");
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
}

#[test]
fn shutting_a_scope_down_cuts_its_sleep_short() {
    let db = database();
    let engine = ScriptEngine::new(db.read_write());
    block_on(
        engine.spawn_scope(
            extension("sleeper"),
            String::new(),
            Cow::Owned(vec![Mount::Cohesion, Mount::Sleep]),
            db.read_only()
                .for_extension(Arc::new("sleeper".to_string())),
        ),
    )
    .unwrap();
    let handle = engine.scope(&extension("sleeper")).unwrap();
    let started = std::time::Instant::now();
    let sleeping = std::thread::spawn(move || {
        block_on(handle.run(|sandbox| sandbox.evaluate("cohesion.sleep(30)".into())))
    });
    std::thread::sleep(std::time::Duration::from_millis(100));
    engine.shutdown_scope(&extension("sleeper"));
    let Err(ScriptError::Exception { message, .. }) = sleeping.join().unwrap() else {
        panic!("the sleep should be cut short");
    };
    assert!(message.contains("stopped"), "{message}");
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
}

#[test]
fn only_extensions_cohesion_run_and_agents_turns_may_sleep() {
    let db = database();
    let engine = ScriptEngine::new(db.read_write());
    let probe = "(() => { try { cohesion.sleep(0); return true; } catch (e) { return false; } })()";

    assert_eq!(json(&db, probe), serde_json::json!(true));
    let (ScriptOutput(text), _) =
        block_on(engine.foreground_agent_run(1, probe.to_string(), ScriptStop::default())).unwrap();
    assert_eq!(text, "true");

    let (ScriptOutput(text), _) =
        block_on(engine.event_turn_run(1, DatabaseEventTrace::default(), probe.to_string()))
            .unwrap();
    assert_eq!(text, "true");
    let cue = block_on(engine.evaluate_cue(
        1,
        DatabaseEventTrace::default(),
        format!("() => {probe}"),
        Vec::new(),
    ))
    .unwrap();
    assert_eq!(cue.payload.0, "false");
}

// A document's excerpt is its first 500 characters, with `…` only when
// the text was longer; the count is characters, not bytes.
#[test]
fn a_document_excerpt_cuts_long_contents_and_leaves_short_ones() {
    let db = database();
    create_workspace(&db, "Home");
    let found = json(
        &db,
        r#"
        const home = cohesion.workspace("Home");
        const short = home.createDocument("Short", "Eggs");
        const exact = home.createDocument("Exact", "é".repeat(500));
        const long = home.createDocument("Long", "é".repeat(501));
        [short.excerpt(), exact.excerpt() === exact.contents, long.excerpt() === "é".repeat(500) + "…"]
        "#,
    );
    assert_eq!(found, serde_json::json!(["Eggs", true, true]));
}

#[test]
fn a_script_browses_versions_and_none_can_be_removed() {
    let db = database();
    create_workspace(&db, "Home");
    let output = json(
        &db,
        "const ws = cohesion.workspaces()[0];\n\
         const created = ws.createTask({ title: 'first' }).versions()[0];\n\
         ({\n\
             change: created.change,\n\
             author: created.author,\n\
             current: created.current,\n\
             title: created.open().title,\n\
             noDelete: typeof created.delete === 'undefined'\n\
                 && typeof created.open().delete === 'undefined',\n\
         })",
    );
    assert_eq!(output["change"], "created");
    assert_eq!(output["author"], "script");
    assert_eq!(output["current"], true);
    assert_eq!(output["title"], "first");
    assert_eq!(output["noDelete"], true);
}

#[test]
fn a_script_brings_back_a_deleted_item_but_a_read_only_one_only_looks() {
    let db = database();
    let workspace_id = create_workspace(&db, "Home");
    let task_id = json(
        &db,
        "cohesion.workspaces()[0].createTask({ title: 'lost' }).id",
    );
    db.send_blocking(cohesion_db::DeleteTaskCommand {
        workspace_id,
        task_id: task_id.as_i64().unwrap(),
    })
    .unwrap();

    let sandbox = Sandbox::new(
        SandboxOptions::STANDARD,
        db.read_only(),
        &[Mount::Cohesion],
        None,
    )
    .unwrap();
    let ScriptOutput(text) = sandbox
        .evaluate(
            "const version = cohesion.workspaces()[0].deletedTasks()[0].versions()[0].open();\n\
             try { version.restore(); 'wrote' }\n\
             catch (e) { e.message.includes('read-only access') ? version.title : 'other' }"
                .to_string(),
        )
        .unwrap();
    assert_eq!(text, r#""lost""#);

    let output = json(
        &db,
        "const ws = cohesion.workspaces()[0];\n\
         const [lost] = ws.deletedTasks();\n\
         const version = lost.versions()[0];\n\
         const back = version.open().restore();\n\
         ({ deleted: lost.deleted, change: version.change, back: back.deleted,\n\
            restored: back.versions()[0].change, left: ws.deletedTasks().length })",
    );
    assert_eq!(output["deleted"], true);
    assert_eq!(output["change"], "deleted");
    assert_eq!(output["back"], false);
    assert_eq!(output["restored"], "restored");
    assert_eq!(output["left"], 0);
}

#[test]
fn a_script_renames_and_deletes_documents_but_a_read_only_one_cannot() {
    let db = database();
    create_workspace(&db, "Home");
    let result = json(
        &db,
        r#"
        const home = cohesion.workspace("Home");
        const draft = home.createDocument("Draft", "Eggs");
        const kept = home.createDocument("Kept");
        draft.rename("Groceries");
        let emptyRefused = false;
        try { draft.rename(""); } catch (e) { emptyRefused = true; }
        const renamed = [draft.title, home.document(draft.id).title];
        kept.delete();
        [renamed, emptyRefused, kept.deleted,
         home.documents().map((d) => d.title),
         home.deletedDocuments().map((d) => d.title)]
        "#,
    );
    assert_eq!(
        result,
        serde_json::json!([
            ["Groceries", "Groceries"],
            true,
            true,
            ["Groceries"],
            ["Kept"]
        ])
    );

    let sandbox = Sandbox::new(
        SandboxOptions::STANDARD,
        db.read_only(),
        &[Mount::Cohesion],
        None,
    )
    .unwrap();
    let ScriptOutput(text) = sandbox
        .evaluate(
            "const doc = cohesion.workspace('Home').documents()[0];\n\
             [() => doc.rename('Shopping'), () => doc.delete()].map(write => {\n\
                 try { write(); return 'wrote'; }\n\
                 catch (e) { return e instanceof Error && e.message.includes('read-only access'); }\n\
             })"
            .to_string(),
        )
        .unwrap();
    assert_eq!(text, "[true,true]");
}

#[test]
fn a_listed_document_reads_its_text_when_asked_and_always_the_latest() {
    let db = database();
    create_workspace(&db, "Home");
    let result = json(
        &db,
        r#"
        const home = cohesion.workspace("Home");
        home.createDocument("Long", "é".repeat(600));
        home.createDocument("Short", "Eggs");
        const [long, short] = ["Long", "Short"].map(
            (title) => home.documents().find((d) => d.title === title));
        const before = short.contents;
        home.document(short.id).append(" and milk");
        [before, short.contents, long.contents.length, short.excerpt(),
         long.excerpt().length, "contents" in JSON.parse(JSON.stringify(short))]
        "#,
    );
    assert_eq!(
        result,
        // The excerpt is as listed, like the title; only `contents` reads anew.
        serde_json::json!(["Eggs", "Eggs and milk", 600, "Eggs", 501, false])
    );
}

#[test]
fn documents_file_into_nested_folders_and_the_inbox() {
    let db = database();
    create_workspace(&db, "Home");
    let result = json(
        &db,
        r#"
        const home = cohesion.workspace("Home");
        const work = home.createDocumentFolder("Work");
        const plans = work.createFolder("Plans");
        const launch = plans.createDocument("Launch", "Go");
        const loose = home.createDocument("Loose");
        const before = [launch.folder().title, loose.folder(), plans.parent().title];
        loose.moveTo(work);
        const filed = work.documents().map((d) => d.title);
        loose.moveTo(null);
        plans.moveTo(null);
        plans.rename("Ideas");
        const tree = home.documentFolders().map((f) => [f.title, f.parentId]);
        let refused = false;
        try { work.moveTo(work); } catch (e) { refused = true; }
        const inside = work.folders().length;
        work.delete();
        [before, filed, loose.folderId, tree, refused, inside,
         home.documentFolders().map((f) => f.title),
         home.documentFolder(plans.id).documents().map((d) => d.title)]
        "#,
    );
    assert_eq!(
        result,
        serde_json::json!([
            ["Plans", null, "Work"],
            ["Loose"],
            null,
            [["Work", null], ["Ideas", null]],
            true,
            0,
            ["Ideas"],
            ["Launch"]
        ])
    );
}

#[test]
fn scripts_shape_a_tables_columns_and_read_and_write_its_rows() {
    let db = database();
    create_workspace(&db, "Home");
    let result = json(
        &db,
        r#"
        const home = cohesion.workspace("Home");
        const books = home.createDocumentFolder("Books");
        const before = books.representation;
        const pages = books.createProperty("Pages", PropertyDataType.integer);
        const status = books.createProperty("Status", "choice", { options: ["To read", "Read"] });
        const price = books.createProperty("Price", "currency");
        const total = books.createProperty("Total", "currency",
            { formula: "(row) => row.Price * 2", updateMode: FormulaUpdateMode.continuously });
        const after = books.representation;
        status.addOption("Reading", "purple");
        status.moveOption("Reading", "Read");
        status.setOptionColor("read", "teal");
        status.renameOption("To read", "Later");
        const options = status.options.map((o) => [o.name, o.color]);
        price.rename("Cost");
        price.moveBefore(pages);
        const dune = books.createDocument("Dune");
        dune.setValue("Pages", 412);
        dune.setValue(status, "Reading");
        dune.setValue("Cost", 9.5);
        const values = dune.values();
        const refused = [
            () => dune.setValue("Pages", "many"),
            () => dune.setValue("Pages", 1.5),
            () => dune.setValue("Status", "Lost"),
            () => dune.setValue("Nope", 1),
            () => dune.setValue("Total", 3),
            () => home.createDocument("Loose").setValue("Pages", 1),
            () => books.representation = "board",
        ].map((write) => { try { write(); return false; } catch (e) { return true; } });
        pages.setDataType(PropertyDataType.text);
        dune.setValue("Pages", null);
        status.deleteOption("Reading");
        const later = dune.values();
        total.setFormula(null);
        books.representation = DocumentFolderRepresentation.list;
        const listed = home.documentFolder(books.id).representation;
        const columns = books.properties().map((p) =>
            [p.name, p.dataType, p.formula, p.formulaUpdateMode]);
        status.delete();
        [before, after, options, values, refused, later, listed, columns,
         books.properties().length, home.createDocument("Loose 2").values()]
        "#,
    );
    assert_eq!(
        result,
        serde_json::json!([
            "list",
            "table",
            [["Later", null], ["Reading", "purple"], ["Read", "teal"]],
            {"Cost": 9.5, "Pages": 412, "Status": "Reading", "Total": null},
            [true, true, true, true, true, true, true],
            {"Cost": 9.5, "Pages": null, "Status": null, "Total": null},
            "list",
            [
                ["Cost", "currency", null, null],
                ["Pages", "text", null, null],
                ["Status", "choice", null, null],
                ["Total", "currency", null, null]
            ],
            3,
            {}
        ])
    );
}

#[test]
fn a_read_only_script_reads_a_table_but_cant_change_it() {
    let db = database();
    create_workspace(&db, "Home");
    run(
        &db,
        r#"
        const books = cohesion.workspace("Home").createDocumentFolder("Books");
        books.createProperty("Pages", "integer");
        books.createDocument("Dune").setValue("Pages", 412);
        "#,
    )
    .unwrap();
    let sandbox = Sandbox::new(
        SandboxOptions::STANDARD,
        db.read_only(),
        &[Mount::Cohesion],
        None,
    )
    .unwrap();
    let ScriptOutput(text) = sandbox
        .evaluate(
            "const books = cohesion.workspace('Home').documentFolders()[0];\n\
             const dune = books.documents()[0];\n\
             const pages = books.properties()[0];\n\
             [dune.values().Pages, ...[() => dune.setValue('Pages', 1), () => pages.rename('P'),\n\
              () => { books.representation = 'list'; }].map(write => {\n\
                 try { write(); return 'wrote'; }\n\
                 catch (e) { return e instanceof Error && e.message.includes('read-only access'); }\n\
             })]"
            .to_string(),
        )
        .unwrap();
    assert_eq!(text, "[412,true,true,true]");
}
