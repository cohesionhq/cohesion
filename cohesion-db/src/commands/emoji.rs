//! The emoji a workspace or agent shows in place of its initial, or that
//! reacts to an agent message.

use crate::ValidationError;

/// The emoji to store: trimmed, with blank meaning none. Anything longer than
/// a few characters is rejected, since it stands in for a single initial.
pub(super) fn emoji(emoji: Option<String>) -> Result<Option<String>, ValidationError> {
    let Some(emoji) = emoji
        .map(|e| e.trim().to_string())
        .filter(|e| !e.is_empty())
    else {
        return Ok(None);
    };
    if emoji.chars().count() > 8 {
        return Err(ValidationError::in_field(
            "emoji",
            "Use a single emoji.".to_string(),
        ));
    }
    Ok(Some(emoji))
}
