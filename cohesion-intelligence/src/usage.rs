use cohesion_core::model::EndpointModel;

#[derive(Debug, Default)]
pub struct Usage {
    /// Number of tokens in. This is used to calculate costs at
    /// the input rates.
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_reads: u64,
    pub cache_writes: u64,
    /// Of `cache_writes`, how many were written to the provider's long
    /// cache: a subset as the provider reports it (the API is wire input, so
    /// pricing never trusts it past `cache_writes`), and 0 for a provider
    /// that doesn't say.
    pub long_cache_writes: u64,
}
impl Usage {
    /// How many tokens the provider counted in the request's prompt, cached
    /// or not.
    pub fn prompt_tokens(&self) -> u64 {
        // The counts are the provider's own, so they are added without trusting
        // them to fit.
        self.input_tokens
            .saturating_add(self.cache_reads)
            .saturating_add(self.cache_writes)
    }

    /// Computes what this turn's usage cost at the model's rates, a long
    /// cache write at `long_cache_write_multiple` times a short one:
    /// [`TurnCost::None`] when the model has no base input/output rates.
    pub(crate) fn cost(&self, model: &EndpointModel, long_cache_write_multiple: f64) -> TurnCost {
        self.priced(model, long_cache_write_multiple)
            .unwrap_or_default()
    }

    fn priced(&self, model: &EndpointModel, long_cache_write_multiple: f64) -> Option<TurnCost> {
        // Rates are USD per million tokens.
        const PER_TOKEN: f64 = 1.0 / 1_000_000.0;

        let prompt_tokens = self.prompt_tokens();
        let large_context = model
            .large_context_threshold
            .is_some_and(|threshold| prompt_tokens > threshold as u64);

        let pick = |large: Option<f64>, base: Option<f64>| {
            if large_context { large.or(base) } else { base }
        };

        let input_rate = pick(model.large_context_input_token_cost, model.input_token_cost)?;
        let output_rate = pick(
            model.large_context_output_token_cost,
            model.output_token_cost,
        )?;
        // Providers that don't publish separate cache rates bill those tokens as input.
        let cache_read_rate = pick(
            model.large_context_cache_read_token_cost,
            model.cache_read_token_cost,
        )
        .unwrap_or(input_rate);
        let cache_write_rate = pick(
            model.large_context_cache_write_token_cost,
            model.cache_write_token_cost,
        )
        .unwrap_or(input_rate);

        let long_cache_writes = self.long_cache_writes.min(self.cache_writes);
        let long_cache_write_rate = cache_write_rate * long_cache_write_multiple;

        let cost = |tokens: u64, rate: f64| tokens as f64 * rate * PER_TOKEN;

        Some(TurnCost::Detailed {
            input_cost: cost(self.input_tokens, input_rate),
            output_cost: cost(self.output_tokens, output_rate),
            cache_read_cost: cost(self.cache_reads, cache_read_rate),
            cache_write_cost: cost(self.cache_writes - long_cache_writes, cache_write_rate)
                + cost(long_cache_writes, long_cache_write_rate),
        })
    }
}

/// What a turn cost, in USD, as far as anyone said.
#[derive(Debug, Default)]
pub enum TurnCost {
    /// What each kind of token cost.
    Detailed {
        input_cost: f64,
        output_cost: f64,
        cache_read_cost: f64,
        cache_write_cost: f64,
    },
    /// Only what the turn cost in all.
    Simple { total_cost: f64 },
    /// Nothing: the provider didn't report a cost, so the session prices the
    /// turn's usage at the catalog's rates.
    #[default]
    None,
}

#[cfg(test)]
mod tests;
