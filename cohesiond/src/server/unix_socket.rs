//! The Unix socket local clients connect to.

use std::future::Future;
use std::io;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::path::{Path, PathBuf};

use tokio::net::{UnixListener, UnixStream};

pub struct SocketServer {
    path: PathBuf,
    listener: UnixListener,
}

impl SocketServer {
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_socket() => std::fs::remove_file(&path)?,
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("{} exists and is not a socket", path.display()),
                ));
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        // Socket connections skip registration, so only the owner may connect:
        // the socket is bound under a mask that grants no one else access,
        // never briefly open to them.
        let listener = bind_owner_only(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        Ok(Self { path, listener })
    }

    pub async fn accept(&self) -> io::Result<UnixStream> {
        let (stream, _) = self.listener.accept().await?;
        Ok(stream)
    }

    pub async fn run<F, Fut>(&self, handler: F) -> io::Result<()>
    where
        F: Fn(UnixStream) -> Fut,
        Fut: Future<Output = ()> + Send + 'static,
    {
        loop {
            let stream = self.accept().await?;
            tokio::spawn(handler(stream));
        }
    }
}

/// The C library's `mode_t`.
#[cfg(target_os = "macos")]
type Mode = u16;
#[cfg(not(target_os = "macos"))]
type Mode = u32;

unsafe extern "C" {
    fn umask(mask: Mode) -> Mode;
    fn geteuid() -> u32;
}

/// The user id the daemon runs as, the only user the socket serves.
pub fn daemon_uid() -> u32 {
    // SAFETY: geteuid takes no arguments and cannot fail.
    unsafe { geteuid() }
}

/// Binds `path` with the process's umask set to leave group and others
/// nothing, then restores it.
fn bind_owner_only(path: &Path) -> io::Result<UnixListener> {
    // SAFETY: umask only swaps the process's file creation mask.
    let previous = unsafe { umask(0o077) };
    let bound = UnixListener::bind(path);
    // SAFETY: as above.
    unsafe { umask(previous) };
    bound
}

impl Drop for SocketServer {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
