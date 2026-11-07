use cohesion_macros::Fields;

/// What one model's calls cost together, summed from its
/// [`IntelligenceCostEvent`](super::IntelligenceCostEvent)s.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ModelUsage {
    /// The provider that served the model, such as `"anthropic"`.
    pub provider: String,
    /// The model's identifier at that provider, such as `"claude-opus-5.5"`.
    pub model: String,
    /// How many times the model was called.
    pub calls: i64,
    /// Prompt tokens read fresh, not from the cache.
    pub input_tokens: i64,
    /// Tokens the model wrote.
    pub output_tokens: i64,
    /// Prompt tokens read from the provider's cache.
    pub cache_read_tokens: i64,
    /// Prompt tokens written to the provider's cache.
    pub cache_write_tokens: i64,
    /// What the calls cost, in US dollars.
    pub cost: f64,
}

/// What every model call cost together: the sum of the [`ModelUsage`]s it
/// is collected from.
#[derive(Clone, Debug, Default, PartialEq, Fields)]
pub struct IntelligenceUsage {
    /// How many model calls were made.
    pub calls: i64,
    /// Prompt tokens read fresh, not from the cache.
    pub input_tokens: i64,
    /// Tokens the models wrote.
    pub output_tokens: i64,
    /// Prompt tokens read from the providers' caches.
    pub cache_read_tokens: i64,
    /// Prompt tokens written to the providers' caches.
    pub cache_write_tokens: i64,
    /// What the calls cost, in US dollars.
    pub cost: f64,
}

impl<'a> FromIterator<&'a ModelUsage> for IntelligenceUsage {
    fn from_iter<I: IntoIterator<Item = &'a ModelUsage>>(models: I) -> Self {
        models
            .into_iter()
            .fold(Self::default(), |total, model| IntelligenceUsage {
                calls: total.calls + model.calls,
                input_tokens: total.input_tokens + model.input_tokens,
                output_tokens: total.output_tokens + model.output_tokens,
                cache_read_tokens: total.cache_read_tokens + model.cache_read_tokens,
                cache_write_tokens: total.cache_write_tokens + model.cache_write_tokens,
                cost: total.cost + model.cost,
            })
    }
}
