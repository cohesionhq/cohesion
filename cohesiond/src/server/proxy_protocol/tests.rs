use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cohesionkit::db::{Container, ListClientsCommand, ListSecurityEventPageCommand, Secrets};
use cohesionkit::{Authentication, Client, CohesionContext, Host, ServerAddress};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, SignatureScheme};
use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

use super::{HEADER_WITHIN, MAX_ADDRESSES, SIGNATURE, TCP4, TCP6, TrustedProxies, read_source};
use crate::server::serve;

fn tcp4_header(source: Ipv4Addr, port: u16) -> Vec<u8> {
    let mut header = SIGNATURE.to_vec();
    header.extend([0x21, TCP4, 0, 12]);
    header.extend(source.octets());
    header.extend([198, 51, 100, 1]);
    header.extend(port.to_be_bytes());
    header.extend(8443u16.to_be_bytes());
    header
}

fn tcp6_header(source: Ipv6Addr, port: u16) -> Vec<u8> {
    let mut header = SIGNATURE.to_vec();
    header.extend([0x21, TCP6, 0, 36]);
    header.extend(source.octets());
    header.extend(Ipv6Addr::LOCALHOST.octets());
    header.extend(port.to_be_bytes());
    header.extend(8443u16.to_be_bytes());
    header
}

/// What `read_source` makes of a peer that sends `sent` and hangs up.
async fn read_all(sent: &[u8]) -> std::io::Result<SocketAddr> {
    let (mut near, mut far) = duplex(1024);
    far.write_all(sent).await.unwrap();
    drop(far);
    read_source(&mut near).await
}

// The parser, on bytes.

#[tokio::test]
async fn a_header_gives_its_source_and_leaves_what_follows_it() {
    for (header, source) in [
        (
            tcp4_header(Ipv4Addr::new(203, 0, 113, 7), 4444),
            "203.0.113.7:4444",
        ),
        (
            tcp6_header("2001:db8::7".parse().unwrap(), 4444),
            "[2001:db8::7]:4444",
        ),
        // A dual-stack router names an IPv4 client this way.
        (
            tcp6_header(Ipv4Addr::new(203, 0, 113, 7).to_ipv6_mapped(), 4444),
            "203.0.113.7:4444",
        ),
    ] {
        let (mut near, mut far) = duplex(1024);
        // In one write, as the Router sends it with the ClientHello.
        far.write_all(&[header.as_slice(), b"client hello"].concat())
            .await
            .unwrap();
        drop(far);
        let read = read_source(&mut near).await.unwrap();
        assert_eq!(read.to_string(), source);
        let mut rest = Vec::new();
        near.read_to_end(&mut rest).await.unwrap();
        assert_eq!(rest, b"client hello");
    }
}

#[tokio::test]
async fn extensions_after_the_addresses_are_skipped() {
    let mut header = tcp4_header(Ipv4Addr::new(203, 0, 113, 7), 4444);
    header[15] = 12 + 5;
    header.extend([0x04, 0, 2, 9, 9]);
    header.extend(b"client hello");
    let (mut near, mut far) = duplex(1024);
    far.write_all(&header).await.unwrap();
    drop(far);
    assert_eq!(
        read_source(&mut near).await.unwrap().to_string(),
        "203.0.113.7:4444"
    );
    let mut rest = Vec::new();
    near.read_to_end(&mut rest).await.unwrap();
    assert_eq!(rest, b"client hello");
}

#[tokio::test]
async fn a_malformed_header_is_refused() {
    let good = tcp4_header(Ipv4Addr::new(203, 0, 113, 7), 4444);
    let with = |at: usize, byte: u8| {
        let mut header = good.clone();
        header[at] = byte;
        header
    };
    for bad in [
        with(0, 0x16),  // the signature, or a ClientHello with no header
        with(12, 0x11), // version 1
        with(12, 0x20), // command LOCAL
        with(13, 0x00), // family UNSPEC
        with(13, 0x12), // UDP over IPv4
        with(13, 0x31), // Unix
        with(15, 11),   // too short for IPv4's addresses
        [good[..13].to_vec(), vec![TCP6, 0, 12], good[16..].to_vec()].concat(), // too short for IPv6's
    ] {
        let err = read_all(&bad).await.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData, "{bad:?}");
    }
    // Real TLS with no header in front of it, from the peer that must send one.
    let client_hello = [[0x16, 0x03, 0x01, 0x00, 0xc8, 0x01].as_slice(), &[0; 200]].concat();
    assert_eq!(
        read_all(&client_hello).await.unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );
}

#[tokio::test]
async fn a_header_longer_than_the_limit_is_refused_without_reading_it() {
    let at_limit = MAX_ADDRESSES as u16;
    let header = |length: u16| {
        let mut header = SIGNATURE.to_vec();
        header.extend([0x21, TCP4]);
        header.extend(length.to_be_bytes());
        header
    };
    // The limit itself is fine.
    let mut largest = header(at_limit);
    largest.extend(vec![1; MAX_ADDRESSES]);
    assert!(read_all(&largest).await.is_ok());
    // Past it, the prefix alone is refused: none of the rest is ever sent.
    for length in [at_limit + 1, u16::MAX] {
        let (mut near, mut far) = duplex(1024);
        far.write_all(&header(length)).await.unwrap();
        let err = read_source(&mut near).await.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        drop(far);
    }
}

#[tokio::test]
async fn a_truncated_header_is_refused_at_every_length() {
    for header in [
        tcp4_header(Ipv4Addr::new(203, 0, 113, 7), 4444),
        tcp6_header("2001:db8::7".parse().unwrap(), 4444),
    ] {
        for cut in 0..header.len() {
            let err = read_all(&header[..cut]).await.unwrap_err();
            assert_eq!(
                err.kind(),
                std::io::ErrorKind::UnexpectedEof,
                "cut at {cut}"
            );
        }
    }
}

#[tokio::test(start_paused = true)]
async fn a_silent_peer_times_out() {
    let (mut near, _far) = duplex(1024);
    let started = tokio::time::Instant::now();
    let err = read_source(&mut near).await.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
    assert_eq!(started.elapsed(), HEADER_WITHIN);
}

/// A byte at a time keeps every read moving, so the limit must be on the whole
/// header rather than each read.
#[tokio::test(start_paused = true)]
async fn a_trickling_peer_times_out() {
    let header = tcp4_header(Ipv4Addr::new(203, 0, 113, 7), 4444);
    let (mut near, mut far) = duplex(1024);
    let trickle = tokio::spawn(async move {
        for byte in header {
            tokio::time::sleep(Duration::from_secs(1)).await;
            if far.write_all(&[byte]).await.is_err() {
                return;
            }
        }
    });
    let started = tokio::time::Instant::now();
    let err = read_source(&mut near).await.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
    assert_eq!(started.elapsed(), HEADER_WITHIN);
    trickle.abort();
}

/// No byte of a header can panic the parser, whatever it decides.
#[tokio::test]
async fn no_single_changed_byte_panics() {
    for header in [
        tcp4_header(Ipv4Addr::new(203, 0, 113, 7), 4444),
        tcp6_header("2001:db8::7".parse().unwrap(), 4444),
    ] {
        for at in 0..header.len() {
            for byte in 0..=u8::MAX {
                let mut changed = header.clone();
                changed[at] = byte;
                let read = read_all(&changed).await;
                // The signature and version decide alone; other bytes may yield
                // another readable header.
                if at <= SIGNATURE.len() && byte != header[at] {
                    read.unwrap_err();
                }
            }
        }
    }
}

#[tokio::test]
async fn only_a_listed_peer_is_asked_for_a_header() {
    let router = SocketAddr::new(Ipv4Addr::new(10, 0, 0, 2).into(), 50000);
    let stranger = SocketAddr::new(Ipv4Addr::new(10, 0, 0, 9).into(), 50000);
    let header = tcp4_header(Ipv4Addr::new(203, 0, 113, 7), 4444);
    let proxies = TrustedProxies::new([router.ip()]);

    let (mut near, mut far) = duplex(1024);
    far.write_all(&header).await.unwrap();
    assert_eq!(
        proxies
            .client_address(&mut near, router)
            .await
            .unwrap()
            .ip(),
        IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7))
    );

    // The stranger's header is left unread, and it is taken for who it is.
    let (mut near, mut far) = duplex(1024);
    far.write_all(&header).await.unwrap();
    drop(far);
    assert_eq!(
        proxies.client_address(&mut near, stranger).await.unwrap(),
        stranger
    );
    let mut rest = Vec::new();
    near.read_to_end(&mut rest).await.unwrap();
    assert_eq!(rest, header);

    // A peer seen as a mapped address is the same peer.
    let mapped = SocketAddr::new(Ipv4Addr::new(10, 0, 0, 2).to_ipv6_mapped().into(), 50000);
    let (mut near, mut far) = duplex(1024);
    far.write_all(&header).await.unwrap();
    assert_eq!(
        proxies
            .client_address(&mut near, mapped)
            .await
            .unwrap()
            .ip(),
        IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7))
    );
}

// The real server: its acceptor, TLS and routes, over TCP on loopback.

/// A running daemon, on a free port, believing headers from `proxies`.
struct Daemon {
    context: CohesionContext,
    port: u16,
    server: tokio::task::JoinHandle<std::io::Result<()>>,
}

impl Daemon {
    async fn start(proxies: &[IpAddr]) -> Self {
        let mut container = Container::in_memory().unwrap();
        container.migrate().unwrap();
        let key = Authentication::generate().unwrap();
        let context = CohesionContext::new(
            container,
            Secrets::new(),
            ServerAddress {
                host: Host::Ip(IpAddr::V4(Ipv4Addr::LOCALHOST)),
                port: 3000,
                fingerprint: key.fingerprint(),
            },
        );
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let socket = std::env::temp_dir().join(format!("cpp-{}-{port}.sock", std::process::id()));
        let server = tokio::spawn(serve::start(
            context.clone(),
            key.self_signed_certificate("cohesiond").unwrap(),
            port,
            socket,
            proxies.to_vec(),
        ));
        let deadline = Instant::now() + Duration::from_secs(10);
        while TcpStream::connect(("127.0.0.1", port)).await.is_err() {
            assert!(Instant::now() < deadline, "the daemon never listened");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        Self {
            context,
            port,
            server,
        }
    }

    /// Writes `header`, then attempts `POST path` over mutual TLS as `identity`,
    /// and returns the response, or `None` if the connection was closed on it.
    async fn post(
        &self,
        identity: &Authentication,
        header: &[u8],
        path: &str,
        body: &str,
    ) -> Option<String> {
        let mut tcp = TcpStream::connect(("127.0.0.1", self.port)).await.unwrap();
        tcp.write_all(header).await.ok()?;
        let cert = identity.self_signed_certificate("test").unwrap();
        let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
        let config = ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .unwrap()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AnyServer))
            .with_client_auth_cert(cert.chain, cert.private_key)
            .unwrap();
        let name = ServerName::try_from("cohesiond").unwrap();
        let mut tls = TlsConnector::from(Arc::new(config))
            .connect(name, tcp)
            .await
            .ok()?;
        let request = format!(
            "POST {path} HTTP/1.1\r\nhost: cohesiond\r\nconnection: close\r\n\
             content-length: {}\r\n\r\n{body}",
            body.len()
        );
        tls.write_all(request.as_bytes()).await.ok()?;
        let mut response = Vec::new();
        let _ = tls.read_to_end(&mut response).await;
        (!response.is_empty()).then(|| String::from_utf8_lossy(&response).into_owned())
    }

    /// A pairing attempt with the wrong setup key, after `header`.
    async fn refused_pairing(&self, header: &[u8]) -> Option<String> {
        self.post(
            &Authentication::generate().unwrap(),
            header,
            "/register",
            r#"{"setup_key":"wrong","client_name":"Stranger"}"#,
        )
        .await
    }

    /// The addresses of the refused pairings on record, once `expected` of them
    /// are: the record is written after the reply.
    async fn refused_addresses(&self, expected: usize) -> Vec<String> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let events = self
                .context
                .database
                .send(ListSecurityEventPageCommand {
                    offset: 0,
                    limit: Some(50),
                })
                .await
                .unwrap()
                .events;
            if events.len() >= expected || Instant::now() >= deadline {
                let mut addresses: Vec<_> = events
                    .into_iter()
                    .filter_map(|event| event.address)
                    .collect();
                addresses.sort();
                return addresses;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// The one client, once it was last seen from `address`, or as it stands
    /// when that never happens.
    async fn client_seen_from(&self, address: &str) -> Client {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let clients = self
                .context
                .database
                .send(ListClientsCommand {})
                .await
                .unwrap();
            let client = clients.into_iter().next().unwrap();
            if client.last_address.as_deref() == Some(address) || Instant::now() >= deadline {
                return client;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        self.server.abort();
    }
}

#[derive(Debug)]
struct AnyServer;

impl ServerCertVerifier for AnyServer {
    fn verify_server_cert(
        &self,
        _: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &CertificateDer<'_>,
        _: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _: &[u8],
        _: &CertificateDer<'_>,
        _: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::aws_lc_rs::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

const LOOPBACK: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
/// A peer this test can't connect from, so that loopback is a stranger.
const SOMEONE_ELSE: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1));

fn is_401(response: Option<String>) -> bool {
    response.is_some_and(|response| response.starts_with("HTTP/1.1 401"))
}

#[tokio::test]
async fn real_addresses_reach_the_security_record() {
    let daemon = Daemon::start(&[LOOPBACK]).await;
    let v4 = tcp4_header(Ipv4Addr::new(203, 0, 113, 7), 4444);
    let v6 = tcp6_header("2001:db8::7".parse().unwrap(), 4444);
    assert!(is_401(daemon.refused_pairing(&v4).await));
    assert!(is_401(daemon.refused_pairing(&v6).await));
    // One record per real address, so a second from the same one adds nothing,
    // where counting the Router's address would have stopped at the first.
    assert!(is_401(daemon.refused_pairing(&v4).await));
    assert_eq!(
        daemon.refused_addresses(2).await,
        ["2001:db8::7", "203.0.113.7"]
    );
}

#[tokio::test]
async fn real_addresses_reach_the_client_list() {
    let daemon = Daemon::start(&[LOOPBACK]).await;
    let identity = Authentication::generate().unwrap();
    let setup_key = daemon.context.secrets.begin_pairing();
    let body = format!(r#"{{"setup_key":"{setup_key}","client_name":"Phone"}}"#);

    // Pairing over IPv4 lists the client there.
    let v4 = tcp4_header(Ipv4Addr::new(203, 0, 113, 7), 4444);
    let response = daemon
        .post(&identity, &v4, "/register", &body)
        .await
        .unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    let client = daemon.client_seen_from("203.0.113.7").await;
    assert_eq!(client.last_address.as_deref(), Some("203.0.113.7"));

    // Connecting again over IPv6 moves it, and it stays a registered client.
    let v6 = tcp6_header("2001:db8::7".parse().unwrap(), 4444);
    let response = daemon
        .post(
            &identity,
            &v6,
            "/command",
            r#"{"type":"listClients","id":1}"#,
        )
        .await
        .unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    let client = daemon.client_seen_from("2001:db8::7").await;
    assert_eq!(client.last_address.as_deref(), Some("2001:db8::7"));
}

#[tokio::test]
async fn a_forged_header_from_an_untrusted_peer_is_not_believed() {
    let daemon = Daemon::start(&[SOMEONE_ELSE]).await;
    let forged = tcp4_header(Ipv4Addr::new(203, 0, 113, 7), 4444);
    // The header reaches rustls as bytes that aren't a ClientHello: closed.
    assert_eq!(daemon.refused_pairing(&forged).await, None);
    // The same peer speaking plain TLS is served, under its own address.
    assert!(is_401(daemon.refused_pairing(&[]).await));
    assert_eq!(daemon.refused_addresses(1).await, ["127.0.0.1"]);
}

#[tokio::test]
async fn a_trusted_peer_without_a_good_header_gets_nothing() {
    let daemon = Daemon::start(&[LOOPBACK]).await;
    // No header: TLS straight away fails, rather than showing the Router's address.
    assert_eq!(daemon.refused_pairing(&[]).await, None);
    let mut malformed = tcp4_header(Ipv4Addr::new(203, 0, 113, 7), 4444);
    malformed[13] = 0x31;
    assert_eq!(daemon.refused_pairing(&malformed).await, None);
    let mut too_long = tcp4_header(Ipv4Addr::new(203, 0, 113, 7), 4444);
    too_long[14] = 0xff;
    assert_eq!(daemon.refused_pairing(&too_long).await, None);
    // A short header runs on into the ClientHello, which is then cut in two.
    assert_eq!(
        daemon
            .refused_pairing(&tcp4_header(Ipv4Addr::LOCALHOST, 1)[..20])
            .await,
        None
    );
    // The daemon survived all of it, and recorded nothing false.
    let good = tcp4_header(Ipv4Addr::new(203, 0, 113, 8), 4444);
    assert!(is_401(daemon.refused_pairing(&good).await));
    assert_eq!(daemon.refused_addresses(1).await, ["203.0.113.8"]);
}

#[tokio::test]
async fn with_the_flag_off_nothing_is_read_before_tls() {
    let daemon = Daemon::start(&[]).await;
    assert!(is_401(daemon.refused_pairing(&[]).await));
    let header = tcp4_header(Ipv4Addr::new(203, 0, 113, 7), 4444);
    assert_eq!(daemon.refused_pairing(&header).await, None);
    assert_eq!(daemon.refused_addresses(1).await, ["127.0.0.1"]);
}
