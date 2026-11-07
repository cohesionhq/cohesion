//! `accounts` against an in-memory database, driven by real extension scripts.

use std::sync::Arc;

use cohesion_core::model::{
    AccountHost, ConnectorDefinition, Credential, ExtensionManifest, ServiceDescription,
};
use cohesion_db::{
    Container, CreateExtensionCommand, DatabaseContext, GuardedDatabaseContext,
    ListConnectorAccountRowsCommand, Secrets, UpsertConnectorAccountCommand,
};

use crate::globals::Mount;
use crate::sandbox::{Sandbox, SandboxOptions};
use crate::{ScriptError, ScriptOutput};

/// Far enough ahead never to need a refresh.
const FAR_FUTURE: i64 = 4_102_444_800;

fn database() -> DatabaseContext {
    let mut container = Container::in_memory().unwrap();
    container.migrate().unwrap();
    DatabaseContext::new(container, Secrets::new(), ())
}

/// Installs extension `identifier` with one account, `account`, whose
/// credential is `token` and expires at `expires_at`.
fn install(db: &DatabaseContext, identifier: &str, account: &str, expires_at: i64) {
    let extension = db
        .send_blocking(CreateExtensionCommand {
            script: String::new(),
            manifest: ExtensionManifest {
                identifier: identifier.to_string(),
                name: identifier.to_string(),
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
    db.send_blocking(UpsertConnectorAccountCommand {
        extension_id: extension.id,
        connector: ConnectorDefinition {
            identifier: "com.example".to_string(),
            icon: None,
            version: 1,
            description: ServiceDescription {
                sign_in_hint: None,
                title: "Service".to_string(),
                unit: "Workspace".to_string(),
                kinds: vec![],
                metadata_keys: vec![],
            },
        },
        identifier: account.to_string(),
        description: "Example Co.".to_string(),
        credential: Credential {
            authorization: "token".to_string(),
            refresh: Some("r".to_string()),
            expires_at: Some(expires_at),
        },
    })
    .unwrap();
}

/// Extension `identifier`'s sandbox with `accounts` mounted, running `script`.
fn sandbox(guard: GuardedDatabaseContext, identifier: &str, script: &str) -> Sandbox {
    let sandbox = Sandbox::new(
        SandboxOptions::STANDARD,
        guard.for_extension(Arc::new(identifier.to_string())),
        &[Mount::Accounts],
        None,
    )
    .unwrap();
    sandbox.evaluate(script.to_string()).unwrap();
    sandbox
}

/// What is stored for `account`, read without storing a refresh: one the
/// manager asks for is handed the stored credential and fails.
fn stored_credential(db: &DatabaseContext, account: &str) -> Credential {
    let row = db
        .send_blocking(ListConnectorAccountRowsCommand {})
        .unwrap()
        .into_iter()
        .find(|row| {
            matches!(&row.host, AccountHost::Extension { identifier, .. } if identifier == account)
        })
        .unwrap();
    let mut stored = None;
    let current = db
        .credentials()
        .fresh(row.host.extension_id().unwrap(), row.id, |credential| {
            stored = Some(credential.clone());
            Err(())
        });
    current.ok().or(stored).unwrap()
}

#[test]
fn accounts_lists_only_the_extensions_own() {
    let db = database();
    install(&db, "com.example.a", "acct-a", FAR_FUTURE);
    install(&db, "com.example.b", "acct-b", FAR_FUTURE);
    let sandbox = sandbox(db.read_only(), "com.example.a", "0");

    let ScriptOutput(text) = sandbox
        .evaluate(
            "accounts.all().map(a => [a instanceof Account, a.identifier, 'extensionId' in a])"
                .to_string(),
        )
        .unwrap();
    assert_eq!(text, r#"[[true,"acct-a",false]]"#);
}

#[test]
fn a_current_credential_is_returned_without_a_refresh() {
    let db = database();
    install(&db, "com.example.a", "acct-a", FAR_FUTURE);
    let sandbox = sandbox(
        db.read_only(),
        "com.example.a",
        "function completeRefresh() { throw new Error('refreshed'); }",
    );

    let ScriptOutput(text) = sandbox
        .evaluate("accounts.all()[0].credential().authorization".to_string())
        .unwrap();
    assert_eq!(text, r#""token""#);
    assert_eq!(stored_credential(&db, "acct-a").authorization, "token");
}

#[test]
fn an_expiring_credential_is_refreshed_and_stored_even_read_only() {
    let db = database();
    install(&db, "com.example.a", "acct-a", 1);
    let sandbox = sandbox(
        db.read_only(),
        "com.example.a",
        "function completeRefresh(c) {
           return { authorization: c.authorization + '+', refresh: c.refresh,
                    expiresAt: 4102444800 };
         }",
    );

    let ScriptOutput(text) = sandbox
        .evaluate(
            "const account = accounts.all()[0];
             [account.credential().authorization, account.credential().authorization]"
                .to_string(),
        )
        .unwrap();
    // Refreshed once: the second call sees the stored credential is current.
    assert_eq!(text, r#"["token+","token+"]"#);
    let stored = stored_credential(&db, "acct-a");
    assert_eq!(stored.authorization, "token+");
    assert_eq!(stored.expires_at, Some(FAR_FUTURE));
}

#[test]
fn an_expiring_credential_without_complete_refresh_throws() {
    let db = database();
    install(&db, "com.example.a", "acct-a", 1);
    let sandbox = sandbox(db.read_write(), "com.example.a", "0");

    let refreshed = sandbox.evaluate("accounts.all()[0].credential()".to_string());
    assert!(
        matches!(&refreshed, Err(ScriptError::Exception { message, .. }) if message.contains("completeRefresh")),
        "{refreshed:?}"
    );
    assert_eq!(stored_credential(&db, "acct-a").authorization, "token");
}

#[test]
fn credential_inside_complete_refresh_throws_instead_of_waiting_on_itself() {
    let db = database();
    install(&db, "com.example.a", "acct-a", 1);
    let sandbox = sandbox(
        db.read_only(),
        "com.example.a",
        "function completeRefresh() { return accounts.all()[0].credential(); }",
    );

    let refreshed = sandbox.evaluate("accounts.all()[0].credential()".to_string());
    assert!(
        matches!(&refreshed, Err(ScriptError::Exception { message, .. }) if message.contains("while `completeRefresh` runs")),
        "{refreshed:?}"
    );
    assert_eq!(stored_credential(&db, "acct-a").authorization, "token");
    // The worker is free to refresh again.
    sandbox
        .evaluate(
            "completeRefresh = c => ({ authorization: 'fresh', expiresAt: 4102444800 }); 0"
                .to_string(),
        )
        .unwrap();
    let ScriptOutput(text) = sandbox
        .evaluate("accounts.all()[0].credential().authorization".to_string())
        .unwrap();
    assert_eq!(text, r#""fresh""#);
}

#[test]
fn an_account_creates_an_endpoint_that_signs_in_with_it() {
    let db = database();
    install(&db, "com.example.a", "acct-a", FAR_FUTURE);
    let create = "accounts.all()[0].createEndpoint({
        identifier: 'openai', baseURL: 'https://api.openai.com/v1',
        dialect: Dialect.statelessResponses, name: 'ChatGPT',
    })";

    // Endpoints are settings: without `intelligence` the write is refused.
    let refused = Sandbox::new(
        SandboxOptions::STANDARD,
        db.read_only()
            .for_extension(Arc::new("com.example.a".to_string())),
        &[Mount::Accounts, Mount::Intelligence],
        None,
    )
    .unwrap()
    .evaluate(create.to_string());
    assert!(
        matches!(&refused, Err(ScriptError::Exception { message, .. }) if message.contains("intelligence")),
        "{refused:?}"
    );

    let sandbox = Sandbox::new(
        SandboxOptions::STANDARD,
        db.read_write()
            .for_extension(Arc::new("com.example.a".to_string())),
        &[Mount::Accounts, Mount::Intelligence],
        None,
    )
    .unwrap();
    let ScriptOutput(text) = sandbox
        .evaluate(format!(
            "const endpoint = {create};
             const listed = intelligence.endpoint('ChatGPT');
             // A plan pays for its models, so they can say they cost nothing.
             const model = endpoint.addModel({{ identifier: 'gpt', prices: {{ input: 0, output: 0 }} }});
             [endpoint.accountId === accounts.all()[0].id, listed.dialect, listed.baseUrl,
              listed.providerIdentifier, listed.clientId, model.inputTokenCost]"
        ))
        .unwrap();
    assert_eq!(
        text,
        r#"[true,"stateless-responses","https://api.openai.com/v1","openai",null,0]"#
    );
}
