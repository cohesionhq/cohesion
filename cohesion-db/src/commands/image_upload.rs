//! The check every image passes, the user's to an agent's message or in an
//! actor session, one an actor shows, or a photo of a contact or the user: the
//! providers' limits, which an actor's model shares.

use cohesion_core::model::{Attachment, AttachmentMessage, ImageBytes, Photo};
use cohesion_core::protocol::ImageUpload;

use crate::{CommandContext, CommandError, ValidationError};

/// The most images one message carries.
pub const MAX_IMAGES: usize = 10;
/// The largest image, in bytes of base64: the most the providers take.
const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;
/// The image types every provider reads.
const IMAGE_TYPES: [&str; 4] = ["image/png", "image/jpeg", "image/gif", "image/webp"];

/// Rejects images the providers wouldn't take: more than [`MAX_IMAGES`], or
/// one of an unknown type, whose data isn't base64, or over
/// [`MAX_IMAGE_BYTES`].
pub fn validate_images(images: &[ImageUpload]) -> Result<(), ValidationError> {
    if images.len() > MAX_IMAGES {
        return Err(ValidationError::in_field(
            "attachments",
            format!("Attach at most {MAX_IMAGES} images."),
        ));
    }
    images
        .iter()
        .try_for_each(|image| validate_image(image, "attachments"))
}

/// The photo an upload sets, or none for `None`, once the image passes the
/// same check as an attachment.
pub fn validate_photo(upload: Option<ImageUpload>) -> Result<Photo, ValidationError> {
    let Some(image) = upload else {
        return Ok(Photo::None);
    };
    validate_image(&image, "photo")?;
    Ok(Photo::Image {
        photo_media_type: image.media_type,
        photo_data: image.data,
    })
}

/// Rejects an image of an unknown type, whose data isn't base64, or over
/// [`MAX_IMAGE_BYTES`], naming `field` as the one at fault.
fn validate_image(image: &ImageUpload, field: &'static str) -> Result<(), ValidationError> {
    if !IMAGE_TYPES.contains(&image.media_type.as_str()) {
        return Err(ValidationError::in_field(
            field,
            "Attach PNG, JPEG, GIF or WebP images.".to_string(),
        ));
    }
    let data = image.data.as_bytes();
    let base64 = !data.is_empty()
        && data.len().is_multiple_of(4)
        && data
            .iter()
            .rev()
            .skip_while(|&&byte| byte == b'=')
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'+' || *byte == b'/');
    if !base64 {
        return Err(ValidationError::in_field(
            field,
            "An image couldn’t be read.".to_string(),
        ));
    }
    if data.len() > MAX_IMAGE_BYTES {
        return Err(ValidationError::in_field(
            field,
            "Attach images of at most 5 MB.".to_string(),
        ));
    }
    Ok(())
}

/// Stores `attachments`, which passed [`validate_images`], beside `message`, in
/// order.
pub(super) fn insert_attachments(
    ctx: &mut CommandContext<'_>,
    message: AttachmentMessage,
    attachments: Vec<ImageUpload>,
) -> Result<(), CommandError> {
    for attachment in attachments {
        let data = ImageBytes::from_base64(&attachment.data).map_err(|_| {
            ValidationError::in_field("attachments", "This image couldn’t be read.".to_string())
        })?;
        ctx.db
            .insert::<Attachment>()
            .set_group(message)
            .set(Attachment::MEDIA_TYPE, attachment.media_type)
            .set(Attachment::DATA, data)
            .execute()?;
    }
    Ok(())
}
