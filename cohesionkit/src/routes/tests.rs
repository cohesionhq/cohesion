use std::sync::LazyLock;

use cohesion_core::model::{ExtensionPermissionKind, HookKind, PairingOpener};
use cohesion_core::protocol::{
    CallExtensionRequest, Command, CreateWorkspaceRequest, DeleteClientRequest,
    DeleteWorkspaceRequest, ErrorCode, Outcome, PreviewExtensionRequest,
    PreviewExtensionScriptRequest, RegisterExtensionRequest, ReplyData, RevokeOtherClientsRequest,
    SetExtensionHookRequest, SetExtensionPermissionRequest, UpdateSecuritySettingsRequest,
};
use cohesion_core::{Client, ClientKeyKind, LocalIdentity};
use cohesion_db::{Container, IdentifyLocalClientCommand, ListConnectorsCommand, Secrets};
use cohesion_js::ScriptArgument;

use crate::extension_manager::ExtensionFn;
use crate::{ClientRequest, CohesionContext};

/// The client the tests ask as: one on this computer, never stored.
static ME: LazyLock<Client> = LazyLock::new(|| Client {
    id: 0,
    deleted: false,
    key_kind: ClientKeyKind::Local,
    key: String::new(),
    client_name: "Tests".to_string(),
    created_at: None,
    last_seen_at: None,
    last_address: None,
});

/// `context`, asked by [`ME`].
fn asking(context: &CohesionContext) -> ClientRequest<'_> {
    ClientRequest {
        context,
        client: &ME,
    }
}

fn context() -> CohesionContext {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    CohesionContext::new(container, Secrets::new(), test_server())
}

#[tokio::test]
async fn dispatch_reaches_the_database() {
    let context = context();
    let created = Command::from(CreateWorkspaceRequest {
        title: "w".to_string(),
        emoji: None,
    })
    .dispatch(&asking(&context))
    .await;
    assert!(matches!(created, Outcome::Ok(ReplyData::Workspace(w)) if w.title == "w"));
    let listed = Command::ListWorkspaces.dispatch(&asking(&context)).await;
    assert!(matches!(listed, Outcome::Ok(ReplyData::Workspaces(ws)) if ws.len() == 1));
    let missing = Command::DeleteWorkspace(DeleteWorkspaceRequest { id: 999 })
        .dispatch(&asking(&context))
        .await;
    assert!(matches!(missing, Outcome::Error(e) if e.code == ErrorCode::NotFound));
}

/// The hash a client checks without a session is the one `Hello` sends, and
/// the one the bundle it would fetch carries.
#[tokio::test]
async fn web_bundle_hash_matches_the_bundle() {
    let context = context();
    let reply = Command::WebBundleHash.dispatch(&asking(&context)).await;
    let Outcome::Ok(ReplyData::WebBundleHash(answered)) = reply else {
        panic!("webBundleHash failed");
    };
    assert_eq!(answered.hash, crate::web_bundle_hash());
    assert_eq!(
        answered.hash.as_ref(),
        crate::web_bundle().map(|bundle| &bundle.hash)
    );
}

/// Reads the manifest of a script whose manifest lists `permissions`, and
/// which describes a service.
async fn preview(context: &CohesionContext, permissions: &str) -> Outcome {
    let script = format!(
        "function manifest() {{
           return {{ identifier: 'com.example.c', name: 'c', version: 1,
                    permissions: {permissions}, hooks: [] }};
         }}
         function serviceDescription() {{
           return {{ title: 'C', unit: 'Space' }};
         }}"
    );
    Command::from(PreviewExtensionScriptRequest { script })
        .dispatch(&asking(context))
        .await
}

#[tokio::test]
async fn a_manifest_listing_connector_must_list_oauth() {
    let context = context();
    let alone = preview(&context, "['connector']").await;
    assert!(matches!(alone, Outcome::Error(e) if e.code == ErrorCode::ValidationFailed));
    let paired = preview(&context, "['oauth', 'connector']").await;
    assert!(matches!(
        paired,
        Outcome::Ok(ReplyData::ExtensionPreview(_))
    ));
}

/// Installs `script`, replacing an installed extension with its identifier.
async fn install(context: &CohesionContext, script: String) {
    let registered = Command::from(RegisterExtensionRequest {
        script,
        allow_overwrite: true,
    })
    .dispatch(&asking(context))
    .await;
    assert!(matches!(registered, Outcome::Ok(_)));
}

/// The id of the installed extension `identifier`.
async fn extension_id(context: &CohesionContext, identifier: &str) -> i64 {
    let Outcome::Ok(ReplyData::Extensions(installed)) =
        Command::ListExtensions.dispatch(&asking(context)).await
    else {
        panic!("listing extensions failed");
    };
    installed
        .into_iter()
        .find(|e| e.script_identifier == identifier)
        .unwrap()
        .id
}

/// An `oauth` extension at `version` whose service is `title`.
fn service(version: i64, title: &str) -> String {
    format!(
        "function manifest() {{
           return {{ identifier: 'com.example.svc', name: 'Svc', version: {version},
                    permissions: ['oauth'], hooks: [] }};
         }}
         function serviceDescription() {{
           return {{ title: '{title}', unit: 'Space' }};
         }}"
    )
}

#[tokio::test]
async fn installing_stores_the_connector_and_a_higher_version_rewrites_it() {
    let context = context();
    let title = || async {
        let connectors = context
            .database
            .send(ListConnectorsCommand {})
            .await
            .unwrap();
        let [connector] = &connectors[..] else {
            panic!("expected one connector");
        };
        connector.title.clone()
    };
    install(&context, service(2, "Second")).await;
    assert_eq!(title().await, "Second");
    install(&context, service(1, "First")).await;
    assert_eq!(title().await, "Second");
    install(&context, service(3, "Third")).await;
    assert_eq!(title().await, "Third");
}

/// The test registry's `announced()`: how many `extension:update` hooks it saw.
struct Announced;
impl ExtensionFn for Announced {
    const NAME: &'static str = "announced";
    const REQUIRES: ExtensionPermissionKind = ExtensionPermissionKind::Registry;
    type Output = i64;
    fn arguments(self) -> Vec<ScriptArgument> {
        Vec::new()
    }
}

fn stamp(version: &str) -> String {
    format!(
        "function manifest() {{
           return {{ identifier: 'com.example.stamp', name: 'Stamp', version: {version},
                    permissions: [], hooks: [] }};
         }}"
    )
}

#[tokio::test]
async fn an_update_is_a_registry_entry_at_a_higher_version_until_installed() {
    let context = context();
    // Lists every identifier it's asked about: itself at its own version, the
    // rest at 2. Counts the updates announced to it.
    install(
        &context,
        "function manifest() {
           return { identifier: 'com.example.registry', name: 'Registry', version: 1,
                    permissions: ['registry'], hooks: ['extension:update'] };
         }
         function registryLookup(identifiers) {
           return identifiers.map((identifier) => ({
             manifest: { identifier, name: identifier, permissions: [], hooks: [],
                         version: identifier === 'com.example.registry' ? 1 : 2 },
             sourceURL: 'https://example.com/' + identifier + '.js',
           }));
         }
         function onExtensionUpdate(update) {
           storage.set(update.identifier, (storage.get(update.identifier) ?? 0) + 1);
         }
         function announced() { return storage.get('com.example.stamp'); }"
            .to_string(),
    )
    .await;
    let registry = extension_id(&context, "com.example.registry").await;
    let enabled = Command::from(SetExtensionPermissionRequest {
        extension_id: registry,
        permission: ExtensionPermissionKind::Registry,
        enabled: true,
    })
    .dispatch(&asking(&context))
    .await;
    assert!(matches!(enabled, Outcome::Ok(_)));
    let hooked = Command::from(SetExtensionHookRequest {
        extension_id: registry,
        hook: HookKind::ExtensionUpdate,
        enabled: true,
    })
    .dispatch(&asking(&context))
    .await;
    assert!(matches!(hooked, Outcome::Ok(_)));
    install(&context, stamp("1")).await;
    let stamp_id = extension_id(&context, "com.example.stamp").await;

    let checked = Command::CheckExtensionUpdates
        .dispatch(&asking(&context))
        .await;
    let Outcome::Ok(ReplyData::ExtensionUpdates(updates)) = checked else {
        panic!("checking for updates failed");
    };
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].extension_id, stamp_id);
    assert_eq!(updates[0].registry_extension_id, registry);
    assert_eq!(updates[0].result.manifest.version, 2);

    // A second check finds the same version, already announced.
    let rechecked = Command::CheckExtensionUpdates
        .dispatch(&asking(&context))
        .await;
    assert!(matches!(rechecked, Outcome::Ok(ReplyData::ExtensionUpdates(u)) if u.len() == 1));
    context.work.idle().await;
    let announced = context.extensions.call(registry, Announced).await.unwrap();
    assert_eq!(announced, 1);

    install(&context, stamp("2")).await;
    let listed = Command::ListExtensionUpdates
        .dispatch(&asking(&context))
        .await;
    assert!(matches!(listed, Outcome::Ok(ReplyData::ExtensionUpdates(u)) if u.is_empty()));

    // A registry behind the installed version offers no downgrade.
    install(&context, stamp("3")).await;
    let behind = Command::CheckExtensionUpdates
        .dispatch(&asking(&context))
        .await;
    assert!(matches!(behind, Outcome::Ok(ReplyData::ExtensionUpdates(u)) if u.is_empty()));
}

/// The identifier `service_extension` installs.
const SERVICE: &str = "com.example.service";

/// Installs `com.example.service`, with `body` after its manifest, granting it
/// `service` when `granted`, and answers its identifier.
async fn service_extension(context: &CohesionContext, body: &str, granted: bool) -> &'static str {
    install(
        context,
        format!(
            "function manifest() {{
               return {{ identifier: 'com.example.service', name: 'Service', version: 1,
                        permissions: ['service'], hooks: [] }};
             }}
             {body}"
        ),
    )
    .await;
    let id = extension_id(context, SERVICE).await;
    if granted {
        let enabled = Command::from(SetExtensionPermissionRequest {
            extension_id: id,
            permission: ExtensionPermissionKind::Service,
            enabled: true,
        })
        .dispatch(&asking(context))
        .await;
        assert!(matches!(enabled, Outcome::Ok(_)));
    }
    SERVICE
}

async fn call(
    context: &CohesionContext,
    identifier: &str,
    name: &str,
    args: serde_json::Value,
) -> Outcome {
    Command::from(CallExtensionRequest {
        identifier: identifier.to_string(),
        name: name.to_string(),
        args,
    })
    .dispatch(&asking(context))
    .await
}

const ECHO: &str = "function serviceRequest(name, args) {
                      if (name === 'echo') return { name, doubled: args.n * 2 };
                      if (name === 'nothing') return undefined;
                      throw new Error('unknown request `' + name + '`');
                    }";

#[tokio::test]
async fn a_service_request_answers_what_the_script_returns() {
    let context = context();
    let extension = service_extension(&context, ECHO, true).await;
    let echoed = call(&context, extension, "echo", serde_json::json!({ "n": 21 })).await;
    let Outcome::Ok(ReplyData::ExtensionCallResult(echoed)) = echoed else {
        panic!("the call failed");
    };
    assert_eq!(
        echoed.result,
        serde_json::json!({ "name": "echo", "doubled": 42 })
    );
    let nothing = call(&context, extension, "nothing", serde_json::Value::Null).await;
    assert!(matches!(
        nothing,
        Outcome::Ok(ReplyData::ExtensionCallResult(r)) if r.result.is_null()
    ));
}

#[tokio::test]
async fn a_service_request_to_an_unknown_identifier_is_not_found() {
    let context = context();
    service_extension(&context, ECHO, true).await;
    let called = call(
        &context,
        "com.example.missing",
        "echo",
        serde_json::Value::Null,
    )
    .await;
    assert!(matches!(called, Outcome::Error(e) if e.code == ErrorCode::NotFound));
}

#[tokio::test]
async fn a_service_request_needs_the_service_grant() {
    let context = context();
    let extension = service_extension(&context, ECHO, false).await;
    let called = call(&context, extension, "echo", serde_json::json!({ "n": 1 })).await;
    assert!(matches!(
        called,
        Outcome::Error(e) if e.code == ErrorCode::ValidationFailed && e.message.contains("`service`")
    ));
}

#[tokio::test]
async fn a_service_request_that_throws_shows_the_scripts_message() {
    let context = context();
    let extension = service_extension(&context, ECHO, true).await;
    let called = call(&context, extension, "foo", serde_json::Value::Null).await;
    assert!(matches!(
        called,
        Outcome::Error(e) if e.code == ErrorCode::ValidationFailed
            && e.message.contains("unknown request `foo`")
    ));
}

#[tokio::test]
async fn a_service_request_to_an_extension_without_servicerequest_says_so() {
    let context = context();
    let extension = service_extension(&context, "", true).await;
    let called = call(&context, extension, "echo", serde_json::Value::Null).await;
    assert!(matches!(
        called,
        Outcome::Error(e) if e.code == ErrorCode::ValidationFailed
            && e.message.contains("serviceRequest")
    ));
}

/// `com.example.revoked`, a service that keeps each client `client:revoke`
/// names and answers them at `revoked`.
const REVOKED: &str = "function manifest() {
       return { identifier: 'com.example.revoked', name: 'Revoked', version: 1,
                permissions: ['service'], hooks: ['client:revoke'] };
     }
     function onClientRevoke(client) {
       storage.set('revoked', [...(storage.get('revoked') ?? []), client]);
     }
     function serviceRequest(name, args, caller) {
       return { caller, revoked: storage.get('revoked') ?? [] };
     }";

/// Revoking a client, one at a time or every other, calls `onClientRevoke`
/// with it, by the id a service request from it carries as `clientId`.
#[tokio::test]
async fn revoking_a_client_calls_client_revoke_hooks() {
    let context = context();
    install(&context, REVOKED.to_string()).await;
    let id = extension_id(&context, "com.example.revoked").await;
    let granted = Command::from(SetExtensionPermissionRequest {
        extension_id: id,
        permission: ExtensionPermissionKind::Service,
        enabled: true,
    })
    .dispatch(&asking(&context))
    .await;
    assert!(matches!(granted, Outcome::Ok(_)));
    let hooked = Command::from(SetExtensionHookRequest {
        extension_id: id,
        hook: HookKind::ClientRevoke,
        enabled: true,
    })
    .dispatch(&asking(&context))
    .await;
    assert!(matches!(hooked, Outcome::Ok(_)));
    let mut clients = Vec::new();
    for name in ["Desk", "Phone", "Tablet"] {
        let identity = LocalIdentity {
            key: name.to_string(),
            name: name.to_string(),
        };
        let client = context
            .database
            .send(IdentifyLocalClientCommand { identity })
            .await
            .unwrap();
        clients.push(client);
    }
    let [desk, phone, tablet] = <[Client; 3]>::try_from(clients).ok().unwrap();
    let as_desk = ClientRequest {
        context: &context,
        client: &desk,
    };

    let revoked = Command::from(DeleteClientRequest { id: phone.id })
        .dispatch(&as_desk)
        .await;
    assert!(matches!(revoked, Outcome::Ok(_)));
    let others = Command::from(RevokeOtherClientsRequest)
        .dispatch(&as_desk)
        .await;
    assert!(matches!(others, Outcome::Ok(_)));
    context.work.idle().await;

    let called = Command::from(CallExtensionRequest {
        identifier: "com.example.revoked".to_string(),
        name: "revoked".to_string(),
        args: serde_json::Value::Null,
    })
    .dispatch(&as_desk)
    .await;
    let Outcome::Ok(ReplyData::ExtensionCallResult(called)) = called else {
        panic!("the call failed");
    };
    assert_eq!(
        called.result,
        serde_json::json!({
            "caller": { "clientId": desk.id },
            "revoked": [
                { "id": phone.id, "name": "Phone" },
                { "id": tablet.id, "name": "Tablet" },
            ],
        })
    );
}

/// A manifest for `com.example.counter` listing `permissions` and
/// declaring `service`, with a `serviceRequest` that echoes the name.
fn counter(permissions: &str, service: &str) -> String {
    format!(
        "function manifest() {{
           return {{ identifier: 'com.example.counter', name: 'Counter', version: 1,
                    permissions: {permissions}, hooks: [], service: {service} }};
         }}
         function serviceRequest(name) {{ return name; }}"
    )
}

const COUNTER: &str = "{ functions: {
                         read: { returns: 'number', readOnly: true },
                         add: { args: `{\n  by: number\n}` } } }";

#[tokio::test]
async fn a_service_block_needs_exactly_one_service_permission() {
    let context = context();
    for permissions in ["[]", "['network']", "['service', 'service:isolated']"] {
        let script = counter(permissions, COUNTER);
        let previewed = Command::from(PreviewExtensionScriptRequest { script })
            .dispatch(&asking(&context))
            .await;
        assert!(
            matches!(previewed, Outcome::Error(ref e) if e.code == ErrorCode::ValidationFailed),
            "{permissions} was accepted"
        );
    }
    for permissions in ["['service']", "['service:isolated']"] {
        let script = counter(permissions, COUNTER);
        let previewed = Command::from(PreviewExtensionScriptRequest { script })
            .dispatch(&asking(&context))
            .await;
        let preview = match previewed {
            Outcome::Ok(ReplyData::ExtensionPreview(preview)) => preview,
            Outcome::Error(e) => panic!("{permissions} was refused: {}", e.message),
            _ => panic!(),
        };
        let service = preview.manifest.service.unwrap();
        assert!(service.functions["read"].read_only.unwrap());
        // Spread over lines in the script, kept to one in the `.d.ts`.
        assert!(
            !service.functions["add"]
                .args
                .as_ref()
                .unwrap()
                .contains('\n')
        );
    }
}

#[tokio::test]
async fn service_names_must_be_identifiers() {
    let context = context();
    for service in [
        "{ functions: { delete: {} } }",
        "{ functions: { 'two words': {} } }",
        "{ functions: { '': {} } }",
        "{ functions: { '1st': {} } }",
        "{ functions: { constructor: {} } }",
        "{ functions: { 'a-b': {} } }",
    ] {
        let script = counter("['service']", service);
        let previewed = Command::from(PreviewExtensionScriptRequest { script })
            .dispatch(&asking(&context))
            .await;
        assert!(
            matches!(previewed, Outcome::Error(ref e) if e.code == ErrorCode::ValidationFailed),
            "{service} was accepted"
        );
    }
}

/// Installs the counter with `permission` and grants it.
async fn granted_counter(context: &CohesionContext, permission: ExtensionPermissionKind) {
    let listed = format!("['{}']", permission.as_str());
    install(context, counter(&listed, COUNTER)).await;
    let id = extension_id(context, "com.example.counter").await;
    let enabled = Command::from(SetExtensionPermissionRequest {
        extension_id: id,
        permission,
        enabled: true,
    })
    .dispatch(&asking(context))
    .await;
    assert!(matches!(enabled, Outcome::Ok(_)));
}

#[tokio::test]
async fn a_declared_service_answers_only_its_functions() {
    for permission in [
        ExtensionPermissionKind::Service,
        ExtensionPermissionKind::ServiceIsolated,
    ] {
        let context = context();
        granted_counter(&context, permission).await;
        let read = call(
            &context,
            "com.example.counter",
            "read",
            serde_json::Value::Null,
        )
        .await;
        assert!(matches!(
            read,
            Outcome::Ok(ReplyData::ExtensionCallResult(r)) if r.result == "read"
        ));
        let other = call(
            &context,
            "com.example.counter",
            "reset",
            serde_json::Value::Null,
        )
        .await;
        assert!(matches!(other, Outcome::Error(e) if e.code == ErrorCode::ValidationFailed));
    }
}

fn test_server() -> cohesion_core::ServerAddress {
    cohesion_core::ServerAddress {
        host: std::net::IpAddr::from(std::net::Ipv4Addr::LOCALHOST).into(),
        port: 3000,
        fingerprint: String::new(),
    }
}

/// With pairing kept to this computer, only a client on it opens pairing,
/// and asking whether it may says the same.
#[tokio::test]
async fn pairing_kept_to_this_computer_refuses_other_clients() {
    let context = context();
    let settings = Command::from(UpdateSecuritySettingsRequest {
        pairing_opened_by: PairingOpener::LocalClients,
        alert_new_client: false,
        revoke_inactive_after_days: None,
    })
    .dispatch(&asking(&context))
    .await;
    assert!(matches!(settings, Outcome::Ok(_)));
    // The pairing is recorded as the client's, which must be stored.
    let desk = context
        .database
        .send(IdentifyLocalClientCommand {
            identity: LocalIdentity {
                key: "desk".to_string(),
                name: "Desk".to_string(),
            },
        })
        .await
        .unwrap();
    let phone = Client {
        key_kind: ClientKeyKind::Certificate,
        ..desk.clone()
    };
    let context = &context;
    let asked_by = |command: Command, client| async move {
        command.dispatch(&ClientRequest { context, client }).await
    };
    let away = asked_by(Command::BeginPairing, &phone).await;
    assert!(matches!(away, Outcome::Error(e) if e.code == ErrorCode::BadRequest));
    let here = asked_by(Command::BeginPairing, &desk).await;
    assert!(matches!(here, Outcome::Ok(ReplyData::PairingCode(_))));
    // Asking whether it may answers as opening would.
    let access = asked_by(Command::GetPairingAccess, &phone).await;
    assert!(matches!(access, Outcome::Ok(ReplyData::PairingAccess(a)) if a.refusal.is_some()));
    let access = asked_by(Command::GetPairingAccess, &desk).await;
    assert!(matches!(access, Outcome::Ok(ReplyData::PairingAccess(a)) if a.refusal.is_none()));
}

/// Serves `body` once, as a `200`, on a loopback port, and answers its URL.
fn serve_once(body: Vec<u8>) -> String {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0; 4096];
        let _ = stream.read(&mut request);
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(head.as_bytes());
        let _ = stream.write_all(&body);
    });
    format!("http://{address}/extension.js")
}

async fn preview_url(context: &CohesionContext, source_url: String) -> Outcome {
    Command::from(PreviewExtensionRequest { source_url })
        .dispatch(&asking(context))
        .await
}

#[tokio::test]
async fn previewing_a_url_reads_the_downloaded_script() {
    let context = context();
    let script = counter("[]", "undefined");
    let url = serve_once(script.clone().into_bytes());
    let Outcome::Ok(ReplyData::ExtensionPreview(preview)) = preview_url(&context, url).await else {
        panic!("the download wasn't read");
    };
    assert_eq!(preview.script, script);
    assert_eq!(preview.manifest.identifier, "com.example.counter");
}

#[tokio::test]
async fn previewing_a_url_refuses_a_script_too_large() {
    let context = context();
    let url = serve_once(vec![b' '; super::extension_routes::MAX_SCRIPT_BYTES + 1]);
    let previewed = preview_url(&context, url).await;
    assert!(matches!(previewed, Outcome::Error(e) if e.code == ErrorCode::UpstreamError));
}

#[tokio::test]
async fn previewing_a_url_takes_only_http() {
    let context = context();
    for url in ["file:///etc/passwd", "ftp://example.com/a.js", "not a url"] {
        let previewed = preview_url(&context, url.to_string()).await;
        assert!(
            matches!(previewed, Outcome::Error(ref e) if e.code == ErrorCode::ValidationFailed),
            "{url} was accepted"
        );
    }
}
