use std::{fmt, net::IpAddr, str::FromStr};

use serde::{Deserialize, Serialize};

/// Where a client reaches cohesiond: an IP address, or a DNS name such as
/// `cohesion.example.com` for a server behind a forwarded port.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Host {
    Ip(IpAddr),
    /// Dot-separated labels of letters, digits and hyphens, lowercased.
    Name(String),
}

/// Text that is neither an IP address nor a DNS name.
#[derive(Debug)]
pub struct InvalidHost;

impl Host {
    /// `host:port`, with an IPv6 address in brackets: the authority of a URL.
    pub fn with_port(&self, port: u16) -> String {
        match self {
            Host::Ip(IpAddr::V6(ip)) => format!("[{ip}]:{port}"),
            host => format!("{host}:{port}"),
        }
    }
}

impl FromStr for Host {
    type Err = InvalidHost;

    fn from_str(text: &str) -> Result<Host, InvalidHost> {
        if let Ok(ip) = text.parse() {
            return Ok(Host::Ip(ip));
        }
        let is_label = |label: &str| {
            (1..=63).contains(&label.len())
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        };
        if text.len() <= 253 && text.split('.').all(is_label) {
            Ok(Host::Name(text.to_ascii_lowercase()))
        } else {
            Err(InvalidHost)
        }
    }
}

impl fmt::Display for Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Host::Ip(ip) => ip.fmt(f),
            Host::Name(name) => f.write_str(name),
        }
    }
}

impl TryFrom<String> for Host {
    type Error = InvalidHost;

    fn try_from(text: String) -> Result<Host, InvalidHost> {
        text.parse()
    }
}

impl From<Host> for String {
    fn from(host: Host) -> String {
        host.to_string()
    }
}

impl From<IpAddr> for Host {
    fn from(ip: IpAddr) -> Host {
        Host::Ip(ip)
    }
}

impl fmt::Display for InvalidHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("expected an IP address or a DNS name")
    }
}

impl std::error::Error for InvalidHost {}
