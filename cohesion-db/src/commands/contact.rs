use cohesion_core::FromRequest;
use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::{Contact, ContactMethod, ContactMethodKind, NewContactMethod};
use cohesion_core::protocol::{
    AddContactMethodRequest, CreateContactRequest, DeleteContactMethodRequest,
    DeleteContactRequest, GetContactRequest, ImageUpload, ListContactsRequest,
    SetContactPhotoRequest, UpdateContactMethodRequest, UpdateContactRequest,
};

use crate::changed::{Changed, Invalidates};
use crate::commands::image_upload::validate_photo;
use crate::commands::workspace_scope::ensure_workspace;
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, Filtered, Found, Read,
    ReadableDatabase, ValidationError, Write, WriteContainer,
};

/// Rejects an empty first or last name, and reads an empty middle name as
/// none.
fn checked_names(
    first_name: &str,
    middle_name: Option<String>,
    last_name: &str,
) -> Result<Option<String>, ValidationError> {
    if first_name.is_empty() {
        return Err(ValidationError::in_field(
            "first_name",
            "First name can’t be empty.".to_string(),
        ));
    }
    if last_name.is_empty() {
        return Err(ValidationError::in_field(
            "last_name",
            "Last name can’t be empty.".to_string(),
        ));
    }
    Ok(middle_name.filter(|name| !name.is_empty()))
}

/// Rejects a method with an empty value.
fn ensure_value(value: &str) -> Result<(), ValidationError> {
    if value.is_empty() {
        return Err(ValidationError::in_field(
            "value",
            "A contact method can’t be empty.".to_string(),
        ));
    }
    Ok(())
}

/// Method `method_id` of a live contact in workspace `workspace_id`.
fn method_in<'c>(
    db: impl ReadableDatabase<'c>,
    workspace_id: i64,
    method_id: i64,
) -> DatabaseCommandResult<ContactMethod> {
    db.select()
        .filter_eq(ContactMethod::ID, method_id)
        .filter("contacts.workspace_id = ? AND contacts.deleted = FALSE")
        .bind(workspace_id)
        .one()
        .found()
}

/// Adds a method to contact `contact_id`, after its others.
fn insert_method(
    db: WriteContainer<'_>,
    contact_id: i64,
    kind: ContactMethodKind,
    value: String,
) -> DatabaseCommandResult<ContactMethod> {
    ensure_value(&value)?;
    db.insert()
        .set(ContactMethod::CONTACT_ID, contact_id)
        .set(ContactMethod::KIND, kind)
        .set(ContactMethod::VALUE, value)
        .returning_one()
        .found()
}

/// Invalidates the contacts of workspace `workspace_id`, where `value` was
/// just written.
fn changed<T>(value: T, workspace_id: i64) -> Changed<T> {
    value.invalidating([Invalidation::Contacts(workspace_id)])
}

/// Live contacts in a workspace, by last name, then first name, ignoring
/// case.
#[derive(FromRequest)]
#[request(ListContactsRequest)]
pub struct ListContactsCommand {
    pub workspace_id: i64,
}
impl DatabaseCommand for ListContactsCommand {
    type Response = Vec<Contact>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Vec<Contact>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        Ok(ctx
            .db
            .select_live()
            .filter_eq(Contact::WORKSPACE_ID, self.workspace_id)
            .order("last_name COLLATE NOCASE, first_name COLLATE NOCASE, id")
            .all()?)
    }
}

/// A live contact in a workspace by id.
#[derive(FromRequest)]
#[request(GetContactRequest)]
pub struct GetContactCommand {
    pub workspace_id: i64,
    pub contact_id: i64,
}
impl DatabaseCommand for GetContactCommand {
    type Response = Contact;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Contact> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        ctx.db
            .select_live()
            .filter_eq(Contact::WORKSPACE_ID, self.workspace_id)
            .filter_eq(Contact::ID, self.contact_id)
            .one()
            .found()
    }
}

/// Creates a contact in a workspace, with its methods in their order.
#[derive(FromRequest)]
#[request(CreateContactRequest)]
pub struct CreateContactCommand {
    pub workspace_id: i64,
    pub first_name: String,
    pub middle_name: Option<String>,
    pub last_name: String,
    pub notes: String,
    pub methods: Vec<NewContactMethod>,
}
impl DatabaseCommand for CreateContactCommand {
    type Response = Contact;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Contact>> {
        let middle_name = checked_names(&self.first_name, self.middle_name, &self.last_name)?;
        ensure_workspace(ctx.db, self.workspace_id)?;
        let contact = ctx
            .db
            .insert::<Contact>()
            .set(Contact::WORKSPACE_ID, self.workspace_id)
            .set(Contact::FIRST_NAME, self.first_name)
            .set(Contact::MIDDLE_NAME, middle_name)
            .set(Contact::LAST_NAME, self.last_name)
            .set(Contact::NOTES, self.notes)
            .returning_one()
            .found()?;
        for method in self.methods {
            insert_method(ctx.db, contact.id, method.kind, method.value)?;
        }
        let contact = GetContactCommand {
            workspace_id: self.workspace_id,
            contact_id: contact.id,
        }
        .handle(&mut ctx.as_read())?;
        Ok(changed(contact, self.workspace_id))
    }
}

/// Replaces a live contact's names and notes.
#[derive(FromRequest)]
#[request(UpdateContactRequest)]
pub struct UpdateContactCommand {
    pub workspace_id: i64,
    pub contact_id: i64,
    pub first_name: String,
    pub middle_name: Option<String>,
    pub last_name: String,
    pub notes: String,
}
impl DatabaseCommand for UpdateContactCommand {
    type Response = Contact;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Contact>> {
        let middle_name = checked_names(&self.first_name, self.middle_name, &self.last_name)?;
        ensure_workspace(ctx.db, self.workspace_id)?;
        let contact = ctx
            .db
            .update_live()
            .set(Contact::FIRST_NAME, self.first_name)
            .set(Contact::MIDDLE_NAME, middle_name)
            .set(Contact::LAST_NAME, self.last_name)
            .set(Contact::NOTES, self.notes)
            .set_now(Contact::MODIFIED)
            .filter_eq(Contact::WORKSPACE_ID, self.workspace_id)
            .filter_eq(Contact::ID, self.contact_id)
            .returning_one()
            .found()?;
        Ok(changed(contact, self.workspace_id))
    }
}

/// Sets a live contact's photo, replacing any it had, or removes it.
#[derive(FromRequest)]
#[request(SetContactPhotoRequest)]
pub struct SetContactPhotoCommand {
    pub workspace_id: i64,
    pub contact_id: i64,
    pub photo: Option<ImageUpload>,
}
impl DatabaseCommand for SetContactPhotoCommand {
    type Response = Contact;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Contact>> {
        let photo = validate_photo(self.photo)?;
        ensure_workspace(ctx.db, self.workspace_id)?;
        let contact = ctx
            .db
            .update_live()
            .set_group(photo)
            .set_now(Contact::MODIFIED)
            .filter_eq(Contact::WORKSPACE_ID, self.workspace_id)
            .filter_eq(Contact::ID, self.contact_id)
            .returning_one()
            .found()?;
        Ok(changed(contact, self.workspace_id))
    }
}

/// Soft-deletes a contact in a workspace, keeping its methods.
#[derive(FromRequest)]
#[request(DeleteContactRequest)]
pub struct DeleteContactCommand {
    pub workspace_id: i64,
    pub contact_id: i64,
}
impl DatabaseCommand for DeleteContactCommand {
    type Response = Contact;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Contact>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        let contact = ctx
            .db
            .update_live()
            .set_true(Contact::DELETED)
            .filter_eq(Contact::WORKSPACE_ID, self.workspace_id)
            .filter_eq(Contact::ID, self.contact_id)
            .returning_one()
            .found()?;
        Ok(changed(contact, self.workspace_id))
    }
}

/// Adds an email address or phone number to a live contact, after its
/// others.
#[derive(FromRequest)]
#[request(AddContactMethodRequest)]
pub struct AddContactMethodCommand {
    pub workspace_id: i64,
    pub contact_id: i64,
    pub kind: ContactMethodKind,
    pub value: String,
}
impl DatabaseCommand for AddContactMethodCommand {
    type Response = ContactMethod;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<ContactMethod>> {
        let contact = GetContactCommand {
            workspace_id: self.workspace_id,
            contact_id: self.contact_id,
        }
        .handle(&mut ctx.as_read())?;
        let method = insert_method(ctx.db, contact.id, self.kind, self.value)?;
        Ok(changed(method, self.workspace_id))
    }
}

/// Replaces the kind and value of a method of a live contact.
#[derive(FromRequest)]
#[request(UpdateContactMethodRequest)]
pub struct UpdateContactMethodCommand {
    pub workspace_id: i64,
    pub method_id: i64,
    pub kind: ContactMethodKind,
    pub value: String,
}
impl DatabaseCommand for UpdateContactMethodCommand {
    type Response = ContactMethod;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<ContactMethod>> {
        ensure_value(&self.value)?;
        ensure_workspace(ctx.db, self.workspace_id)?;
        let method = method_in(ctx.db.read(), self.workspace_id, self.method_id)?;
        let method = ctx
            .db
            .update()
            .set(ContactMethod::KIND, self.kind)
            .set(ContactMethod::VALUE, self.value)
            .set_now(ContactMethod::MODIFIED)
            .filter_eq(ContactMethod::ID, method.id)
            .returning_one()
            .found()?;
        Ok(changed(method, self.workspace_id))
    }
}

/// Removes a method from a live contact.
#[derive(FromRequest)]
#[request(DeleteContactMethodRequest)]
pub struct DeleteContactMethodCommand {
    pub workspace_id: i64,
    pub method_id: i64,
}
impl DatabaseCommand for DeleteContactMethodCommand {
    type Response = ContactMethod;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<ContactMethod>> {
        ensure_workspace(ctx.db, self.workspace_id)?;
        let method = method_in(ctx.db.read(), self.workspace_id, self.method_id)?;
        ctx.db
            .delete::<ContactMethod>()
            .filter_eq(ContactMethod::ID, method.id)
            .execute()?;
        Ok(changed(method, self.workspace_id))
    }
}

#[cfg(test)]
mod tests;
