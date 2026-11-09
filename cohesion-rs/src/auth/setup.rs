use std::net::SocketAddr;

use base64::{Engine, engine::general_purpose::STANDARD};

use super::Host;

/// The HTTPS port cohesiond listens on unless told otherwise.
pub const DEFAULT_PORT: u16 = 3000;

#[derive(Debug)]
pub enum SetupCodeError {
    NotBase64,
    Malformed,
    InvalidAddress,
}

/// What a new client needs to register with cohesiond, shared as
/// `base64("<key>|<address>|<fingerprint>")`. The address is `host:port`
/// (`10.0.0.2:4000`, `[fe80::1]:4000`, `cohesion.example.com:8443`).
pub struct SetupCode {
    /// The secret cohesiond expects on `/register`.
    pub key: String,
    /// Where cohesiond can be reached.
    pub host: Host,
    /// The HTTPS port clients connect to.
    pub port: u16,
    /// [`key_fingerprint`](super::certificate::key_fingerprint) of cohesiond's certificate, so the
    /// client can tell it's talking to the right server.
    pub fingerprint: String,
}

/// Where cohesiond can be reached and the key that identifies it: what a
/// setup code carries besides its one-time key.
#[derive(Clone)]
pub struct ServerAddress {
    pub host: Host,
    /// The port clients connect to, which a forwarded port can make
    /// different from the one cohesiond listens on.
    pub port: u16,
    pub fingerprint: String,
}

impl ServerAddress {
    /// The setup code that registers a client with `key`.
    pub fn setup_code(&self, key: String) -> SetupCode {
        SetupCode {
            key,
            host: self.host.clone(),
            port: self.port,
            fingerprint: self.fingerprint.clone(),
        }
    }
}

impl SetupCode {
    /// The address part of the code, and how the CLI names the server it
    /// points at: `host:port` (`[v6]:port` for IPv6).
    pub fn address(&self) -> String {
        self.host.with_port(self.port)
    }

    pub fn encode(&self) -> String {
        STANDARD.encode(format!(
            "{}|{}|{}",
            self.key,
            self.address(),
            self.fingerprint
        ))
    }

    pub fn decode(code: &str) -> Result<SetupCode, SetupCodeError> {
        let bytes = STANDARD
            .decode(code.trim())
            .map_err(|_| SetupCodeError::NotBase64)?;
        let text = String::from_utf8(bytes).map_err(|_| SetupCodeError::Malformed)?;
        let mut parts = text.split('|');
        let (Some(key), Some(address), Some(fingerprint), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(SetupCodeError::Malformed);
        };
        let (host, port) = parse_address(address).ok_or(SetupCodeError::InvalidAddress)?;
        Ok(SetupCode {
            key: key.to_string(),
            host,
            port,
            fingerprint: fingerprint.to_string(),
        })
    }
}

/// Splits `host:port` or `[v6]:port`.
fn parse_address(address: &str) -> Option<(Host, u16)> {
    if let Ok(socket) = address.parse::<SocketAddr>() {
        return Some((Host::Ip(socket.ip()), socket.port()));
    }
    let (host, port) = address.rsplit_once(':')?;
    Some((host.parse().ok()?, port.parse().ok()?))
}

#[cfg(test)]
mod tests;
