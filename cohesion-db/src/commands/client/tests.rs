use cohesion_core::{Client, ClientKeyKind, LocalIdentity};

use crate::test_support::context;
use crate::{Changed, Invalidates};
use crate::{
    CommandContext, CommandError, DatabaseCommand, DatabaseCommandResult, DeleteClientCommand,
    Found, IdentifyLocalClientCommand, ListClientsCommand, LookupClientByFingerprintCommand,
    RegisterClientCommand, RenameClientCommand, Write,
};

/// Inserts a client with certificate `fingerprint`, skipping the setup key.
struct InsertClient {
    fingerprint: &'static str,
}
impl DatabaseCommand for InsertClient {
    type Response = Client;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Client>> {
        let client: Client = ctx
            .db
            .insert()
            .set(Client::KEY, self.fingerprint)
            .set(Client::CLIENT_NAME, self.fingerprint)
            .returning_one()
            .found()?;
        Ok(client.invalidating_nothing())
    }
}

#[test]
fn deleted_client_is_neither_found_nor_listed() {
    let db = context();
    let [phone, laptop] = ["phone", "laptop"]
        .map(|fingerprint| db.send_blocking(InsertClient { fingerprint }).unwrap());
    db.send_blocking(DeleteClientCommand { id: phone.id })
        .unwrap();
    assert!(
        db.send_blocking(LookupClientByFingerprintCommand {
            fingerprint: "phone".to_string(),
        })
        .unwrap()
        .is_none()
    );
    let listed = db.send_blocking(ListClientsCommand {}).unwrap();
    assert_eq!(
        listed.iter().map(|client| client.id).collect::<Vec<_>>(),
        [laptop.id]
    );
}

/// A context, and the secrets whose pairing windows it registers against.
fn keyed_context() -> (crate::DatabaseContext, crate::Secrets) {
    let mut container = crate::Container::in_memory().unwrap();
    container.migrate().unwrap();
    let secrets = crate::Secrets::new();
    (
        crate::DatabaseContext::new(container, secrets.clone(), ()),
        secrets,
    )
}

/// Registers in a pairing window opened just for it.
fn register(secrets: &crate::Secrets, fingerprint: &str, name: &str) -> RegisterClientCommand {
    RegisterClientCommand {
        setup_key: secrets.begin_pairing(),
        fingerprint: fingerprint.to_string(),
        client_name: name.to_string(),
        address: "203.0.113.1".to_string(),
    }
}

fn identify(key: &str, name: &str) -> IdentifyLocalClientCommand {
    IdentifyLocalClientCommand {
        identity: LocalIdentity {
            key: key.to_string(),
            name: name.to_string(),
        },
    }
}

#[test]
fn a_local_key_names_a_client_once_and_is_refused_once_revoked() {
    let (db, _) = keyed_context();
    let studio = db.send_blocking(identify("k1", " Studio ")).unwrap();
    assert_eq!(studio.key_kind, ClientKeyKind::Local);
    assert_eq!(studio.client_name, "Studio");
    // The key, not the name, finds it again.
    assert_eq!(
        db.send_blocking(identify("k1", "Other")).unwrap().id,
        studio.id
    );
    // Names may repeat; a new key is a new client.
    assert_ne!(
        db.send_blocking(identify("k2", "Studio")).unwrap().id,
        studio.id
    );
    assert!(matches!(
        db.send_blocking(identify("k3", " ")),
        Err(CommandError::Validation(_))
    ));

    db.send_blocking(DeleteClientCommand { id: studio.id })
        .unwrap();
    assert!(matches!(
        db.send_blocking(identify("k1", "Studio")),
        Err(CommandError::Validation(_))
    ));
}

#[test]
fn a_local_key_never_finds_a_certificate() {
    let (db, secrets) = keyed_context();
    let paired = db
        .send_blocking(register(&secrets, "k1", "Mac mini"))
        .unwrap();
    assert_eq!(paired.key_kind, ClientKeyKind::Certificate);
    // The same text as a local key is hashed first, and is another kind of
    // key besides.
    let local = db.send_blocking(identify("k1", "Mac mini")).unwrap();
    assert_ne!(local.id, paired.id);
    // Registering again answers the same client, whatever the name.
    assert_eq!(
        db.send_blocking(register(&secrets, "k1", "Other"))
            .unwrap()
            .id,
        paired.id
    );
}

#[test]
fn any_live_client_is_renamed() {
    let (db, secrets) = keyed_context();
    let rename = |id, name: &str| {
        db.send_blocking(RenameClientCommand {
            id,
            client_name: name.to_string(),
        })
    };
    let paired = db
        .send_blocking(register(&secrets, "a", "Mac mini"))
        .unwrap();
    let local = db.send_blocking(identify("k", "Laptop")).unwrap();
    assert_eq!(rename(paired.id, " Studio ").unwrap().client_name, "Studio");
    assert_eq!(rename(local.id, "Desk").unwrap().client_name, "Desk");
    assert!(matches!(
        rename(local.id, " "),
        Err(CommandError::Validation(_))
    ));
    db.send_blocking(DeleteClientCommand { id: local.id })
        .unwrap();
    assert!(matches!(
        rename(local.id, "Desk"),
        Err(CommandError::NotFound(_))
    ));
}

#[test]
fn a_setup_key_registers_one_client() {
    let (db, secrets) = keyed_context();
    let key = secrets.begin_pairing();
    let command = |fingerprint: &str| RegisterClientCommand {
        setup_key: key.clone(),
        fingerprint: fingerprint.to_string(),
        client_name: "Phone".to_string(),
        address: "203.0.113.1".to_string(),
    };
    db.send_blocking(command("a")).unwrap();
    assert!(matches!(
        db.send_blocking(command("b")),
        Err(CommandError::Unauthorized)
    ));
}

#[test]
fn a_refused_registration_leaves_the_window_open() {
    let (db, secrets) = keyed_context();
    let key = secrets.begin_pairing();
    let command = |name: &str| RegisterClientCommand {
        setup_key: key.clone(),
        fingerprint: "a".to_string(),
        client_name: name.to_string(),
        address: "203.0.113.1".to_string(),
    };
    assert!(matches!(
        db.send_blocking(command(" ")),
        Err(CommandError::Validation(_))
    ));
    db.send_blocking(command("Phone")).unwrap();
}

#[test]
fn a_revoked_certificate_registers_again_with_a_new_key() {
    let (db, secrets) = keyed_context();
    let phone = db.send_blocking(register(&secrets, "a", "Phone")).unwrap();
    db.send_blocking(DeleteClientCommand { id: phone.id })
        .unwrap();
    let again = db.send_blocking(register(&secrets, "a", "Phone")).unwrap();
    assert_ne!(again.id, phone.id);
}
