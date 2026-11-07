use std::fs;

pub fn hostname() -> Option<String> {
    let name = fs::read_to_string("/proc/sys/kernel/hostname").ok()?;
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_owned())
}
