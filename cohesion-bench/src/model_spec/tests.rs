use super::*;

#[test]
fn a_described_provider_serves_its_models() {
    let local = Provider::described("local=chatCompletions,http://127.0.0.1:1/v1").unwrap();
    let model = ModelUnderTest::parse("local/org/model:7b", &[local]).unwrap();
    assert_eq!(model.model, "org/model:7b");
    assert_eq!(model.slug(), "local_org_model_7b");
    assert_eq!(model.endpoint.api_key, None);
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
fn a_preset_takes_the_key_bench_json_gives_it() {
    let provider = Provider {
        name: "work-anthropic".to_string(),
        preset: Some("anthropic".to_string()),
        api_key: Some("sk-test".to_string()),
        ..Provider::default()
    };
    let model = ModelUnderTest::parse("work-anthropic/claude", &[provider]).unwrap();
    assert_eq!(model.endpoint.api_key.as_deref(), Some("sk-test"));
    assert!(matches!(
        model.endpoint.preset,
        IntelligenceEndpointPreset::Provider {
            preset: IntelligenceProvider::Anthropic
        }
    ));
}

#[test]
fn a_model_needs_a_provider_and_a_name() {
    for text in ["claude", "/claude", "anthropic/", "nowhere/model"] {
        assert!(ModelUnderTest::parse(text, &[]).is_err(), "{text}");
    }
    assert!(Provider::described("local=pigeon,http://x").is_err());
    let half = Provider {
        name: "half".to_string(),
        dialect: Some(Dialect::Messages),
        ..Provider::default()
    };
    assert!(half.endpoint().is_err());
}
