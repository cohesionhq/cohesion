//! Serde definitions for the models.dev catalog (`https://models.dev/catalog.json`).
//!
//! Only the fields a `CatalogModel` is built from are declared; serde skips the
//! rest (descriptions, benchmarks, modalities, …) without allocating them. The
//! catalog runs to thousands of models, so every extra field costs memory on
//! each refresh.

use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Catalog {
    pub providers: HashMap<String, Provider>,
}

#[derive(Debug, Deserialize)]
pub struct Provider {
    pub name: String,
    pub models: HashMap<String, Model>,
}

#[derive(Debug, Deserialize)]
pub struct Model {
    pub name: String,
    pub limit: Limit,
    pub cost: Option<Cost>,
}

#[derive(Debug, Deserialize)]
pub struct Limit {
    pub context: i64,
}

/// Prices in USD per million tokens.
#[derive(Debug, Deserialize)]
pub struct Cost {
    pub input: f64,
    pub output: f64,
    pub cache_read: Option<f64>,
    pub cache_write: Option<f64>,
    pub tiers: Option<Vec<CostTier>>,
}

#[derive(Debug, Deserialize)]
pub struct CostTier {
    pub input: f64,
    pub output: f64,
    pub cache_read: Option<f64>,
    pub cache_write: Option<f64>,
    pub tier: TierCondition,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TierCondition {
    /// Applies once the prompt exceeds `size` tokens.
    Context { size: i64 },
    #[serde(other)]
    Unknown,
}
