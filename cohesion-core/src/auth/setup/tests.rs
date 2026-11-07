//! Codec tests for the setup code. Normally a wire format isn't worth a test,
//! but this is the credential a new client pairs with: a decode regression
//! locks every client out.

use base64::{Engine, engine::general_purpose::STANDARD};

use super::*;

fn code(host: &str, port: u16) -> SetupCode {
    SetupCode {
        key: "key".into(),
        host: host.parse().unwrap(),
        port,
        fingerprint: "fp".into(),
    }
}

fn text(code: &SetupCode) -> String {
    String::from_utf8(STANDARD.decode(code.encode()).unwrap()).unwrap()
}

#[test]
fn every_port_round_trips() {
    for (host, port, expected) in [
        ("10.0.0.2", 4000, "key|10.0.0.2:4000|fp"),
        ("fe80::1", 4000, "key|[fe80::1]:4000|fp"),
        (
            "cohesion.example.com",
            4000,
            "key|cohesion.example.com:4000|fp",
        ),
        ("10.0.0.2", DEFAULT_PORT, "key|10.0.0.2:3000|fp"),
    ] {
        let original = code(host, port);
        assert_eq!(text(&original), expected);
        let decoded = SetupCode::decode(&original.encode()).unwrap();
        assert_eq!(decoded.host, original.host);
        assert_eq!(decoded.port, port);
        assert_eq!(decoded.key, "key");
        assert_eq!(decoded.fingerprint, "fp");
    }
}

#[test]
fn a_bad_address_is_refused() {
    for address in [
        "",
        "10.0.0.2",
        "fe80::1",
        "cohesion.example.com",
        "bad host:4000",
        "-host.local",
        "host..local",
        "host.local:",
        "10.0.0.2:notaport",
        "10.0.0.2:70000",
        "host.local:70000",
    ] {
        let bad = STANDARD.encode(format!("key|{address}|fp"));
        assert!(matches!(
            SetupCode::decode(&bad),
            Err(SetupCodeError::InvalidAddress)
        ));
    }
}
