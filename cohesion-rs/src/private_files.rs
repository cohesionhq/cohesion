//! Files and directories only the current user may reach, for keys, tokens
//! and the database: created with their mode from the start, so there is no
//! moment at which another user could read or enter them.

use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

/// Writes `bytes` to `path` (created or truncated) readable and writable by
/// this user alone. A file that already exists is restricted before its
/// contents are replaced.
pub fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = open_private(path)?;
    file.write_all(bytes)
}

/// Opens `path` for writing, created if missing and truncated, for this user
/// alone; the mode is applied to the open file before anything is written.
pub fn open_private(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    // Truncate only after the mode is right, so an old file's contents are
    // never rewritten under looser permissions.
    let file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    file.set_len(0)?;
    Ok(file)
}

/// Creates `path` and any missing parents, each enterable by this user alone.
/// A directory that already exists keeps its mode.
pub fn create_private_dir(path: &Path) -> io::Result<()> {
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}
