//! This computer's hostname, which starts a newly registered client's name. One module
//! per platform, each with `hostname()`.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
mod other;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::hostname;
#[cfg(target_os = "macos")]
pub use macos::hostname;
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub use other::hostname;
#[cfg(target_os = "windows")]
pub use windows::hostname;
