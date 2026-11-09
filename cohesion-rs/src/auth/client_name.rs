/// The name a client is listed under when its user gave none: the device it
/// runs on, then the client in parentheses, as in `apollo (CLI)`, so several
/// clients on one device can be told apart. The name is only a label; a client
/// is known by its key.
pub fn generated_client_name(device: &str, client: &str) -> String {
    format!("{device} ({client})")
}

#[cfg(test)]
mod tests;
