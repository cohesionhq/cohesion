use super::*;

fn args(text: &str) -> Vec<String> {
    text.split_whitespace().map(String::from).collect()
}

#[test]
fn flags_override_the_saved_options() {
    let mut config = Config {
        models: vec!["anthropic/a".to_string(), "openai/b".to_string()],
        only: vec!["create-basic".to_string()],
        repeat: 3,
        ..Config::default()
    };
    config
        .apply(&args(
            "--model xai/c --jobs 2 --no-prices --all --tests elsewhere",
        ))
        .unwrap();
    assert_eq!(config.models, ["xai/c"]);
    assert_eq!((config.repeat, config.jobs, config.prices), (3, 2, false));
    assert!(config.only.is_empty());
    assert_eq!(config.tests, Some(PathBuf::from("elsewhere")));
    assert!(config.apply(&args("--repeat 0")).is_err());
    assert!(config.apply(&args("--frobnicate")).is_err());
}

// The file keeps the providers and their keys, and only its owner reads it.
#[test]
fn saved_options_read_back() {
    let path = std::env::temp_dir().join(format!("cohesion-bench-{}.json", std::process::id()));
    let config = Config {
        providers: vec![Provider {
            name: "local".to_string(),
            dialect: Some(cohesionkit::model::Dialect::ChatCompletions),
            base_url: Some("http://127.0.0.1:1/v1".to_string()),
            api_key: Some("sk-local".to_string()),
            ..Provider::default()
        }],
        models: vec!["local/llama".to_string()],
        repeat: 2,
        ..Config::default()
    };
    config.save(&path).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    let loaded = Config::load(&path).unwrap();
    _ = std::fs::remove_file(&path);
    assert_eq!(loaded, config);
    let models = loaded.models_under_test().unwrap();
    assert_eq!(models[0].model, "llama");
    assert_eq!(models[0].endpoint.api_key.as_deref(), Some("sk-local"));
    assert_eq!(Config::load(&path).unwrap(), Config::default());
}
