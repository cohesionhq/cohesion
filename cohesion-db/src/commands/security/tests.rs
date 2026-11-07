use cohesion_core::model::{PairingOpener, SecurityEventKind};
use cohesion_core::{Client, LocalIdentity};

use crate::test_support::{RawSql, context, recording_context};
use crate::{
    DatabaseContext, DatabaseEventKind, DeleteClientCommand, DeleteOldSecurityEventsCommand,
    IdentifyLocalClientCommand, ListClientsCommand, ListSecurityEventPageCommand,
    RevokeInactiveClientsCommand, RevokeOtherClientsCommand, UpdateSecuritySettingsCommand,
};

/// A local client named `name`, as the Unix socket's handshake makes it.
fn local(db: &DatabaseContext, name: &str) -> Client {
    db.send_blocking(IdentifyLocalClientCommand {
        identity: LocalIdentity {
            key: name.to_string(),
            name: name.to_string(),
        },
    })
    .unwrap()
}

/// The security settings as they start, changed by `change`.
fn settings(
    change: impl FnOnce(&mut UpdateSecuritySettingsCommand),
) -> UpdateSecuritySettingsCommand {
    let mut settings = UpdateSecuritySettingsCommand {
        pairing_opened_by: PairingOpener::AnyClient,
        alert_new_client: false,
        revoke_inactive_after_days: None,
    };
    change(&mut settings);
    settings
}

/// Each recorded event's kind and client name, newest first.
fn activity(db: &DatabaseContext) -> Vec<(SecurityEventKind, Option<String>)> {
    db.send_blocking(ListSecurityEventPageCommand {
        offset: 0,
        limit: None,
    })
    .unwrap()
    .events
    .into_iter()
    .map(|event| (event.kind, event.client_name))
    .collect()
}

fn notifications(events: Vec<DatabaseEventKind>) -> Vec<String> {
    events
        .into_iter()
        .filter_map(|event| match event {
            DatabaseEventKind::NotificationSent { message, .. } => Some(message),
            _ => None,
        })
        .collect()
}

#[test]
fn a_new_client_is_recorded_and_announced_only_when_asked() {
    let (db, events) = recording_context();
    local(&db, "Desk");
    assert!(notifications(events.take()).is_empty());
    db.send_blocking(settings(|s| s.alert_new_client = true))
        .unwrap();
    local(&db, "Laptop");
    assert_eq!(
        notifications(events.take()),
        ["Laptop paired with Cohesion."]
    );
    // A known key is the same client, not a new one.
    local(&db, "Laptop");
    assert!(notifications(events.take()).is_empty());
    assert_eq!(
        activity(&db),
        [
            (SecurityEventKind::ClientPaired, Some("Laptop".to_string())),
            (SecurityEventKind::SettingsChanged, None),
            (SecurityEventKind::ClientPaired, Some("Desk".to_string())),
        ]
    );
}

#[test]
fn revoking_the_others_keeps_the_client_asking() {
    let db = context();
    let [desk, phone, tablet] = ["Desk", "Phone", "Tablet"].map(|name| local(&db, name));
    let revoked = db
        .send_blocking(RevokeOtherClientsCommand { keep: desk.id })
        .unwrap();
    assert_eq!(
        revoked.iter().map(|client| client.id).collect::<Vec<_>>(),
        [phone.id, tablet.id]
    );
    let listed = db.send_blocking(ListClientsCommand {}).unwrap();
    assert_eq!(
        listed.iter().map(|client| client.id).collect::<Vec<_>>(),
        [desk.id]
    );
}

#[test]
fn inactive_clients_are_revoked_unless_connected_or_never_dated() {
    let db = context();
    let [old, recent, connected, undated] =
        ["Old", "Recent", "Connected", "Undated"].map(|name| local(&db, name));
    db.send_blocking(RawSql(
        "UPDATE clients SET last_seen_at = unixepoch() - 40 * 86400
         WHERE client_name IN ('Old', 'Connected')",
    ))
    .unwrap();
    db.send_blocking(RawSql(
        "UPDATE clients SET last_seen_at = NULL, created_at = NULL WHERE client_name = 'Undated'",
    ))
    .unwrap();
    let revoke = || {
        db.send_blocking(RevokeInactiveClientsCommand {
            online: vec![connected.id],
        })
        .unwrap()
    };
    // No limit set: nothing goes.
    assert!(revoke().is_empty());
    db.send_blocking(settings(|s| s.revoke_inactive_after_days = Some(30)))
        .unwrap();
    let revoked = revoke();
    assert_eq!(
        revoked.iter().map(|client| client.id).collect::<Vec<_>>(),
        [old.id]
    );
    let listed: Vec<i64> = db
        .send_blocking(ListClientsCommand {})
        .unwrap()
        .iter()
        .map(|client| client.id)
        .collect();
    assert!(listed.contains(&recent.id) && listed.contains(&undated.id));
    assert_eq!(
        activity(&db)[0],
        (
            SecurityEventKind::InactiveClientRevoked,
            Some("Old".to_string())
        )
    );
}

/// The clients each `client:revoke` event names, by id and name.
fn revocations(events: Vec<DatabaseEventKind>) -> Vec<(i64, String)> {
    events
        .into_iter()
        .filter_map(|event| match event {
            DatabaseEventKind::ClientRevoked { client_id, name } => Some((client_id, name)),
            _ => None,
        })
        .collect()
}

/// Every way a client is revoked raises `client:revoke` for it, so an
/// extension can forget what it keeps for the client.
#[test]
fn every_revoke_raises_client_revoked() {
    let (db, events) = recording_context();
    let [desk, phone, tablet, old] =
        ["Desk", "Phone", "Tablet", "Old"].map(|name| local(&db, name));
    db.send_blocking(DeleteClientCommand { id: phone.id })
        .unwrap();
    assert_eq!(
        revocations(events.take()),
        [(phone.id, "Phone".to_string())]
    );

    db.send_blocking(RawSql(
        "UPDATE clients SET last_seen_at = unixepoch() - 40 * 86400 WHERE client_name = 'Old'",
    ))
    .unwrap();
    db.send_blocking(settings(|s| s.revoke_inactive_after_days = Some(30)))
        .unwrap();
    db.send_blocking(RevokeInactiveClientsCommand { online: vec![] })
        .unwrap();
    assert_eq!(revocations(events.take()), [(old.id, "Old".to_string())]);

    db.send_blocking(RevokeOtherClientsCommand { keep: desk.id })
        .unwrap();
    assert_eq!(
        revocations(events.take()),
        [(tablet.id, "Tablet".to_string())]
    );
}

#[test]
fn security_activity_is_kept_for_thirty_days() {
    let db = context();
    local(&db, "Recent");
    db.send_blocking(RawSql(
        "INSERT INTO security_events (created_at, kind, client_name)
         VALUES (unixepoch() - 31 * 86400, 'clientPaired', 'Old')",
    ))
    .unwrap();
    db.send_blocking(DeleteOldSecurityEventsCommand).unwrap();
    assert_eq!(
        activity(&db),
        [(SecurityEventKind::ClientPaired, Some("Recent".to_string()))]
    );
}
