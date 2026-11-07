use cohesion_core::protocol::ImageUpload;
use cohesion_db::MAX_IMAGES;

use super::ImageTray;

fn image(data: &str) -> ImageUpload {
    ImageUpload {
        media_type: "image/png".to_string(),
        data: data.to_string(),
    }
}

fn data(images: Vec<ImageUpload>) -> Vec<String> {
    images.into_iter().map(|image| image.data).collect()
}

#[test]
fn images_are_numbered_across_scripts_and_sent_in_the_order_asked() {
    let tray = ImageTray::default();
    assert_eq!(tray.attach(Vec::new()), None);
    assert!(
        tray.attach(vec![image("AAAA")])
            .unwrap()
            .contains("image 1;")
    );
    assert!(
        tray.attach(vec![image("BBBB"), image("CCCC")])
            .unwrap()
            .contains("images 2 to 3;")
    );
    assert_eq!(data(tray.get(&[3, 1]).unwrap()), ["CCCC", "AAAA"]);
    assert!(tray.get(&[0]).is_err());
    assert!(tray.get(&[4]).is_err());
}

#[test]
fn only_the_latest_images_stay_and_keep_their_numbers() {
    let tray = ImageTray::default();
    let images: Vec<_> = (0..MAX_IMAGES + 2).map(|n| image(&n.to_string())).collect();
    tray.attach(images);
    // The first two were dropped; the rest keep the numbers they were given.
    assert!(tray.get(&[2]).unwrap_err().contains("no longer attached"));
    assert_eq!(data(tray.get(&[3]).unwrap()), ["2"]);
    let last = MAX_IMAGES + 2;
    assert_eq!(data(tray.get(&[last]).unwrap()), [(last - 1).to_string()]);
}
