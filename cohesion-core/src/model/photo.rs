use cohesion_macros::{ColumnGroup, TsWire};
use serde::{Deserialize, Serialize};

/// A photo of someone, a contact or the user, or that there is none;
/// stored as two nullable columns set together.
///
/// - `"none"`: no photo.
/// - `"image"`: an image of them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ColumnGroup, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Photo {
    None,
    Image {
        /// `image/png`, `image/jpeg`, `image/gif` or `image/webp`.
        photo_media_type: String,
        /// The image, base64-encoded.
        photo_data: String,
    },
}
