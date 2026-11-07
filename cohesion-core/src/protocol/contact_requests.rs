//! Requests about contacts, the people a workspace keeps, and their contact
//! methods.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

use super::ImageUpload;
use crate::model::{ContactMethodKind, NewContactMethod};

/// Lists the live contacts in workspace `workspace_id`, by last name, then
/// first name, ignoring case.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct ListContactsRequest {
    pub workspace_id: i64,
}

/// Fetches live contact `contact_id` in workspace `workspace_id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct GetContactRequest {
    pub workspace_id: i64,
    pub contact_id: i64,
}

/// Creates a contact in workspace `workspace_id`, with `methods` in their
/// order. The first and last names may not be empty, nor may a method's
/// value; an empty middle name is none.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct CreateContactRequest {
    pub workspace_id: i64,
    pub first_name: String,
    pub middle_name: Option<String>,
    pub last_name: String,
    pub notes: String,
    pub methods: Vec<NewContactMethod>,
}

/// Replaces live contact `contact_id`'s names and notes in workspace
/// `workspace_id`, by the same rules as creating one.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct UpdateContactRequest {
    pub workspace_id: i64,
    pub contact_id: i64,
    pub first_name: String,
    pub middle_name: Option<String>,
    pub last_name: String,
    pub notes: String,
}

/// Deletes contact `contact_id` in workspace `workspace_id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DeleteContactRequest {
    pub workspace_id: i64,
    pub contact_id: i64,
}

/// Adds an email address or phone number, which may not be empty, to live
/// contact `contact_id` in workspace `workspace_id`, after its others.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct AddContactMethodRequest {
    pub workspace_id: i64,
    pub contact_id: i64,
    pub kind: ContactMethodKind,
    pub value: String,
}

/// Replaces contact method `method_id`'s kind and value, which may not be
/// empty, on a live contact in workspace `workspace_id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct UpdateContactMethodRequest {
    pub workspace_id: i64,
    pub method_id: i64,
    pub kind: ContactMethodKind,
    pub value: String,
}

/// Removes contact method `method_id` from a live contact in workspace
/// `workspace_id`.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct DeleteContactMethodRequest {
    pub workspace_id: i64,
    pub method_id: i64,
}

/// Sets live contact `contact_id`'s photo in workspace `workspace_id`,
/// replacing any it had, or removes it when `photo` is `null`. The photo
/// passes the same check as an image attached to a message.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetContactPhotoRequest {
    pub workspace_id: i64,
    pub contact_id: i64,
    pub photo: Option<ImageUpload>,
}
