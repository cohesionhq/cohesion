use cohesion_core::FromRequest;
use cohesion_core::invalidation::Invalidation;
use cohesion_core::model::Profile;
use cohesion_core::protocol::{
    GetProfileRequest, ImageUpload, SetProfileBioRequest, SetProfileNameRequest,
    SetProfilePhotoRequest,
};

use crate::changed::{Changed, Invalidates};
use crate::commands::image_upload::validate_photo;
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, Found, Read, ReadableDatabase,
    ValidationError, Write,
};

/// The longest a bio runs, in characters. It sits in every agent's system
/// prompt, so each one costs something on every thread.
const BIO_LIMIT: usize = 1000;

/// `text` without the spaces around it, with blank meaning none.
fn trimmed(text: Option<String>) -> Option<String> {
    text.map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

/// The profile's one row.
#[derive(FromRequest)]
#[request(GetProfileRequest)]
pub struct GetProfileCommand {}
impl DatabaseCommand for GetProfileCommand {
    type Response = Profile;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Profile> {
        ctx.db.select().one().found()
    }
}

/// Sets the user's name, trimmed, or removes it when it is none or empty.
#[derive(FromRequest)]
#[request(SetProfileNameRequest)]
pub struct SetProfileNameCommand {
    pub name: Option<String>,
}
impl DatabaseCommand for SetProfileNameCommand {
    type Response = Profile;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Profile>> {
        let profile: Profile = ctx
            .db
            .update()
            .set(Profile::NAME, trimmed(self.name))
            .returning_one()
            .found()?;
        Ok(profile.invalidating([Invalidation::Profile]))
    }
}

/// Sets the user's bio, trimmed, or removes it when it is none or empty; one
/// past [`BIO_LIMIT`] is refused, and the bio kept.
#[derive(FromRequest)]
#[request(SetProfileBioRequest)]
pub struct SetProfileBioCommand {
    pub bio: Option<String>,
}
impl DatabaseCommand for SetProfileBioCommand {
    type Response = Profile;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Profile>> {
        let bio = trimmed(self.bio);
        if bio
            .as_ref()
            .is_some_and(|bio| bio.chars().count() > BIO_LIMIT)
        {
            return Err(ValidationError::in_field(
                "bio",
                "Keep your bio to 1,000 characters.".to_string(),
            )
            .into());
        }
        let profile: Profile = ctx
            .db
            .update()
            .set(Profile::BIO, bio)
            .returning_one()
            .found()?;
        Ok(profile.invalidating([Invalidation::Profile]))
    }
}

/// Sets the user's photo, replacing any they had, or removes it.
#[derive(FromRequest)]
#[request(SetProfilePhotoRequest)]
pub struct SetProfilePhotoCommand {
    pub photo: Option<ImageUpload>,
}
impl DatabaseCommand for SetProfilePhotoCommand {
    type Response = Profile;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<Profile>> {
        let profile: Profile = ctx
            .db
            .update()
            .set_group(validate_photo(self.photo)?)
            .returning_one()
            .found()?;
        Ok(profile.invalidating([Invalidation::Profile]))
    }
}

#[cfg(test)]
mod tests;
