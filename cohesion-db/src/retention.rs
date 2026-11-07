//! How the cleanup turns the storage settings' retentions into seconds, and
//! how much freed space is worth a `VACUUM`. How long most things are kept
//! is a setting ([`StorageSettings`](cohesion_core::model::StorageSettings));
//! [`PeriodicCleanupCommand`](crate::PeriodicCleanupCommand) applies them.

/// `days` in seconds, saturating rather than overflowing.
pub(crate) fn days_in_secs(days: i64) -> i64 {
    days.saturating_mul(24 * 60 * 60)
}

/// Whether a file of `bytes` holding `free_bytes` of free pages is worth
/// vacuuming. `VACUUM` rewrites the whole file while every other command
/// waits, and SQLite reuses free pages anyway, so what retention frees and
/// fills again day after day never earns a rewrite: only a real shrink does,
/// at least 8 MiB and at least a quarter of the file.
pub(crate) fn worth_vacuuming(bytes: i64, free_bytes: i64) -> bool {
    free_bytes >= 8 * 1024 * 1024 && free_bytes.saturating_mul(4) >= bytes
}
