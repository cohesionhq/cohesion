use cohesion_core::model::{
    CacheLength, CacheLifetime, ConnectorDefinition, Credential, Dialect, ExtensionManifest,
    IntelligenceEndpointPreset, IntelligenceProvider, LongCache, ServiceDescription,
};
use cohesion_core::protocol::{RedactedEndpointBackend, RedactedIntelligenceEndpoint};

use crate::test_support::context;
use crate::{
    CommandError, CreateAccountEndpointCommand, CreateExtensionCommand,
    CreateIntelligenceEndpointCommand, DatabaseContext, DeleteConnectorAccountCommand,
    DeleteExtensionCommand, ListIntelligenceEndpointsCommand,
    LookupIntelligenceEndpointByNameCommand, SetIntelligenceEndpointCacheMinutesCommand,
    SetIntelligenceEndpointLongCacheCommand, SetIntelligenceEndpointUsageUrlCommand,
    UpsertConnectorAccountCommand,
};

#[test]
fn a_taken_endpoint_name_is_refused() {
    let db = context();
    let create = |name: Option<&str>| {
        db.send_blocking(CreateIntelligenceEndpointCommand {
            name: name.map(str::to_string),
            api_key: Some("key".to_string()),
            usage_url: None,
            cache_minutes: None,
            long_cache: None,
            preset: IntelligenceEndpointPreset::Provider {
                preset: IntelligenceProvider::OpenAi,
            },
        })
    };

    // Unnamed, an endpoint takes its provider's name, so a second one needs its own.
    assert_eq!(create(None).unwrap().name, "OpenAI");
    assert!(matches!(create(None), Err(CommandError::Validation(_))));
    assert!(matches!(
        create(Some("OpenAI")),
        Err(CommandError::Validation(_))
    ));
    assert!(matches!(
        create(Some("openai")),
        Err(CommandError::Validation(_))
    ));
    assert_eq!(create(Some("Personal")).unwrap().name, "Personal");
}

fn usage_url(endpoint: &RedactedIntelligenceEndpoint) -> Option<&str> {
    match &endpoint.backend {
        RedactedEndpointBackend::Integrated { usage_url, .. }
        | RedactedEndpointBackend::Account { usage_url, .. } => usage_url.as_deref(),
    }
}

#[test]
fn an_endpoint_starts_with_its_presets_usage_url_and_changes_it_by_hand() {
    let db = context();
    let create = |name: &str, usage: Option<&str>| {
        db.send_blocking(CreateIntelligenceEndpointCommand {
            name: Some(name.to_string()),
            api_key: Some("key".to_string()),
            usage_url: usage.map(str::to_string),
            cache_minutes: None,
            long_cache: None,
            preset: IntelligenceEndpointPreset::Provider {
                preset: IntelligenceProvider::OpenRouter,
            },
        })
        .unwrap()
    };
    let set = |id: i64, usage: Option<&str>| {
        db.send_blocking(SetIntelligenceEndpointUsageUrlCommand {
            id,
            usage_url: usage.map(str::to_string),
        })
    };

    let preset = create("Work", None);
    assert_eq!(
        usage_url(&preset),
        Some(IntelligenceProvider::OpenRouter.defaults().usage_url)
    );
    let given = create("Personal", Some("https://example.com/usage"));
    assert_eq!(usage_url(&given), Some("https://example.com/usage"));

    // Setting it keeps the rest of the backend, the key included.
    assert_eq!(usage_url(&set(preset.id, None).unwrap()), None);
    let backfilled = set(preset.id, Some("https://example.com/work")).unwrap();
    assert_eq!(usage_url(&backfilled), Some("https://example.com/work"));
    let stored = db
        .send_blocking(LookupIntelligenceEndpointByNameCommand {
            name: "Work".to_string(),
        })
        .unwrap();
    assert!(matches!(
        stored.backend,
        cohesion_core::model::EndpointBackend::Integrated {
            api_key: Some(_),
            ..
        }
    ));
}

fn cache_minutes(endpoint: &RedactedIntelligenceEndpoint) -> i64 {
    match &endpoint.backend {
        RedactedEndpointBackend::Integrated { cache_minutes, .. }
        | RedactedEndpointBackend::Account { cache_minutes, .. } => *cache_minutes,
    }
}

#[test]
fn an_endpoint_starts_with_its_presets_cache_lifetime_and_changes_it_by_hand() {
    let db = context();
    let create = |name: &str, preset: IntelligenceEndpointPreset, minutes: Option<i64>| {
        db.send_blocking(CreateIntelligenceEndpointCommand {
            name: Some(name.to_string()),
            api_key: Some("key".to_string()),
            usage_url: None,
            cache_minutes: minutes,
            long_cache: None,
            preset,
        })
    };
    let provider = |preset| IntelligenceEndpointPreset::Provider { preset };
    let set = |id: i64, minutes: i64| {
        db.send_blocking(SetIntelligenceEndpointCacheMinutesCommand {
            id,
            cache_minutes: minutes,
        })
    };

    let openai = create("OpenAI", provider(IntelligenceProvider::OpenAi), None).unwrap();
    assert_eq!(cache_minutes(&openai), 30);
    let anthropic = create("Anthropic", provider(IntelligenceProvider::Anthropic), None).unwrap();
    assert_eq!(cache_minutes(&anthropic), 5);
    let custom = IntelligenceEndpointPreset::Custom {
        provider_identifier: "ollama".to_string(),
        dialect: Dialect::ChatCompletions,
        base_url: "http://localhost:11434/v1".to_string(),
    };
    let local = create("Local", custom, None).unwrap();
    assert_eq!(cache_minutes(&local), 5);
    let given = create("Work", provider(IntelligenceProvider::OpenAi), Some(60)).unwrap();
    assert_eq!(cache_minutes(&given), 60);
    assert!(matches!(
        create("None", provider(IntelligenceProvider::OpenAi), Some(0)),
        Err(CommandError::Validation(_))
    ));

    assert_eq!(cache_minutes(&set(openai.id, 10).unwrap()), 10);
    assert!(matches!(
        set(openai.id, 0),
        Err(CommandError::Validation(_))
    ));
}

#[test]
fn a_long_cache_is_the_presets_or_given_and_an_endpoint_without_one_gives_its_short_cache() {
    let db = context();
    let lifetime = |name: &str| {
        db.send_blocking(LookupIntelligenceEndpointByNameCommand {
            name: name.to_string(),
        })
        .unwrap()
        .backend
        .cache_lifetime(CacheLength::Long)
    };
    let create = |name: &str, preset: IntelligenceEndpointPreset, long_cache: Option<LongCache>| {
        db.send_blocking(CreateIntelligenceEndpointCommand {
            name: Some(name.to_string()),
            api_key: Some("key".to_string()),
            usage_url: None,
            cache_minutes: None,
            long_cache,
            preset,
        })
    };
    let provider = |preset| IntelligenceEndpointPreset::Provider { preset };
    let custom = || IntelligenceEndpointPreset::Custom {
        provider_identifier: "proxy".to_string(),
        dialect: Dialect::Messages,
        base_url: "http://localhost:8080/v1".to_string(),
    };
    let long = |minutes, write_multiple| LongCache {
        minutes,
        write_multiple,
    };

    create("OpenAI", provider(IntelligenceProvider::OpenAi), None).unwrap();
    assert_eq!(lifetime("OpenAI"), CacheLifetime::Long(long(720, 2.0)));
    create("Anthropic", provider(IntelligenceProvider::Anthropic), None).unwrap();
    assert_eq!(lifetime("Anthropic"), CacheLifetime::Long(long(60, 1.6)));
    create("Mistral", provider(IntelligenceProvider::Mistral), None).unwrap();
    assert_eq!(lifetime("Mistral"), CacheLifetime::Short { minutes: 5 });

    let local = create("Local", custom(), None).unwrap();
    assert_eq!(lifetime("Local"), CacheLifetime::Short { minutes: 5 });
    db.send_blocking(SetIntelligenceEndpointLongCacheCommand {
        id: local.id,
        long_cache: Some(long(30, 1.5)),
    })
    .unwrap();
    assert_eq!(lifetime("Local"), CacheLifetime::Long(long(30, 1.5)));
    db.send_blocking(SetIntelligenceEndpointLongCacheCommand {
        id: local.id,
        long_cache: None,
    })
    .unwrap();
    assert_eq!(lifetime("Local"), CacheLifetime::Short { minutes: 5 });

    create("Proxy", custom(), Some(long(60, 2.0))).unwrap();
    assert_eq!(lifetime("Proxy"), CacheLifetime::Long(long(60, 2.0)));
    for refused in [long(0, 2.0), long(60, 0.0)] {
        assert!(matches!(
            create("Refused", custom(), Some(refused)),
            Err(CommandError::Validation(_))
        ));
    }
}

#[test]
fn a_users_endpoint_is_found_by_its_name_ignoring_case() {
    let db = context();
    db.send_blocking(CreateIntelligenceEndpointCommand {
        name: None,
        api_key: Some("key".to_string()),
        usage_url: None,
        cache_minutes: None,
        long_cache: None,
        preset: IntelligenceEndpointPreset::Provider {
            preset: IntelligenceProvider::Anthropic,
        },
    })
    .unwrap();
    let found = db
        .send_blocking(LookupIntelligenceEndpointByNameCommand {
            name: "anthropic".to_string(),
        })
        .unwrap();
    assert_eq!(found.name, "Anthropic");
}

fn account_endpoint(db: &DatabaseContext, identifier: &str, endpoint: &str) -> (i64, i64) {
    let extension = db
        .send_blocking(CreateExtensionCommand {
            script: String::new(),
            manifest: ExtensionManifest {
                identifier: identifier.to_string(),
                name: identifier.to_string(),
                version: 1,
                description: None,
                author: None,
                icon: None,
                permissions: vec![],
                hooks: vec![],
                service: None,
            },
        })
        .unwrap();
    let account = db
        .send_blocking(UpsertConnectorAccountCommand {
            extension_id: extension.id,
            connector: ConnectorDefinition {
                identifier: identifier.to_string(),
                icon: None,
                version: 1,
                description: ServiceDescription {
                    sign_in_hint: None,
                    title: "ChatGPT".to_string(),
                    unit: "Account".to_string(),
                    kinds: vec![],
                    metadata_keys: vec![],
                },
            },
            identifier: "user".to_string(),
            description: "me@example.com".to_string(),
            credential: Credential {
                authorization: "token".to_string(),
                refresh: None,
                expires_at: None,
            },
        })
        .unwrap();
    db.send_blocking(CreateAccountEndpointCommand {
        account_id: account.id,
        name: Some(endpoint.to_string()),
        provider_identifier: "openai".to_string(),
        dialect: Dialect::StatelessResponses,
        base_url: "https://api.openai.com/v1".to_string(),
        usage_url: None,
        cache_minutes: None,
    })
    .unwrap();
    (extension.id, account.id)
}

fn endpoint_names(db: &DatabaseContext) -> Vec<String> {
    db.send_blocking(ListIntelligenceEndpointsCommand {})
        .unwrap()
        .into_iter()
        .map(|endpoint| endpoint.name)
        .collect()
}

// An endpoint signing in with an account is the account's: removing the
// account removes it, and so does uninstalling its extension, which keeps
// the account (for its resources) but leaves it no credential.
#[test]
fn an_account_endpoint_goes_when_its_account_can_no_longer_sign_in() {
    let db = context();
    let (_, removed) = account_endpoint(&db, "com.example.removed", "Removed");
    let (uninstalled, _) = account_endpoint(&db, "com.example.uninstalled", "Uninstalled");
    account_endpoint(&db, "com.example.kept", "Kept");

    db.send_blocking(DeleteConnectorAccountCommand {
        account_id: removed,
    })
    .unwrap();
    db.send_blocking(DeleteExtensionCommand {
        extension_id: uninstalled,
    })
    .unwrap();

    assert_eq!(endpoint_names(&db), ["Kept"]);
}
