//! The check every title a user gives passes.

use crate::ValidationError;

/// Rejects an empty title.
pub(super) fn ensure_title(title: &str) -> Result<(), ValidationError> {
    if title.is_empty() {
        return Err(ValidationError::new("Title can’t be empty."));
    }
    Ok(())
}
