//! Requests about the user's own profile: their name, bio and photo.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

use super::ImageUpload;

/// Answers the user's profile.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct GetProfileRequest;

/// Sets the user's name, without the spaces around it, or removes it when
/// it is `null` or empty.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetProfileNameRequest {
    pub name: Option<String>,
}

/// Sets the user's bio, without the spaces around it, or removes it when it
/// is `null` or empty. A bio past 1,000 characters is refused.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetProfileBioRequest {
    pub bio: Option<String>,
}

/// Sets the user's photo, replacing any they had, or removes it when
/// `photo` is `null`. The photo passes the same check as an image attached
/// to a message.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
pub struct SetProfilePhotoRequest {
    pub photo: Option<ImageUpload>,
}
