//! This machine's host name, reported by the `About` request.

use std::io;

#[cfg(unix)]
pub fn hostname() -> io::Result<String> {
    use std::ffi::{CStr, c_char, c_int};

    unsafe extern "C" {
        fn gethostname(name: *mut c_char, len: usize) -> c_int;
    }

    let mut buf = [0u8; 256];
    let rc = unsafe { gethostname(buf.as_mut_ptr() as *mut c_char, buf.len()) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    // POSIX doesn't guarantee NUL termination if the name was truncated
    buf[buf.len() - 1] = 0;

    let s = unsafe { CStr::from_ptr(buf.as_ptr() as *const c_char) };
    Ok(s.to_string_lossy().into_owned())
}

#[cfg(windows)]
pub fn hostname() -> io::Result<String> {
    // COMPUTER_NAME_FORMAT::ComputerNameDnsHostname
    const COMPUTER_NAME_DNS_HOSTNAME: i32 = 1;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetComputerNameExW(name_type: i32, buffer: *mut u16, size: *mut u32) -> i32;
    }

    // First call with a null buffer fails with ERROR_MORE_DATA and
    // writes the required length (including the NUL) into `size`.
    let mut size: u32 = 0;
    unsafe {
        GetComputerNameExW(COMPUTER_NAME_DNS_HOSTNAME, std::ptr::null_mut(), &mut size);
    }
    if size == 0 {
        return Err(io::Error::last_os_error());
    }

    let mut buf = vec![0u16; size as usize];
    let ok = unsafe { GetComputerNameExW(COMPUTER_NAME_DNS_HOSTNAME, buf.as_mut_ptr(), &mut size) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }

    // On success, `size` is the length in UTF-16 units, excluding the NUL
    buf.truncate(size as usize);
    Ok(String::from_utf16_lossy(&buf))
}

#[cfg(not(any(unix, windows)))]
pub fn hostname() -> io::Result<String> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "hostname is not supported on this platform",
    ))
}
