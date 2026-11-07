use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use cohesion_core::protocol::ImageUpload;
use cohesion_db::MAX_IMAGES;

/// The images a session's scripts attached through `images`, which its
/// `send_message` shows the user: numbered from 1 in the order attached
/// across the session, of which the latest [`MAX_IMAGES`] stay, so a turn
/// can send one again without the session holding every image it saw.
#[derive(Clone, Default)]
pub(super) struct ImageTray(Arc<Mutex<Held>>);

#[derive(Default)]
struct Held {
    /// How many images were ever attached: the newest one's number.
    attached: usize,
    /// The latest of them, oldest first.
    images: VecDeque<ImageUpload>,
}

impl ImageTray {
    /// Attaches `images`, dropping the oldest beyond [`MAX_IMAGES`], and
    /// returns the note telling the model their numbers; `None` for none.
    pub(super) fn attach(&self, images: Vec<ImageUpload>) -> Option<String> {
        if images.is_empty() {
            return None;
        }
        let mut held = self.0.lock().expect("the tray is never poisoned");
        let first = held.attached + 1;
        held.attached += images.len();
        held.images.extend(images);
        while held.images.len() > MAX_IMAGES {
            held.images.pop_front();
        }
        let numbers = match held.attached - first {
            0 => format!("image {first}"),
            _ => format!("images {first} to {}", held.attached),
        };
        Some(format!(
            "Attached as {numbers}; pass `images` to `send_message` to show the user."
        ))
    }

    /// The images numbered `numbers`, in that order; the error names one
    /// never attached or no longer held.
    pub(super) fn get(&self, numbers: &[usize]) -> Result<Vec<ImageUpload>, String> {
        let held = self.0.lock().expect("the tray is never poisoned");
        let oldest = held.attached - held.images.len() + 1;
        numbers
            .iter()
            .map(|&number| match number {
                n if n > held.attached || n == 0 => Err(format!(
                    "no image {n} was attached; attach images with `images` in a script first"
                )),
                n if n < oldest => Err(format!(
                    "image {n} is no longer attached (only the latest {MAX_IMAGES} stay); \
                     attach it again"
                )),
                n => Ok(held.images[n - oldest].clone()),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
