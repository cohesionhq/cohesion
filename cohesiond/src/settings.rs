//! cohesiond's settings: each flag, else `config.json`, else the default.

use std::{
    fmt, fs, io,
    io::ErrorKind,
    net::{IpAddr, Ipv4Addr, UdpSocket},
    num::NonZeroU16,
    path::PathBuf,
};

use cohesionkit::{DEFAULT_PORT, Host, log::Level};
use serde::Deserialize;

use crate::{args::CohesiondArgs, daemon_storage::StoragePath};

/// `config.json` in the config directory: any of cohesiond's flags but
/// `--config`, named in snake_case. cohesiond reads it at startup and never
/// writes it; a flag or its env var wins over it.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
    host: Option<Host>,
    port: Option<NonZeroU16>,
    public_port: Option<NonZeroU16>,
    /// Relative to the config directory.
    socket: Option<PathBuf>,
    log_level: Option<Level>,
    #[serde(default)]
    exit_with_parent: bool,
    #[serde(default)]
    proxy_protocol: Vec<IpAddr>,
}

/// Why `config.json` could not be used.
#[derive(Debug)]
pub enum SettingsError {
    CannotRead(io::Error),
    Invalid(serde_json::Error),
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CannotRead(err) => write!(f, "cannot read config.json: {err}"),
            Self::Invalid(err) => write!(f, "config.json is invalid: {err}"),
        }
    }
}

/// What this run of cohesiond uses, wherever each setting came from.
pub struct Settings {
    /// Where clients reach this server, for the setup code.
    pub host: Host,
    /// The HTTPS port cohesiond listens on.
    pub port: u16,
    /// The port clients connect to, for the setup code.
    pub public_port: u16,
    pub socket: PathBuf,
    pub log_level: Level,
    pub exit_with_parent: bool,
    /// The peers whose PROXY header is believed; none turns the header off.
    pub proxy_protocol: Vec<IpAddr>,
}

impl Settings {
    pub fn resolve(args: CohesiondArgs, storage: &StoragePath) -> Result<Settings, SettingsError> {
        let file = read(storage)?;
        let port = args
            .port
            .or(file.port.map(NonZeroU16::get))
            .unwrap_or(DEFAULT_PORT);
        Ok(Settings {
            host: args
                .host
                .or(file.host)
                .unwrap_or_else(|| outbound_ip().into()),
            port,
            public_port: args
                .public_port
                .or(file.public_port.map(NonZeroU16::get))
                .unwrap_or(port),
            socket: args
                .socket
                .or(file.socket.map(|socket| storage.relative(socket)))
                .unwrap_or_else(|| storage.socket_file()),
            log_level: args.log_level.or(file.log_level).unwrap_or(Level::Info),
            exit_with_parent: args.exit_with_parent || file.exit_with_parent,
            proxy_protocol: if args.proxy_protocol.is_empty() {
                file.proxy_protocol
            } else {
                args.proxy_protocol
            },
        })
    }
}

/// The config file, or every setting unset when there is none.
fn read(storage: &StoragePath) -> Result<ConfigFile, SettingsError> {
    match fs::read(storage.config_file()) {
        Ok(contents) => serde_json::from_slice(&contents).map_err(SettingsError::Invalid),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(ConfigFile::default()),
        Err(err) => Err(SettingsError::CannotRead(err)),
    }
}

/// The address this machine uses to reach other hosts. Connecting a UDP socket
/// only picks a route; nothing is sent.
fn outbound_ip() -> IpAddr {
    UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .and_then(|socket| {
            socket.connect((Ipv4Addr::new(192, 0, 2, 1), 80))?;
            socket.local_addr()
        })
        .map(|addr| addr.ip())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
}
