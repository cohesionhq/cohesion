use cohesion_core::model::{
    ConnectorDefinition, Credential, ExtensionManifest, ServiceDescription,
};

use crate::test_support::context;
use crate::{
    CommandError, CreateExtensionCommand, CredentialError, DatabaseContext,
    GetConnectorCredentialCommand, ListConnectorAccountsCommand, UpsertConnectorAccountCommand,
};

/// Far enough ahead never to need a refresh.
const FAR_FUTURE: i64 = 4_102_444_800;

/// Installs extension `identifier` with one account whose credential is
/// `token` and expires at `expires_at`; returns the extension's and the
/// account's ids.
fn install(db: &DatabaseContext, identifier: &str, expires_at: i64) -> (i64, i64) {
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
    let account = db
        .send_blocking(UpsertConnectorAccountCommand {
            extension_id: extension.id,
            connector: ConnectorDefinition {
                identifier: "com.example.c".to_string(),
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
            identifier: "acct".to_string(),
            description: "Example Co.".to_string(),
            credential: Credential {
                authorization: "token".to_string(),
                refresh: Some("r".to_string()),
                expires_at: Some(expires_at),
            },
        })
        .unwrap();
    (extension.id, account.id)
}

/// Signs the account `install` made in again, as `completeAuthentication`
/// does; returns its id.
fn install_again(db: &DatabaseContext, extension_id: i64) -> i64 {
    db.send_blocking(UpsertConnectorAccountCommand {
        extension_id,
        connector: ConnectorDefinition {
            identifier: "com.example.c".to_string(),
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
        identifier: "acct".to_string(),
        description: "Example Co.".to_string(),
        credential: Credential {
            authorization: "signed-in".to_string(),
            refresh: None,
            expires_at: Some(FAR_FUTURE),
        },
    })
    .unwrap()
    .id
}

fn stored(db: &DatabaseContext, extension_id: i64, account_id: i64) -> Credential {
    db.send_blocking(GetConnectorCredentialCommand {
        extension_id,
        account_id,
    })
    .unwrap()
}

/// A refresh that must not run.
fn never(_: &Credential) -> Result<Credential, ()> {
    panic!("refreshed a credential that didn't need it")
}

#[test]
fn a_current_credential_is_handed_out_without_a_refresh() {
    let db = context();
    let (extension_id, account_id) = install(&db, "com.example.a", FAR_FUTURE);

    let credential = db
        .credentials()
        .fresh(extension_id, account_id, never)
        .unwrap();
    assert_eq!(credential.authorization, "token");
}

#[test]
fn an_expiring_credential_is_refreshed_once_and_stored() {
    let db = context();
    let (extension_id, account_id) = install(&db, "com.example.a", 1);

    let credential = db
        .credentials()
        .fresh(extension_id, account_id, |old| {
            Ok::<_, ()>(Credential {
                authorization: format!("{}+", old.authorization),
                refresh: None,
                expires_at: Some(FAR_FUTURE),
            })
        })
        .unwrap();
    // The refresh token outlives a refresh that doesn't replace it.
    assert_eq!(credential.authorization, "token+");
    assert_eq!(credential.refresh.as_deref(), Some("r"));
    let again = db
        .credentials()
        .fresh(extension_id, account_id, never)
        .unwrap();
    assert_eq!(again.authorization, "token+");
}

#[test]
fn a_failed_refresh_stores_nothing() {
    let db = context();
    let (extension_id, account_id) = install(&db, "com.example.a", 1);

    let failed = db
        .credentials()
        .fresh(extension_id, account_id, |_| Err("revoked"));
    assert!(matches!(failed, Err(CredentialError::Refresh("revoked"))));
    let stored = stored(&db, extension_id, account_id);
    assert_eq!(stored.authorization, "token");
    assert_eq!(stored.expires_at, Some(1));
}

/// When the only account's refresh last failed.
fn failed_at(db: &DatabaseContext) -> Option<i64> {
    let accounts = db.send_blocking(ListConnectorAccountsCommand {}).unwrap();
    accounts[0].credential_failed_at
}

fn renewed(_: &Credential) -> Result<Credential, ()> {
    Ok(Credential {
        authorization: "new".to_string(),
        refresh: None,
        expires_at: Some(FAR_FUTURE),
    })
}

#[test]
fn a_failed_refresh_flags_the_account_until_a_refresh_succeeds() {
    let db = context();
    let (extension_id, account_id) = install(&db, "com.example.a", 1);
    assert_eq!(failed_at(&db), None);

    let _ = db
        .credentials()
        .fresh(extension_id, account_id, |_| Err("revoked"));
    let first = failed_at(&db).expect("flagged");
    // Failing again keeps the first time.
    let _ = db
        .credentials()
        .fresh(extension_id, account_id, |_| Err("revoked"));
    assert_eq!(failed_at(&db), Some(first));

    db.credentials()
        .fresh(extension_id, account_id, renewed)
        .unwrap();
    assert_eq!(failed_at(&db), None);
}

#[test]
fn signing_in_again_clears_the_flag() {
    let db = context();
    let (extension_id, account_id) = install(&db, "com.example.a", 1);
    let _ = db
        .credentials()
        .fresh(extension_id, account_id, |_| Err("revoked"));
    assert!(failed_at(&db).is_some());

    // The same remote account, signed in again.
    assert_eq!(install_again(&db, extension_id), account_id);
    assert_eq!(failed_at(&db), None);
}

#[test]
fn another_extensions_account_is_not_found() {
    let db = context();
    let (_, account_id) = install(&db, "com.example.a", 1);
    let (other_id, _) = install(&db, "com.example.b", FAR_FUTURE);

    let credential = db.credentials().fresh(other_id, account_id, never);
    assert!(
        matches!(
            credential,
            Err(CredentialError::Command(CommandError::NotFound(_)))
        ),
        "{credential:?}"
    );
}

#[test]
fn asking_for_a_credential_while_refreshing_fails_instead_of_waiting() {
    let db = context();
    let (extension_id, account_id) = install(&db, "com.example.a", 1);
    let (other_extension, other_account) = install(&db, "com.example.b", FAR_FUTURE);

    let refreshed = db.credentials().fresh(extension_id, account_id, |_| {
        let same = db.credentials().fresh(extension_id, account_id, never);
        let other = db
            .credentials()
            .fresh(other_extension, other_account, never);
        assert!(matches!(same, Err(CredentialError::Reentrant)));
        assert!(matches!(other, Err(CredentialError::Reentrant)));
        Err(())
    });
    assert!(matches!(refreshed, Err(CredentialError::Refresh(()))));
    // The thread is no longer refreshing.
    db.credentials()
        .fresh(other_extension, other_account, never)
        .unwrap();
}
