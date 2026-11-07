use cohesion_macros::{ColumnGroup, Model};

use super::intelligence_provider::{Dialect, LongCache};

/// A place to send Intelligence requests, with the models it offers in
/// `endpoint_models`. Two endpoints may share a provider, such as a work
/// and a personal account, each with its own name and its own models.
#[derive(Clone, Model)]
#[model(table = "intelligence_endpoints", backup = Included)]
pub struct IntelligenceEndpoint {
    /// Unique, stable identifier.
    pub id: i64,

    /// What the endpoint is called, such as `Anthropic`; unique, ignoring
    /// case.
    pub name: String,
    /// What serves the endpoint.
    #[model(group)]
    pub backend: EndpointBackend,
}

/// What serves an endpoint's requests. The table's check gives every row
/// exactly one of these.
#[derive(Clone, ColumnGroup)]
pub enum EndpointBackend {
    /// Cohesion's built-in Rust backends: a provider's API, or any server
    /// speaking one of the dialects.
    Integrated {
        /// The models.dev provider identifier, used to look up models and
        /// pricing in the catalog.
        provider_identifier: String,
        /// Which dialect the built-in backend speaks to the server.
        dialect: Dialect,
        /// The base URL that requests are sent to.
        base_url: String,
        /// The API key sent with each request, if the server needs one.
        /// Kept from scripts and clients.
        api_key: Option<String>,
        /// The page where a person sees what the endpoint has used, such as
        /// the provider's usage dashboard.
        usage_url: Option<String>,
        /// How many minutes the server keeps a prompt cached after the
        /// request that last used it, by default: an agent's thread idle
        /// for longer is rebuilt rather than resent whole. At least 1.
        cache_minutes: i64,
        /// The longer cache a request may ask for ([`CacheLength::Long`]),
        /// or none when the server offers none.
        long_cache: Option<LongCache>,
    },
    /// A built-in backend whose key is an extension's account's credential,
    /// such as a ChatGPT sign-in: each turn asks for it fresh, so the
    /// extension's `completeRefresh` renews it before it expires. Removing
    /// the account, or uninstalling its extension, removes the endpoint.
    Account {
        /// The models.dev provider identifier, used to look up models and
        /// pricing in the catalog.
        provider_identifier: String,
        /// Which dialect the built-in backend speaks to the server.
        dialect: Dialect,
        /// The base URL that requests are sent to.
        base_url: String,
        /// The connector account whose credential each request carries.
        account_id: i64,
        /// The page where a person sees what the endpoint has used.
        usage_url: Option<String>,
        /// How many minutes the server keeps a prompt cached after the
        /// request that last used it. At least 1.
        cache_minutes: i64,
        /// The longer cache a request may ask for, or none when the server
        /// offers none.
        long_cache: Option<LongCache>,
    },
}
impl EndpointBackend {
    /// What a request asking for `length` gets from this endpoint: `length`
    /// itself, or its short cache when it offers no long one.
    pub fn cache_lifetime(&self, length: CacheLength) -> CacheLifetime {
        let (EndpointBackend::Integrated {
            cache_minutes,
            long_cache,
            ..
        }
        | EndpointBackend::Account {
            cache_minutes,
            long_cache,
            ..
        }) = self;
        match (length, long_cache) {
            (CacheLength::Long, Some(long)) => CacheLifetime::Long(*long),
            _ => CacheLifetime::Short {
                minutes: *cache_minutes,
            },
        }
    }
}

/// Which of an endpoint's prompt caches a request asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheLength {
    /// The server's default cache, lasting `cache_minutes`.
    Short,
    /// The longer cache the endpoint offers, `long_cache`.
    Long,
}

/// The prompt cache an endpoint gives a request.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CacheLifetime {
    /// The server's default, which the request asks for by naming none.
    Short { minutes: i64 },
    /// The longer one, which the request names.
    Long(LongCache),
}
impl CacheLifetime {
    /// How many minutes the cache lasts.
    pub fn minutes(&self) -> i64 {
        match self {
            Self::Short { minutes } => *minutes,
            Self::Long(long) => long.minutes,
        }
    }
}
