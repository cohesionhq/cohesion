use super::{Pkce, s256};

/// RFC 7636 appendix B's verifier and challenge.
#[test]
fn s256_matches_the_rfc() {
    assert_eq!(
        s256("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
}

#[test]
fn a_generated_pair_is_a_43_character_verifier_and_its_challenge() {
    let pkce = Pkce::generate();
    assert_eq!(pkce.verifier.len(), 43);
    assert_eq!(pkce.challenge.challenge, s256(&pkce.verifier));
    assert_eq!(pkce.challenge.method, "S256");
}
