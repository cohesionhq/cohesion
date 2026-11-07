use cohesion_core::Named;
use cohesion_core::model::{Contact, ContactMethod, ContactMethodKind, NewContactMethod, Photo};
use cohesion_core::protocol::ImageUpload;

use crate::test_support::context;
use crate::{
    AddContactMethodCommand, CommandError, CreateContactCommand, CreateWorkspaceCommand,
    DatabaseContext, DeleteContactCommand, DeleteContactMethodCommand, GetContactCommand,
    ListContactsCommand, SetContactPhotoCommand, UpdateContactCommand, UpdateContactMethodCommand,
};

fn workspace(db: &DatabaseContext) -> i64 {
    db.send_blocking(CreateWorkspaceCommand {
        title: "w".to_string(),
        emoji: None,
    })
    .unwrap()
    .id
}

fn create(db: &DatabaseContext, ws: i64, first: &str, last: &str) -> Contact {
    db.send_blocking(CreateContactCommand {
        workspace_id: ws,
        first_name: first.to_string(),
        middle_name: Some(String::new()),
        last_name: last.to_string(),
        notes: String::new(),
        methods: vec![
            NewContactMethod {
                kind: ContactMethodKind::Email,
                value: format!("{first}@example.com"),
            },
            NewContactMethod {
                kind: ContactMethodKind::Phone,
                value: "+44 20 7946 0000".to_string(),
            },
        ],
    })
    .unwrap()
}

fn get(db: &DatabaseContext, ws: i64, id: i64) -> Result<Contact, CommandError> {
    db.send_blocking(GetContactCommand {
        workspace_id: ws,
        contact_id: id,
    })
}

fn listed(db: &DatabaseContext, ws: i64) -> Vec<String> {
    db.send_blocking(ListContactsCommand { workspace_id: ws })
        .unwrap()
        .into_iter()
        .map(|contact| contact.first_name)
        .collect()
}

fn values(contact: &Contact) -> Vec<&str> {
    contact.methods.iter().map(|m| m.value.as_str()).collect()
}

#[test]
fn contacts_list_by_name_with_their_methods_in_order() {
    let db = context();
    let ws = workspace(&db);
    let ada = create(&db, ws, "Ada", "lovelace");
    create(&db, ws, "Charles", "Babbage");
    // An empty middle name is none.
    assert_eq!(ada.middle_name, None);
    assert_eq!(values(&ada), ["Ada@example.com", "+44 20 7946 0000"]);
    assert_eq!(listed(&db, ws), ["Charles", "Ada"]);

    let updated = db
        .send_blocking(UpdateContactCommand {
            workspace_id: ws,
            contact_id: ada.id,
            first_name: "Ada".to_string(),
            middle_name: Some("King".to_string()),
            last_name: "Lovelace".to_string(),
            notes: "Wrote the first program.".to_string(),
        })
        .unwrap();
    assert_eq!(updated.middle_name.as_deref(), Some("King"));
    // Updating a contact leaves its methods alone.
    assert_eq!(values(&updated), values(&ada));
}

#[test]
fn methods_are_added_changed_and_removed_on_their_contact() {
    let db = context();
    let ws = workspace(&db);
    let ada = create(&db, ws, "Ada", "Lovelace");
    let added = db
        .send_blocking(AddContactMethodCommand {
            workspace_id: ws,
            contact_id: ada.id,
            kind: ContactMethodKind::Email,
            value: "ada@work.example".to_string(),
        })
        .unwrap();
    let changed: ContactMethod = db
        .send_blocking(UpdateContactMethodCommand {
            workspace_id: ws,
            method_id: ada.methods[0].id,
            kind: ContactMethodKind::Phone,
            value: "555".to_string(),
        })
        .unwrap();
    assert_eq!(changed.kind, ContactMethodKind::Phone);
    db.send_blocking(DeleteContactMethodCommand {
        workspace_id: ws,
        method_id: ada.methods[1].id,
    })
    .unwrap();
    let read = get(&db, ws, ada.id).unwrap();
    assert_eq!(values(&read), ["555", "ada@work.example"]);
    assert_eq!(read.methods[1].id, added.id);

    // A method is reached only through a live contact in its workspace, and
    // an empty value is refused.
    let other = workspace(&db);
    assert!(matches!(
        db.send_blocking(DeleteContactMethodCommand {
            workspace_id: other,
            method_id: added.id,
        }),
        Err(CommandError::NotFound(ContactMethod::NOUN))
    ));
    assert!(matches!(
        db.send_blocking(UpdateContactMethodCommand {
            workspace_id: ws,
            method_id: added.id,
            kind: ContactMethodKind::Email,
            value: String::new(),
        }),
        Err(CommandError::Validation(_))
    ));
    db.send_blocking(DeleteContactCommand {
        workspace_id: ws,
        contact_id: ada.id,
    })
    .unwrap();
    assert!(matches!(
        db.send_blocking(AddContactMethodCommand {
            workspace_id: ws,
            contact_id: ada.id,
            kind: ContactMethodKind::Email,
            value: "x@y".to_string(),
        }),
        Err(CommandError::NotFound(Contact::NOUN))
    ));
}

fn set_photo(
    db: &DatabaseContext,
    ws: i64,
    id: i64,
    photo: Option<(&str, &str)>,
) -> Result<Contact, CommandError> {
    db.send_blocking(SetContactPhotoCommand {
        workspace_id: ws,
        contact_id: id,
        photo: photo.map(|(media_type, data)| ImageUpload {
            media_type: media_type.to_string(),
            data: data.to_string(),
        }),
    })
}

#[test]
fn a_contacts_photo_is_set_replaced_and_removed() {
    let db = context();
    let ws = workspace(&db);
    let ada = create(&db, ws, "Ada", "Lovelace");
    assert_eq!(ada.photo, Photo::None);
    set_photo(&db, ws, ada.id, Some(("image/png", "AAAA"))).unwrap();
    let replaced = set_photo(&db, ws, ada.id, Some(("image/jpeg", "BBBB"))).unwrap();
    let image = Photo::Image {
        photo_media_type: "image/jpeg".to_string(),
        photo_data: "BBBB".to_string(),
    };
    assert_eq!(replaced.photo, image);
    // Listing carries it, and setting it leaves the rest of the contact alone.
    let listed = db
        .send_blocking(ListContactsCommand { workspace_id: ws })
        .unwrap();
    assert_eq!(listed[0].photo, image);
    assert_eq!(values(&listed[0]), values(&ada));

    // An image a message couldn't carry is refused, and the photo kept.
    for refused in [("image/tiff", "AAAA"), ("image/png", "not base64")] {
        assert!(matches!(
            set_photo(&db, ws, ada.id, Some(refused)),
            Err(CommandError::Validation(_))
        ));
    }
    assert_eq!(get(&db, ws, ada.id).unwrap().photo, image);

    let removed = set_photo(&db, ws, ada.id, None).unwrap();
    assert_eq!(removed.photo, Photo::None);
}

#[test]
fn contacts_refuse_empty_names_and_stay_in_their_workspace() {
    let db = context();
    let ws = workspace(&db);
    let other = workspace(&db);
    let ada = create(&db, ws, "Ada", "Lovelace");
    for (first, last) in [("Ada", ""), ("", "Lovelace")] {
        let refused = db.send_blocking(CreateContactCommand {
            workspace_id: ws,
            first_name: first.to_string(),
            middle_name: None,
            last_name: last.to_string(),
            notes: String::new(),
            methods: Vec::new(),
        });
        assert!(matches!(refused, Err(CommandError::Validation(_))));
    }
    // A method with an empty value refuses the whole contact.
    let refused = db.send_blocking(CreateContactCommand {
        workspace_id: ws,
        first_name: "Grace".to_string(),
        middle_name: None,
        last_name: "Hopper".to_string(),
        notes: String::new(),
        methods: vec![NewContactMethod {
            kind: ContactMethodKind::Email,
            value: String::new(),
        }],
    });
    assert!(matches!(refused, Err(CommandError::Validation(_))));
    assert_eq!(listed(&db, ws), ["Ada"]);
    assert!(matches!(
        get(&db, other, ada.id),
        Err(CommandError::NotFound(Contact::NOUN))
    ));
}

#[test]
fn a_deleted_contact_is_kept_but_hidden() {
    let db = context();
    let ws = workspace(&db);
    let ada = create(&db, ws, "Ada", "Lovelace");
    let deleted = db
        .send_blocking(DeleteContactCommand {
            workspace_id: ws,
            contact_id: ada.id,
        })
        .unwrap();
    assert!(deleted.deleted);
    assert_eq!(values(&deleted), values(&ada));
    assert!(listed(&db, ws).is_empty());
    assert!(matches!(
        get(&db, ws, ada.id),
        Err(CommandError::NotFound(Contact::NOUN))
    ));
}
