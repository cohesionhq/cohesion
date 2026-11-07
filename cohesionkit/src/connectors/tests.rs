use cohesion_core::model::ExtensionPermissionKind;
use cohesion_core::protocol::{
    BeginConnectorAuthenticationRequest, CompleteConnectorAuthenticationRequest,
    DeleteConnectorAccountRequest, ImportResourceRequest, ImportableResource,
    ListConnectorOrganizationUnitsRequest, Local, RedactedEndpointBackend,
    RedactedIntelligenceEndpoint, Route, SearchConnectorResourcesRequest, ServerError,
    SetExtensionPermissionRequest,
};
use cohesion_db::{
    Container, CreateWorkspaceCommand, GetAccountConnectorCommand, ListAccountResourcesCommand,
    ListConnectorAccountRowsCommand, ListIntelligenceEndpointsCommand, Secrets,
};
use serde_json::{Value, json};

use crate::CohesionContext;
use crate::connectors::refresh_resources;

fn context() -> CohesionContext {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    CohesionContext::new(container, Secrets::new(), test_server())
}

/// A `connector` extension serving notes: it lists two, Groceries and
/// Recipes, changed at 1 and holding "old" until `bump()` changes them, and
/// counts each call it answers, as "list", "fetch" or "units", until
/// `taken()` hands them over. `listedAs(date)` makes its listing report
/// `date` (0 or null: no date) for every note, while a fetch still reports 1.
const NOTES_EXTENSION: &str = "function manifest() {
       return { identifier: 'com.example.notes', name: 'Notes', version: 1,
                permissions: ['oauth', 'connector'], hooks: [] };
     }
     function serviceDescription() {
       return { title: 'Notes', unit: 'Account',
                kinds: [{ identifier: 'note', title: 'Note', plural: 'Notes',
                          contents: 'Markdown of the note' }] };
     }
     function authenticationURL() { return 'https://example.com/auth'; }
     function completeAuthentication(code) { return { authorization: code }; }
     function getAccountDescription(credential) {
       return { identifier: credential.authorization, description: 'Account ' + credential.authorization };
     }
     let changed = 1, contents = 'old', calls = [], listed;
     function listedAs0() { listed = 0; return null; }
     function listedAsNull() { listed = null; return null; }
     function bump() { changed = 2; contents = 'new'; return null; }
     function taken() { const t = calls; calls = []; return t; }
     function getOrganizationUnits() {
       calls.push('units');
       return [{ id: 'icloud', display: 'iCloud' }];
     }
     function listResources(filters) {
       calls.push('list');
       const search = (filters.search ?? '').toLowerCase();
       return ['Groceries', 'Recipes']
         .filter((title) => title.toLowerCase().includes(search))
         .map((title) => ({ identifier: title.toLowerCase(),
                            lastUpdated: listed === undefined ? changed : listed, title,
                            kindIdentifier: 'note', metadata: {} }));
     }
     function getResource(identifier) {
       calls.push('fetch');
       return { identifier, lastUpdated: changed, title: identifier, contents,
                contentKind: 'markdown', kindIdentifier: 'note', metadata: {} };
     }";

/// Installs [`NOTES_EXTENSION`] with `oauth` and `connector` on and signs
/// in: the extension's id and the account's.
async fn notes(ctx: &CohesionContext) -> (i64, i64) {
    let (extension_id, account_id) = signed_in(ctx, NOTES_EXTENSION, "a").await;
    ctx.extensions
        .set_permission(SetExtensionPermissionRequest {
            extension_id,
            permission: ExtensionPermissionKind::Connector,
            enabled: true,
        })
        .await
        .unwrap();
    (extension_id, account_id)
}

/// Calls `function` of extension `extension_id`, without arguments.
async fn call(ctx: &CohesionContext, extension_id: i64, function: &'static str) -> Value {
    ctx.extensions
        .scoped(extension_id, None, move |sandbox| {
            sandbox.call_returning(function)
        })
        .await
        .unwrap()
}

async fn workspace(ctx: &CohesionContext) -> i64 {
    ctx.database
        .send(CreateWorkspaceCommand {
            title: "home".to_string(),
            emoji: None,
        })
        .await
        .unwrap()
        .id
}

async fn search(
    ctx: &CohesionContext,
    account_id: i64,
    workspace_id: i64,
    search: Option<&str>,
) -> Vec<ImportableResource> {
    let connector = ctx
        .database
        .send(GetAccountConnectorCommand { account_id })
        .await
        .unwrap();
    Route::<Local, _>::route(
        ctx,
        SearchConnectorResourcesRequest {
            account_id,
            workspace_id,
            kind_id: connector.kinds[0].id,
            organization_unit_id: None,
            search: search.map(str::to_string),
        },
    )
    .await
    .unwrap()
}

async fn import(
    ctx: &CohesionContext,
    account_id: i64,
    workspace_id: i64,
    identifier: &str,
) -> Result<cohesion_core::model::Resource, ServerError> {
    Route::<Local, _>::route(
        ctx,
        ImportResourceRequest {
            account_id,
            workspace_id,
            identifier: identifier.to_string(),
        },
    )
    .await
}

#[tokio::test]
async fn an_accounts_items_are_listed_live_and_imported() {
    let ctx = context();
    let (_, account_id) = notes(&ctx).await;
    let workspace_id = workspace(&ctx).await;

    let listed = search(&ctx, account_id, workspace_id, Some("GROC")).await;
    assert_eq!(listed.len(), 1);
    let resource = import(&ctx, account_id, workspace_id, "groceries")
        .await
        .unwrap();
    assert_eq!(resource.contents, "old");
    assert_eq!(resource.kind, "com.example.notes.note");
    let listed = search(&ctx, account_id, workspace_id, None).await;
    assert!(listed[0].imported && !listed[1].imported);
    let units =
        Route::<Local, _>::route(&ctx, ListConnectorOrganizationUnitsRequest { account_id })
            .await
            .unwrap();
    assert_eq!(
        units
            .iter()
            .map(|unit| unit.display.as_str())
            .collect::<Vec<_>>(),
        ["iCloud"]
    );
}

#[tokio::test]
async fn a_refresh_lists_first_and_fetches_only_what_changed() {
    let ctx = context();
    let workspace_id = workspace(&ctx).await;
    let (extension_id, account_id) = notes(&ctx).await;
    import(&ctx, account_id, workspace_id, "groceries")
        .await
        .unwrap();
    call(&ctx, extension_id, "taken").await;

    // Listed as unchanged: nothing is fetched.
    let summary = refresh_resources(&ctx, Some(workspace_id)).await;
    assert_eq!(
        (summary.checked, summary.updated, summary.failed),
        (1, 0, 0)
    );
    assert_eq!(call(&ctx, extension_id, "taken").await, json!(["list"]));

    // Listed as changed: fetched and rewritten.
    call(&ctx, extension_id, "bump").await;
    let summary = refresh_resources(&ctx, Some(workspace_id)).await;
    assert_eq!(
        (summary.checked, summary.updated, summary.failed),
        (1, 1, 0)
    );
    assert_eq!(
        call(&ctx, extension_id, "taken").await,
        json!(["list", "fetch"])
    );
    let resources = ctx
        .database
        .send(ListAccountResourcesCommand {
            account_id,
            workspace_id: None,
        })
        .await
        .unwrap();
    assert_eq!(resources[0].contents, "new");
}

#[tokio::test]
async fn a_preview_without_a_date_is_listed_as_unknown() {
    let ctx = context();
    let (extension_id, account_id) = notes(&ctx).await;
    let workspace_id = workspace(&ctx).await;
    let dates = async |ctx: &CohesionContext| {
        search(ctx, account_id, workspace_id, None)
            .await
            .iter()
            .map(|item| item.last_updated)
            .collect::<Vec<_>>()
    };

    assert_eq!(dates(&ctx).await, [Some(1), Some(1)]);
    // An extension written before `null` existed reports 0 for "unknown".
    call(&ctx, extension_id, "listedAs0").await;
    assert_eq!(dates(&ctx).await, [None, None]);
    call(&ctx, extension_id, "listedAsNull").await;
    assert_eq!(dates(&ctx).await, [None, None]);
}

#[tokio::test]
async fn a_refresh_fetches_what_the_listing_gives_no_date() {
    for unknown in ["listedAs0", "listedAsNull"] {
        let ctx = context();
        let workspace_id = workspace(&ctx).await;
        let (extension_id, account_id) = notes(&ctx).await;
        import(&ctx, account_id, workspace_id, "groceries")
            .await
            .unwrap();
        call(&ctx, extension_id, unknown).await;
        call(&ctx, extension_id, "taken").await;

        // Nothing to compare, so each refresh fetches, though nothing changed.
        for _ in 0..2 {
            let summary = refresh_resources(&ctx, Some(workspace_id)).await;
            assert_eq!(
                (summary.checked, summary.updated, summary.failed),
                (1, 0, 0),
                "{unknown}"
            );
            assert_eq!(
                call(&ctx, extension_id, "taken").await,
                json!(["list", "fetch"]),
                "{unknown}"
            );
        }
    }
}

/// An `oauth` extension whose sign-in page carries the PKCE challenge it is
/// passed, and whose account describes itself as the code, verifier and
/// challenge its completion saw.
const PKCE_EXTENSION: &str = "function manifest() {
       return { identifier: 'com.example.pkce', name: 'PKCE', version: 1,
                permissions: ['oauth'], hooks: [] };
     }
     function serviceDescription() { return { title: 'PKCE', unit: 'Account' }; }
     let challenge;
     function authenticationURL(pkce) {
       challenge = pkce.challenge;
       return `https://example.com/auth?code_challenge=${pkce.challenge}&code_challenge_method=${pkce.method}`;
     }
     function completeAuthentication(code, verifier) {
       return { authorization: JSON.stringify([code, verifier ?? null, challenge]) };
     }
     function getAccountDescription(credential) {
       return { identifier: 'one', description: credential.authorization };
     }";

#[tokio::test]
async fn a_sign_in_passes_its_pkce_verifier_to_its_completion_once() {
    let ctx = context();
    let extension_id = ctx
        .extensions
        .install(PKCE_EXTENSION.to_string(), true)
        .await
        .unwrap()
        .id;
    ctx.extensions
        .set_permission(SetExtensionPermissionRequest {
            extension_id,
            permission: ExtensionPermissionKind::OAuth,
            enabled: true,
        })
        .await
        .unwrap();
    let complete = || {
        Route::<Local, _>::route(
            &ctx,
            CompleteConnectorAuthenticationRequest {
                extension_id,
                code: "c".to_string(),
            },
        )
    };

    let url = Route::<Local, _>::route(&ctx, BeginConnectorAuthenticationRequest { extension_id })
        .await
        .unwrap()
        .url;
    let challenge = url
        .strip_prefix("https://example.com/auth?code_challenge=")
        .and_then(|rest| rest.strip_suffix("&code_challenge_method=S256"))
        .unwrap();
    let seen: (String, Option<String>, String) =
        serde_json::from_str(&complete().await.unwrap().description).unwrap();
    assert_eq!(seen.0, "c");
    assert_eq!(seen.1.as_ref().map(String::len), Some(43));
    assert_eq!(seen.2, challenge);

    // Taken by the first completion: a second, with no sign-in begun, gets none.
    let again: (String, Option<String>, String) =
        serde_json::from_str(&complete().await.unwrap().description).unwrap();
    assert_eq!(again.1, None);
}

/// An `oauth` extension that sets up an endpoint signing in with each
/// account it is told was added.
const ENDPOINT_EXTENSION: &str = "function manifest() {
       return { identifier: 'com.example.plan', name: 'Plan', version: 1,
                permissions: ['oauth', 'intelligence'], hooks: [] };
     }
     function serviceDescription() { return { title: 'Plan', unit: 'Account' }; }
     function authenticationURL() { return 'https://example.com/auth'; }
     function completeAuthentication(code) { return { authorization: code }; }
     function getAccountDescription(credential) {
       return { identifier: credential.authorization, description: 'Account ' + credential.authorization };
     }
     function accountAdded(added) {
       accounts.all().find((a) => a.identifier === added.identifier).createEndpoint({
         identifier: 'openai', baseURL: 'https://api.openai.com/v1',
         dialect: Dialect.statelessResponses, name: added.description,
       });
     }";

// `accountAdded` runs once the account is stored, so it can set up an
// endpoint on it, and that endpoint goes when the account is removed.
#[tokio::test]
async fn an_added_account_sets_up_an_endpoint_that_goes_with_it() {
    let ctx = context();
    let extension_id = ctx
        .extensions
        .install(ENDPOINT_EXTENSION.to_string(), true)
        .await
        .unwrap()
        .id;
    for permission in [
        ExtensionPermissionKind::OAuth,
        ExtensionPermissionKind::Intelligence,
    ] {
        ctx.extensions
            .set_permission(SetExtensionPermissionRequest {
                extension_id,
                permission,
                enabled: true,
            })
            .await
            .unwrap();
    }
    let account = Route::<Local, _>::route(
        &ctx,
        CompleteConnectorAuthenticationRequest {
            extension_id,
            code: "a".to_string(),
        },
    )
    .await
    .unwrap();
    let endpoints = || ctx.database.send(ListIntelligenceEndpointsCommand {});

    let created = endpoints().await.unwrap();
    assert!(matches!(
        created.as_slice(),
        [RedactedIntelligenceEndpoint {
            name,
            backend: RedactedEndpointBackend::Account { account_id, .. },
            ..
        }] if name == "Account a" && *account_id == account.id
    ));

    Route::<Local, _>::route(
        &ctx,
        DeleteConnectorAccountRequest {
            account_id: account.id,
        },
    )
    .await
    .unwrap();
    assert!(endpoints().await.unwrap().is_empty());
}

/// An `oauth` extension that remembers the accounts its `accountRemoved`
/// was told of, and answers them from `removedAccounts()`.
const REMOVAL_EXTENSION: &str = "function manifest() {
       return { identifier: 'com.example.removal', name: 'Removal', version: 1,
                permissions: ['oauth'], hooks: [] };
     }
     function serviceDescription() { return { title: 'Removal', unit: 'Account' }; }
     function authenticationURL() { return 'https://example.com/auth'; }
     function completeAuthentication(code) { return { authorization: code }; }
     function getAccountDescription(credential) {
       return { identifier: credential.authorization, description: 'Account ' + credential.authorization };
     }
     const removed = [];
     function accountRemoved(account) { removed.push(account); }
     function removedAccounts() { return removed; }";

/// Installs `script` with `oauth` on and signs in with `code`: the
/// extension's id and the account's.
async fn signed_in(ctx: &CohesionContext, script: &str, code: &str) -> (i64, i64) {
    let extension_id = ctx
        .extensions
        .install(script.to_string(), true)
        .await
        .unwrap()
        .id;
    ctx.extensions
        .set_permission(SetExtensionPermissionRequest {
            extension_id,
            permission: ExtensionPermissionKind::OAuth,
            enabled: true,
        })
        .await
        .unwrap();
    let account = Route::<Local, _>::route(
        ctx,
        CompleteConnectorAuthenticationRequest {
            extension_id,
            code: code.to_string(),
        },
    )
    .await
    .unwrap();
    (extension_id, account.id)
}

#[tokio::test]
async fn removing_an_account_tells_its_extension_which_one() {
    let ctx = context();
    let (extension_id, account_id) = signed_in(&ctx, REMOVAL_EXTENSION, "a").await;
    Route::<Local, _>::route(&ctx, DeleteConnectorAccountRequest { account_id })
        .await
        .unwrap();

    let removed: serde_json::Value = ctx
        .extensions
        .scoped(extension_id, None, |sandbox| {
            sandbox.call_returning("removedAccounts")
        })
        .await
        .unwrap();
    assert_eq!(
        removed,
        serde_json::json!([{ "identifier": "a", "description": "Account a" }])
    );
}

#[tokio::test]
async fn removing_an_account_needs_no_account_removed() {
    let ctx = context();
    let (_, account_id) = signed_in(&ctx, PKCE_EXTENSION, "a").await;
    Route::<Local, _>::route(&ctx, DeleteConnectorAccountRequest { account_id })
        .await
        .unwrap();
    let accounts = ctx
        .database
        .send(ListConnectorAccountRowsCommand {})
        .await
        .unwrap();
    assert!(accounts.is_empty());
}

fn test_server() -> cohesion_core::ServerAddress {
    cohesion_core::ServerAddress {
        host: std::net::IpAddr::from(std::net::Ipv4Addr::LOCALHOST).into(),
        port: 3000,
        fingerprint: String::new(),
    }
}

/// Agent scripts block on connector calls from their own threads, outside
/// the runtime; the deadline's timer must not need a runtime on that thread.
#[tokio::test(flavor = "multi_thread")]
async fn a_script_thread_blocks_on_a_call_until_its_deadline() {
    use std::time::{Duration, Instant};

    let runtime = tokio::runtime::Handle::current();
    let outcome = std::thread::spawn(move || {
        let answered = crate::connectors::agent_connectors::block_until(
            &runtime,
            Instant::now() + Duration::from_secs(60),
            async { Ok(7) },
        );
        let late = crate::connectors::agent_connectors::block_until(
            &runtime,
            Instant::now(),
            std::future::pending::<Result<i32, ServerError>>(),
        );
        (answered, late)
    })
    .join()
    .unwrap();
    assert_eq!(outcome.0.unwrap(), 7);
    assert!(outcome.1.is_err());
}
