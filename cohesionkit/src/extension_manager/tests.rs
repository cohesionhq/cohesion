use cohesion_core::model::{ExtensionPermissionKind, HookKind, Initiative};
use cohesion_core::protocol::{SetExtensionHookRequest, SetExtensionPermissionRequest};
use cohesion_db::{
    CommandError, Container, CreateWorkspaceCommand, DatabaseEventTrace, DeleteExtensionCommand,
    GetExtensionCommand, GetExtensionQuery, SandboxKey, Secrets,
};
use cohesion_js::{ScriptError, ScriptOutput, ScriptStop};

use super::{ExtensionError, ServiceCaller};
use crate::CohesionContext;

fn context() -> CohesionContext {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    CohesionContext::new(container, Secrets::new(), test_server())
}

/// An extension `identifier` at `version` whose manifest lists
/// `permissions`, followed by `body`.
fn script(identifier: &str, version: &str, permissions: &str, body: &str) -> String {
    format!(
        "function manifest() {{
           return {{ identifier: '{identifier}', name: '{identifier}', version: {version},
                    permissions: {permissions}, hooks: [] }};
         }}
         {body}"
    )
}

/// Installs `script`, replacing an installed extension with its identifier,
/// and answers its id.
async fn install(context: &CohesionContext, script: String) -> i64 {
    context.extensions.install(script, true).await.unwrap().id
}

async fn grant(context: &CohesionContext, extension_id: i64, permission: ExtensionPermissionKind) {
    context
        .extensions
        .set_permission(SetExtensionPermissionRequest {
            extension_id,
            permission,
            enabled: true,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn an_uninstalled_extension_is_not_respawned() {
    let context = context();
    let id = install(
        &context,
        script(
            "com.example.gone",
            "1",
            "[]",
            "function ping() { return 'pong'; }",
        ),
    )
    .await;
    let pong: String = context
        .extensions
        .call_function(id, None, "ping", vec![])
        .await
        .unwrap();
    assert_eq!(pong, "pong");

    context.extensions.uninstall(id).await.unwrap();
    let called: Result<String, _> = context
        .extensions
        .call_function(id, None, "ping", vec![])
        .await;
    assert!(matches!(
        called,
        Err(ExtensionError::Database(CommandError::NotFound(_)))
    ));
    assert!(
        !context
            .script_engine
            .is_scope_running(&SandboxKey::Extension("com.example.gone".to_string()))
    );
}

#[tokio::test]
async fn a_call_after_a_reinstall_runs_the_new_script() {
    let context = context();
    let which = |version: &str| {
        script(
            "com.example.which",
            version,
            "[]",
            &format!("function which() {{ return {version}; }}"),
        )
    };
    let id = install(&context, which("1")).await;
    let held = context
        .database
        .send(GetExtensionCommand {
            query: GetExtensionQuery::Id(id),
        })
        .await
        .unwrap();
    let first: i64 = context
        .extensions
        .call_function(id, None, "which", vec![])
        .await
        .unwrap();
    assert_eq!(first, 1);

    install(&context, which("2")).await;
    let second: i64 = context
        .extensions
        .call_function(held.id, None, "which", vec![])
        .await
        .unwrap();
    assert_eq!(second, 2);
}

#[tokio::test]
async fn only_cohesion_write_makes_the_scope_writable() {
    let context = context();
    context
        .database
        .send(CreateWorkspaceCommand {
            title: "W".to_string(),
            emoji: None,
        })
        .await
        .unwrap();
    let id = install(
        &context,
        script(
            "com.example.writer",
            "1",
            "['cohesion:read', 'cohesion:write']",
            "function addTask() { cohesion.workspaces()[0].createTask({ title: 't' }); return true; }",
        ),
    )
    .await;

    grant(&context, id, ExtensionPermissionKind::GlobalReadOnly).await;
    let read_only: Result<bool, _> = context
        .extensions
        .call_function(id, None, "addTask", vec![])
        .await;
    assert!(matches!(
        read_only,
        Err(ExtensionError::Script(ScriptError::Exception { message, .. }))
            if message.contains("read-only access")
    ));

    grant(&context, id, ExtensionPermissionKind::GlobalReadWrite).await;
    let written: bool = context
        .extensions
        .call_function(id, None, "addTask", vec![])
        .await
        .unwrap();
    assert!(written);
}

#[tokio::test]
async fn a_call_needing_a_grant_checks_it_when_made() {
    let context = context();
    let id = install(
        &context,
        script(
            "com.example.registry",
            "1",
            "['registry']",
            "function registryResults() { return []; }",
        ),
    )
    .await;
    let requires = Some(ExtensionPermissionKind::Registry);
    let called: Result<Vec<serde_json::Value>, _> = context
        .extensions
        .call_function(id, requires.clone(), "registryResults", vec![])
        .await;
    assert!(matches!(
        called,
        Err(ExtensionError::NotGranted(
            ExtensionPermissionKind::Registry
        ))
    ));
    grant(&context, id, ExtensionPermissionKind::Registry).await;
    let called: Vec<serde_json::Value> = context
        .extensions
        .call_function(id, requires, "registryResults", vec![])
        .await
        .unwrap();
    assert!(called.is_empty());
}

#[tokio::test]
async fn usage_is_mounted_only_once_granted() {
    let context = context();
    let id = install(
        &context,
        script(
            "com.example.meter",
            "1",
            "['usage']",
            "function seen() { return typeof usage === 'undefined' ? null : usage.total().calls; }",
        ),
    )
    .await;
    let before: Option<i64> = context
        .extensions
        .call_function(id, None, "seen", vec![])
        .await
        .unwrap();
    assert_eq!(before, None);

    grant(&context, id, ExtensionPermissionKind::Usage).await;
    let after: Option<i64> = context
        .extensions
        .call_function(id, None, "seen", vec![])
        .await
        .unwrap();
    assert_eq!(after, Some(0));
}

#[tokio::test]
async fn an_update_is_not_offered_to_another_extension_reusing_its_id() {
    let context = context();
    // Lists every identifier it's asked about at version 2.
    let registry = install(
        &context,
        script(
            "com.example.registry",
            "2",
            "['registry']",
            "function registryLookup(identifiers) {
               return identifiers.map((identifier) => ({
                 manifest: { identifier, name: identifier, version: 2, permissions: [], hooks: [] },
                 sourceURL: 'https://example.com/' + identifier + '.js',
               }));
             }",
        ),
    )
    .await;
    grant(&context, registry, ExtensionPermissionKind::Registry).await;
    let stamp = install(&context, script("com.example.stamp", "1", "[]", "")).await;
    let found = context.extensions.check_updates().await.unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].extension_id, stamp);

    // Removed behind the manager's back, so the update outlives it, and its
    // id goes to the next install.
    context
        .database
        .send(DeleteExtensionCommand {
            extension_id: stamp,
        })
        .await
        .unwrap();
    let other = install(&context, script("com.example.other", "1", "[]", "")).await;
    assert_eq!(other, stamp);
    assert!(context.extensions.updates().await.unwrap().is_empty());
}

#[tokio::test]
async fn a_hook_is_checked_when_it_is_called() {
    let context = context();
    let id = install(
        &context,
        "function manifest() {
           return { identifier: 'com.example.hooked', name: 'Hooked', version: 1,
                    permissions: [], hooks: ['task:create'] };
         }
         function onTaskCreate() { return 'ran'; }"
            .to_string(),
    )
    .await;
    let set_hook = |enabled| SetExtensionHookRequest {
        extension_id: id,
        hook: HookKind::TaskCreate,
        enabled,
    };
    let call = || {
        context
            .extensions
            .hook(id, HookKind::TaskCreate, |sandbox| {
                sandbox.call_returning::<_, String>("onTaskCreate")
            })
    };

    assert!(call().await.unwrap().is_none());
    assert!(
        !context
            .script_engine
            .is_scope_running(&SandboxKey::Extension("com.example.hooked".to_string()))
    );

    context.extensions.set_hook(set_hook(true)).await.unwrap();
    assert_eq!(call().await.unwrap().as_deref(), Some("ran"));

    context.extensions.set_hook(set_hook(false)).await.unwrap();
    assert!(call().await.unwrap().is_none());
}

/// `com.example.counter`, listing `permission`, whose `add` takes an
/// argument and whose `read` only reads; its `serviceRequest` answers the
/// name and the arguments it got.
fn counter(permission: &str) -> String {
    format!(
        "function manifest() {{
           return {{ identifier: 'com.example.counter', name: 'Counter', version: 1,
                    permissions: ['{permission}'], hooks: [],
                    service: {{ functions: {{
                      add: {{ args: '{{ by: number }}' }},
                      read: {{ readOnly: true }} }} }} }};
         }}
         function serviceRequest(name, args) {{ return name + ':' + JSON.stringify(args); }}"
    )
}

/// Runs `script` on the general-purpose pool and reads its result.
async fn run(context: &CohesionContext, script: &str) -> serde_json::Value {
    let ScriptOutput(json) = context
        .script_engine
        .general_purpose_run(script.to_string())
        .await
        .unwrap();
    serde_json::from_str(&json).unwrap()
}

/// `call`'s result, or `"refused"` when it throws.
fn refused(call: &str) -> String {
    format!("(() => {{ try {{ return {call}; }} catch (e) {{ return 'refused'; }} }})()")
}

#[tokio::test]
async fn scripts_and_extensions_granted_services_call_declared_functions() {
    let context = context();
    let counter_id = install(&context, counter("service")).await;
    grant(&context, counter_id, ExtensionPermissionKind::Service).await;
    assert_eq!(
        run(
            &context,
            "const counter = services.use('com.example.counter'); \
             [counter.add({ by: 2 }), typeof counter.reset]"
        )
        .await,
        serde_json::json!(["add:{\"by\":2}", "undefined"])
    );

    let caller_id = install(
        &context,
        script(
            "com.example.caller",
            "1",
            "['services']",
            "function read() { return services.use('com.example.counter').read(); }",
        ),
    )
    .await;
    grant(&context, caller_id, ExtensionPermissionKind::Services).await;
    let read: String = context
        .extensions
        .call_function(caller_id, None, "read", vec![])
        .await
        .unwrap();
    assert_eq!(read, "read:null");
}

#[tokio::test]
async fn an_isolated_service_is_out_of_scripts_reach() {
    let context = context();
    let id = install(&context, counter("service:isolated")).await;
    grant(&context, id, ExtensionPermissionKind::ServiceIsolated).await;
    assert_eq!(
        run(&context, &refused("services.use('com.example.counter')")).await,
        serde_json::json!("refused")
    );
    let called = context
        .extensions
        .call_service(
            id,
            ServiceCaller::Script { read_only: false },
            "read".to_string(),
            serde_json::Value::Null,
        )
        .await;
    assert!(called.is_err());
}

/// A service's `serviceRequest` is told which client calls, and that a
/// script is none.
#[tokio::test]
async fn a_service_request_names_the_client_calling() {
    let context = context();
    let id = install(
        &context,
        "function manifest() {
           return { identifier: 'com.example.caller', name: 'Caller', version: 1,
                    permissions: ['service'], hooks: [], service: { functions: { who: {} } } };
         }
         function serviceRequest(name, args, caller) { return caller; }"
            .to_string(),
    )
    .await;
    grant(&context, id, ExtensionPermissionKind::Service).await;
    let call = |caller| {
        context
            .extensions
            .call_service(id, caller, "who".to_string(), serde_json::Value::Null)
    };
    assert_eq!(
        call(ServiceCaller::Client { client_id: 7 }).await.unwrap(),
        serde_json::json!({ "clientId": 7 })
    );
    assert_eq!(
        call(ServiceCaller::Script { read_only: false })
            .await
            .unwrap(),
        serde_json::Value::Null
    );
}

/// What `services.use('com.example.counter')` offers, then what its `add`
/// and `read` answer, `"refused"` for one that throws.
const READ_ONLY_CALLS: &str = "((counter) => [Object.keys(counter), \
    (() => { try { return counter.add({ by: 1 }); } catch (e) { return 'refused'; } })(), \
    counter.read()])(services.use('com.example.counter'))";

#[tokio::test]
async fn a_turn_an_event_started_calls_only_read_only_functions() {
    let context = context();
    let id = install(&context, counter("service")).await;
    grant(&context, id, ExtensionPermissionKind::Service).await;
    let (ScriptOutput(json), _) = context
        .script_engine
        .traced_foreground_agent_run(
            1,
            READ_ONLY_CALLS.to_string(),
            ScriptStop::default(),
            DatabaseEventTrace::default().started_by(Initiative::Event),
        )
        .await
        .unwrap();
    assert_eq!(json, r#"[["add","read"],"refused","read:null"]"#);
}

#[tokio::test]
async fn a_cue_calls_only_read_only_functions() {
    let context = context();
    let id = install(&context, counter("service")).await;
    grant(&context, id, ExtensionPermissionKind::Service).await;
    let evaluated = context
        .script_engine
        .evaluate_cue(
            1,
            DatabaseEventTrace::default(),
            format!("() => {READ_ONLY_CALLS}"),
            Vec::new(),
        )
        .await
        .unwrap();
    assert_eq!(
        evaluated.payload,
        ScriptOutput(r#"[["add","read"],"refused","read:null"]"#.to_string())
    );
}

#[tokio::test]
async fn an_extension_that_only_reads_calls_only_read_only_functions() {
    let context = context();
    let id = install(&context, counter("service")).await;
    grant(&context, id, ExtensionPermissionKind::Service).await;
    let reader = |permissions| {
        script(
            "com.example.reader",
            "1",
            permissions,
            &format!("function go() {{ return {READ_ONLY_CALLS}; }}"),
        )
    };
    let reader_id = install(&context, reader("['services', 'cohesion:read']")).await;
    grant(&context, reader_id, ExtensionPermissionKind::Services).await;
    grant(&context, reader_id, ExtensionPermissionKind::GlobalReadOnly).await;
    let called: serde_json::Value = context
        .extensions
        .call_function(reader_id, None, "go", vec![])
        .await
        .unwrap();
    assert_eq!(
        called,
        serde_json::json!([["add", "read"], "refused", "read:null"])
    );

    // With `cohesion:write`, it may call `add` too.
    let writer_id = install(&context, reader("['services', 'cohesion:write']")).await;
    grant(&context, writer_id, ExtensionPermissionKind::Services).await;
    grant(
        &context,
        writer_id,
        ExtensionPermissionKind::GlobalReadWrite,
    )
    .await;
    let called: serde_json::Value = context
        .extensions
        .call_function(writer_id, None, "go", vec![])
        .await
        .unwrap();
    assert_eq!(called[1], "add:{\"by\":1}");
}

fn test_server() -> cohesion_core::ServerAddress {
    cohesion_core::ServerAddress {
        host: std::net::IpAddr::from(std::net::Ipv4Addr::LOCALHOST).into(),
        port: 3000,
        fingerprint: String::new(),
    }
}
