//! Which extension icons the daemon keeps from a manifest.

/// The largest icon kept, in bytes.
const MAX_ICON_BYTES: usize = 65_536;

/// Keeps `icon` when it is a `data:image/…` or `https://` URL of at most
/// 64 KB; anything else is dropped rather than failing the call.
pub(crate) fn accepted_icon(icon: Option<String>) -> Option<String> {
    icon.filter(|icon| {
        (icon.starts_with("data:image/") || icon.starts_with("https://"))
            && icon.len() <= MAX_ICON_BYTES
    })
}

#[cfg(test)]
mod tests;
