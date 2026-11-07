//! What a write command returns: its response and the invalidation keys it
//! changed, so a write can't commit without saying what clients must
//! refetch.

use cohesion_core::invalidation::{Invalidation, InvalidationContext};

/// A write command's response and the keys it changed, published once the
/// command commits and dropped if it fails. Made with
/// [`Invalidates::invalidating`], or [`Invalidates::invalidating_nothing`]
/// for a write no client shows.
#[must_use = "a write's keys are published only when it is returned or passed to `ctx.run`"]
pub struct Changed<T> {
    value: T,
    keys: Vec<Invalidation>,
}

impl<T> Changed<T> {
    /// Adds `keys` to what changed.
    pub fn and(mut self, keys: impl IntoIterator<Item = Invalidation>) -> Self {
        self.keys.extend(keys);
        self
    }

    /// Adds the keys `other` changed, dropping its response: for helpers
    /// whose writes are part of this one.
    pub fn merge<U>(self, other: Changed<U>) -> Self {
        self.and(other.keys)
    }

    /// The same keys, with the response mapped by `f`.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Changed<U> {
        Changed {
            value: f(self.value),
            keys: self.keys,
        }
    }

    /// What `f` makes of the response, with this one's keys added: for a
    /// write whose next step needs what the last returned.
    pub fn and_then<U, E>(
        self,
        f: impl FnOnce(T) -> Result<Changed<U>, E>,
    ) -> Result<Changed<U>, E> {
        Ok(f(self.value)?.and(self.keys))
    }

    /// The response, its keys added to `invalidations` to publish once the
    /// command commits.
    pub(crate) fn finish(self, invalidations: &mut InvalidationContext) -> T {
        for key in self.keys {
            invalidations.invalidate(key);
        }
        self.value
    }
}

/// Turns any response into a [`Changed`] one.
pub trait Invalidates: Sized {
    /// This response, having changed `keys`: one key as `[key]`, or an
    /// `Option` for a key that applies only sometimes.
    fn invalidating(self, keys: impl IntoIterator<Item = Invalidation>) -> Changed<Self> {
        Changed {
            value: self,
            keys: keys.into_iter().collect(),
        }
    }

    /// This response, from a write that changed nothing a client shows.
    fn invalidating_nothing(self) -> Changed<Self> {
        self.invalidating([])
    }
}

impl<T> Invalidates for T {}
