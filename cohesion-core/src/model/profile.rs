//! The user's own profile: their name, bio and photo.

use cohesion_macros::{Model, TsWire};
use serde::{Deserialize, Serialize};

use super::Photo;

/// The user's profile; there is always exactly one row.
#[derive(Clone, Debug, Serialize, Deserialize, Model, TsWire)]
#[model(table = "profile", backup = Included)]
pub struct Profile {
    pub id: i64,
    /// Their name, `null` until they set one; never empty.
    pub name: Option<String>,
    /// What they wrote about themselves, which every agent reads, `null` until
    /// they write some; never empty, and at most 1,000 characters.
    pub bio: Option<String>,
    /// Their photo, if they have set one.
    #[model(group)]
    pub photo: Photo,
}
