//! `Usage` and `ModelUsage` in scripts.

use cohesion_core::model::{IntelligenceUsage, ModelUsage};
use cohesion_macros::js_entity;

/// What model calls cost together, as `usage.total()` returns it.
#[js_entity(IntelligenceUsage, js = "Usage")]
impl JsUsage {}

/// What one model's calls cost together, as `usage.byModel()` returns it.
#[js_entity(ModelUsage)]
impl JsModelUsage {}
