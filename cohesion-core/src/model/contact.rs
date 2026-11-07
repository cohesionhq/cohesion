use cohesion_macros::{Fields, Model, TextEnum, TsInterface, TsWire};
use serde::{Deserialize, Serialize};

use super::Photo;

/// What a contact method reaches; stored as text.
///
/// - `"email"`: an email address.
/// - `"phone"`: a phone number.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TextEnum, TsWire)]
#[serde(rename_all = "camelCase")]
pub enum ContactMethodKind {
    Email,
    Phone,
}

/// A person a workspace keeps: their name, the ways to reach them and notes.
#[derive(Clone, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "contacts", backup = Included)]
pub struct Contact {
    /// Unique, stable identifier.
    pub id: i64,
    /// Id of the workspace the contact belongs to.
    pub workspace_id: i64,
    /// Whether the contact has been soft-deleted.
    ///
    /// Listings skip deleted contacts, so this is normally `false`.
    pub deleted: bool,
    /// Never empty.
    pub first_name: String,
    /// `null` when the contact has none; never empty.
    pub middle_name: Option<String>,
    /// Never empty.
    pub last_name: String,
    /// Free text about the contact, as plain text / Markdown.
    pub notes: String,
    /// When the contact was added, in Unix seconds.
    pub created_at: i64,
    /// When its names, notes, methods or photo last changed, in Unix
    /// seconds.
    pub modified: i64,
    /// Its photo, if it has one.
    #[model(group)]
    #[fields(skip)]
    pub photo: Photo,
    /// The email addresses and phone numbers that reach the contact, in the
    /// order they were added.
    #[model(json, expr = Contact::METHODS_SQL)]
    #[fields(skip)]
    pub methods: Vec<ContactMethod>,
}

impl Contact {
    /// Its methods, by id, as JSON.
    pub const METHODS_SQL: &str = "(SELECT json_group_array(json_object('id', m.id, \
        'workspace_id', contacts.workspace_id, 'contact_id', m.contact_id, 'kind', m.kind, 'value', m.value, \
        'created_at', m.created_at, 'modified', m.modified)) \
        FROM (SELECT * FROM contact_methods m WHERE m.contact_id = contacts.id ORDER BY m.id) m)";
}

/// One way to reach a contact: an email address or a phone number.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Model, Fields, TsWire)]
#[model(table = "contact_methods", backup = Included)]
#[model(join = Contact, on = "contacts.id = contact_methods.contact_id")]
pub struct ContactMethod {
    /// Unique, stable identifier.
    pub id: i64,
    /// Id of the workspace its contact belongs to.
    #[model(expr = Contact::workspace_id)]
    pub workspace_id: i64,
    /// Id of the contact it reaches.
    pub contact_id: i64,
    /// Whether `value` is an email address or a phone number.
    pub kind: ContactMethodKind,
    /// The address or number, as written; never empty.
    pub value: String,
    /// When the method was added, in Unix seconds.
    pub created_at: i64,
    /// When its kind or value last changed, in Unix seconds.
    pub modified: i64,
}

/// A contact method to add with a new contact.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TsWire, TsInterface)]
pub struct NewContactMethod {
    /// Whether `value` is an email address or a phone number.
    pub kind: ContactMethodKind,
    /// The address or number, as written; may not be empty.
    pub value: String,
}
