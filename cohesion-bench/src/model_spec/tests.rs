use super::*;

#[test]
fn a_described_endpoint_serves_its_models() {
    let local = Endpoint::described("local=chatCompletions,http://127.0.0.1:1/v1").unwrap();
    let model = ModelUnderTest::parse("local/org/model:7b", &[local]).unwrap();
    assert_eq!(model.model, "org/model:7b");
    assert_eq!(model.slug(), "local_org_model_7b");
    match model.endpoint.preset {
        IntelligenceEndpointPreset::Custom {
            provider_identifier,
            dialect,
            base_url,
        } => {
            assert_eq!(provider_identifier, "local");
            assert_eq!(dialect, Dialect::ChatCompletions);
            assert_eq!(base_url, "http://127.0.0.1:1/v1");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_model_needs_an_endpoint_and_a_name() {
    for text in ["claude", "/claude", "anthropic/", "nowhere/model"] {
        assert!(ModelUnderTest::parse(text, &[]).is_err(), "{text}");
    }
    assert!(Endpoint::described("local=pigeon,http://x").is_err());
}
