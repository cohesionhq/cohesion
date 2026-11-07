use cohesion_core::model::Photo;
use cohesion_core::protocol::ImageUpload;

use crate::test_support::context;
use crate::{
    CommandError, DatabaseContext, GetProfileCommand, SetProfileBioCommand, SetProfileNameCommand,
    SetProfilePhotoCommand,
};

fn set_photo(db: &DatabaseContext, photo: Option<(&str, &str)>) -> Result<Photo, CommandError> {
    db.send_blocking(SetProfilePhotoCommand {
        photo: photo.map(|(media_type, data)| ImageUpload {
            media_type: media_type.to_string(),
            data: data.to_string(),
        }),
    })
    .map(|profile| profile.photo)
}

#[test]
fn the_users_photo_is_set_replaced_and_removed() {
    let db = context();
    let photo = || db.send_blocking(GetProfileCommand {}).unwrap().photo;
    assert_eq!(photo(), Photo::None);
    set_photo(&db, Some(("image/png", "AAAA"))).unwrap();
    let image = Photo::Image {
        photo_media_type: "image/jpeg".to_string(),
        photo_data: "BBBB".to_string(),
    };
    assert_eq!(set_photo(&db, Some(("image/jpeg", "BBBB"))).unwrap(), image);
    assert_eq!(photo(), image);

    // An image a message couldn't carry is refused, and the photo kept.
    for refused in [("image/tiff", "AAAA"), ("image/png", "not base64")] {
        assert!(matches!(
            set_photo(&db, Some(refused)),
            Err(CommandError::Validation(_))
        ));
    }
    assert_eq!(photo(), image);

    assert_eq!(set_photo(&db, None).unwrap(), Photo::None);
    assert_eq!(photo(), Photo::None);
}

#[test]
fn the_users_name_is_trimmed_and_an_empty_one_removed() {
    let db = context();
    let set = |name: Option<&str>| {
        db.send_blocking(SetProfileNameCommand {
            name: name.map(str::to_string),
        })
        .unwrap()
        .name
    };
    assert_eq!(db.send_blocking(GetProfileCommand {}).unwrap().name, None);
    assert_eq!(
        set(Some("  Ada Lovelace ")).as_deref(),
        Some("Ada Lovelace")
    );
    // The photo is left alone.
    db.send_blocking(SetProfilePhotoCommand {
        photo: Some(ImageUpload {
            media_type: "image/png".to_string(),
            data: "AAAA".to_string(),
        }),
    })
    .unwrap();
    assert_eq!(set(Some("   ")), None);
    assert_eq!(set(Some("Ada")).as_deref(), Some("Ada"));
    assert_eq!(set(None), None);
    let profile = db.send_blocking(GetProfileCommand {}).unwrap();
    assert!(matches!(profile.photo, Photo::Image { .. }));
}

#[test]
fn the_users_bio_is_trimmed_an_empty_one_removed_and_the_name_left_alone() {
    let db = context();
    let set = |bio: Option<&str>| {
        db.send_blocking(SetProfileBioCommand {
            bio: bio.map(str::to_string),
        })
        .unwrap()
        .bio
    };
    let saved = || db.send_blocking(GetProfileCommand {}).unwrap().bio;
    db.send_blocking(SetProfileNameCommand {
        name: Some("Ada".to_string()),
    })
    .unwrap();
    assert_eq!(saved(), None);

    // Only the spaces around it go; a line break within it stays.
    let bio = "I run a bakery.\n\nMornings are busy.";
    assert_eq!(set(Some(&format!("  {bio}\n "))).as_deref(), Some(bio));
    assert_eq!(saved().as_deref(), Some(bio));

    assert_eq!(set(Some(" \n ")), None);
    assert_eq!(saved(), None);
    assert_eq!(set(Some("I bake.")).as_deref(), Some("I bake."));
    assert_eq!(set(None), None);
    assert_eq!(saved(), None);
    let profile = db.send_blocking(GetProfileCommand {}).unwrap();
    assert_eq!(profile.name.as_deref(), Some("Ada"));
}

// The limit counts characters, not bytes, once the spaces around them are gone.
#[test]
fn a_bio_past_the_limit_is_refused_and_the_bio_kept() {
    let db = context();
    let set = |bio: String| db.send_blocking(SetProfileBioCommand { bio: Some(bio) });
    let at_limit = "é".repeat(1000);
    assert_eq!(
        set(format!(" {at_limit} ")).unwrap().bio.as_deref(),
        Some(at_limit.as_str())
    );
    assert!(matches!(
        set("é".repeat(1001)),
        Err(CommandError::Validation(_))
    ));
    let profile = db.send_blocking(GetProfileCommand {}).unwrap();
    assert_eq!(profile.bio, Some(at_limit));
}
