//! `Contact` and `ContactMethod` in scripts.

use cohesion_core::model::{Contact, ContactMethod, ContactMethodKind, Photo};
use cohesion_core::protocol::ImageUpload;
use cohesion_db::{
    AddContactMethodCommand, DeleteContactCommand, DeleteContactMethodCommand, GetContactCommand,
    SetContactPhotoCommand, UpdateContactCommand, UpdateContactMethodCommand,
};
use cohesion_macros::{JsOptions, js_entity};
use rquickjs::{Ctx, Object, Value};

use crate::conversion::FromJsField;

/// A person a workspace keeps: their name, the email addresses and phone
/// numbers that reach them, and notes.
///
/// Get them from `Workspace.contacts()`, `Workspace.contact(id)` or
/// `Workspace.createContact()`.
///
/// Assigning `firstName`, `middleName`, `lastName`, `notes` or `photo` saves
/// immediately, and `modified` updates. An empty first or last name throws;
/// an empty middle name clears it. Not available in read-only contexts.
#[js_entity(Contact)]
impl JsContact {
    /// Assigning saves immediately.
    #[qjs(set, rename = "firstName")]
    pub fn set_first_name<'js>(&mut self, ctx: Ctx<'js>, name: String) -> rquickjs::Result<()> {
        self.save(&ctx, |contact| contact.first_name = name)
    }

    /// Assigning saves immediately; `null` or `""` clears it.
    #[qjs(set, rename = "middleName")]
    pub fn set_middle_name<'js>(
        &mut self,
        ctx: Ctx<'js>,
        name: Option<String>,
    ) -> rquickjs::Result<()> {
        self.save(&ctx, |contact| contact.middle_name = name)
    }

    /// Assigning saves immediately.
    #[qjs(set, rename = "lastName")]
    pub fn set_last_name<'js>(&mut self, ctx: Ctx<'js>, name: String) -> rquickjs::Result<()> {
        self.save(&ctx, |contact| contact.last_name = name)
    }

    /// Assigning saves immediately.
    #[qjs(set, rename = "notes")]
    pub fn set_notes<'js>(&mut self, ctx: Ctx<'js>, notes: String) -> rquickjs::Result<()> {
        self.save(&ctx, |contact| contact.notes = notes)
    }

    /// The contact's photo, or `null` when it has none.
    #[qjs(get)]
    #[ts(returns = "ContactPhotoImage | null")]
    pub fn photo<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        let Photo::Image {
            photo_media_type,
            photo_data,
        } = &self.inner.photo
        else {
            return Ok(Value::new_null(ctx));
        };
        let photo = Object::new(ctx)?;
        photo.set("mediaType", photo_media_type.as_str())?;
        photo.set("data", photo_data.as_str())?;
        Ok(photo.into_value())
    }

    /// Assigning saves immediately; `null` removes it. Throws if it isn't a
    /// PNG, JPEG, GIF or WebP image, isn't base64, or is over 5 MB.
    #[qjs(set, rename = "photo")]
    pub fn set_photo<'js>(
        &mut self,
        ctx: Ctx<'js>,
        photo: Option<ContactPhotoImage>,
    ) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            SetContactPhotoCommand {
                workspace_id: self.inner.workspace_id,
                contact_id: self.inner.id,
                photo: photo.map(|photo| ImageUpload {
                    media_type: photo.media_type,
                    data: photo.data,
                }),
            },
        )?;
        Ok(())
    }

    /// The email addresses and phone numbers that reach the contact, in the
    /// order they were added.
    ///
    /// As they were when this object was read, or when `addMethod()` last
    /// ran on it; read the contact again after changing a method.
    #[qjs(get)]
    pub fn methods(&self) -> Vec<JsContactMethod> {
        self.inner
            .methods
            .iter()
            .map(|method| JsContactMethod::new(method.clone(), self.db.clone()))
            .collect()
    }

    /// Adds an email address or phone number after the others and returns
    /// it, e.g. `contact.addMethod(ContactMethodKind.email, "ada@example.com")`.
    ///
    /// Saved immediately, and this object's `methods` updates. An empty
    /// `value` throws. Not available in read-only contexts.
    #[qjs(rename = "addMethod")]
    pub fn add_method<'js>(
        &mut self,
        ctx: Ctx<'js>,
        #[ts(type = "ContactMethodKind")] kind: Value<'js>,
        value: String,
    ) -> rquickjs::Result<JsContactMethod> {
        let method = self.write(
            &ctx,
            AddContactMethodCommand {
                workspace_id: self.inner.workspace_id,
                contact_id: self.inner.id,
                kind: ContactMethodKind::from_js_field(&ctx, kind)?,
                value,
            },
        )?;
        self.inner = self.read(
            &ctx,
            GetContactCommand {
                workspace_id: self.inner.workspace_id,
                contact_id: self.inner.id,
            },
        )?;
        Ok(JsContactMethod::new(method, self.db.clone()))
    }

    /// Deletes the contact; it disappears from `Workspace.contacts()`.
    ///
    /// Not available in read-only contexts.
    pub fn delete<'js>(&mut self, ctx: Ctx<'js>) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            DeleteContactCommand {
                workspace_id: self.inner.workspace_id,
                contact_id: self.inner.id,
            },
        )?;
        Ok(())
    }
}

impl JsContact {
    /// Saves the contact with `change` made to its names or notes, and
    /// keeps what was saved.
    fn save(&mut self, ctx: &Ctx<'_>, change: impl FnOnce(&mut Contact)) -> rquickjs::Result<()> {
        let mut contact = self.inner.clone();
        change(&mut contact);
        self.inner = self.write(
            ctx,
            UpdateContactCommand {
                workspace_id: contact.workspace_id,
                contact_id: contact.id,
                first_name: contact.first_name,
                middle_name: contact.middle_name,
                last_name: contact.last_name,
                notes: contact.notes,
            },
        )?;
        Ok(())
    }
}

/// A contact's photo, as `Contact.photo` reads and takes it, e.g.
/// `contact.photo = { mediaType: "image/jpeg", data }`.
#[derive(JsOptions)]
pub struct ContactPhotoImage {
    /// `"image/png"`, `"image/jpeg"`, `"image/gif"` or `"image/webp"`.
    pub media_type: String,
    /// The image, base64-encoded: at most 5 MB once encoded. A
    /// `network.fetch` with `encoding: "base64"` returns a body in this
    /// form.
    pub data: String,
}

/// One way to reach a contact: an email address or a phone number.
///
/// Get them from `Contact.methods` or `Contact.addMethod()`.
///
/// Assigning `kind` or `value` saves immediately, and `modified` updates. An
/// empty `value` throws. Not available in read-only contexts.
#[js_entity(ContactMethod)]
impl JsContactMethod {
    /// Assigning saves immediately.
    #[qjs(set, rename = "kind")]
    pub fn set_kind<'js>(&mut self, ctx: Ctx<'js>, kind: Value<'js>) -> rquickjs::Result<()> {
        let kind = ContactMethodKind::from_js_field(&ctx, kind)?;
        self.save(&ctx, kind, self.inner.value.clone())
    }

    /// Assigning saves immediately.
    #[qjs(set, rename = "value")]
    pub fn set_value<'js>(&mut self, ctx: Ctx<'js>, value: String) -> rquickjs::Result<()> {
        self.save(&ctx, self.inner.kind, value)
    }

    /// Removes the method from its contact.
    ///
    /// Not available in read-only contexts.
    pub fn delete<'js>(&mut self, ctx: Ctx<'js>) -> rquickjs::Result<()> {
        self.write(
            &ctx,
            DeleteContactMethodCommand {
                workspace_id: self.inner.workspace_id,
                method_id: self.inner.id,
            },
        )?;
        Ok(())
    }
}

impl JsContactMethod {
    /// Saves the method as `kind` and `value`, and keeps what was saved.
    fn save(
        &mut self,
        ctx: &Ctx<'_>,
        kind: ContactMethodKind,
        value: String,
    ) -> rquickjs::Result<()> {
        self.inner = self.write(
            ctx,
            UpdateContactMethodCommand {
                workspace_id: self.inner.workspace_id,
                method_id: self.inner.id,
                kind,
                value,
            },
        )?;
        Ok(())
    }
}
