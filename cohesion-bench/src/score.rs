//! The bench's three scores, each from 0 to 100, higher better. They are
//! worked out from fixed weights and references rather than from the other
//! models in a run, so a model's scores mean the same in every run and can
//! be compared across runs on the same cases.
//!
//! - **Accuracy**: the runs that passed, each weighted by its case's
//!   difficulty (easy 1, medium 2, hard 3), as a percentage of the most
//!   there was to get. Runs the bench itself couldn't carry out don't
//!   count.
//! - **Efficiency**: over the runs that passed, how lightly each did it:
//!   the weighted geometric mean of its tokens (0.4), model calls (0.3) and
//!   wall time (0.3), each scored `reference / (reference + used)`, the
//!   references set by difficulty. A run that used exactly the references
//!   scores 50, half of them about 67, and nothing at all 100. Cost is left
//!   out, since it is the provider's price as much as the model's work;
//!   the report shows it apart.
//! - **Overall**: `accuracy^0.7 × efficiency^0.3`, on the same 0–100
//!   scale, so it rises with either and accuracy counts for more. A model
//!   that passes nothing scores 0.

use crate::case::Difficulty;

/// How much a case of `difficulty` counts toward accuracy.
pub fn weight(difficulty: Difficulty) -> f64 {
    match difficulty {
        Difficulty::Easy => 1.0,
        Difficulty::Medium => 2.0,
        Difficulty::Hard => 3.0,
    }
}

/// What a run of a case of some difficulty may use and still score 50 on
/// each part of efficiency. One model call sends about 15,000 tokens of
/// tools and system prompt, so the token references follow the calls'.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reference {
    pub tokens: f64,
    pub model_calls: f64,
    pub seconds: f64,
}

pub fn reference(difficulty: Difficulty) -> Reference {
    match difficulty {
        Difficulty::Easy => Reference {
            tokens: 60_000.0,
            model_calls: 4.0,
            seconds: 45.0,
        },
        Difficulty::Medium => Reference {
            tokens: 120_000.0,
            model_calls: 8.0,
            seconds: 90.0,
        },
        Difficulty::Hard => Reference {
            tokens: 240_000.0,
            model_calls: 16.0,
            seconds: 180.0,
        },
    }
}

const TOKENS_WEIGHT: f64 = 0.4;
const MODEL_CALLS_WEIGHT: f64 = 0.3;
const SECONDS_WEIGHT: f64 = 0.3;

/// How efficiently one run did its case, from 0 to 100.
pub fn run_efficiency(difficulty: Difficulty, tokens: f64, model_calls: f64, seconds: f64) -> f64 {
    let reference = reference(difficulty);
    let part = |reference: f64, used: f64| reference / (reference + used.max(0.0));
    100.0
        * part(reference.tokens, tokens).powf(TOKENS_WEIGHT)
        * part(reference.model_calls, model_calls).powf(MODEL_CALLS_WEIGHT)
        * part(reference.seconds, seconds).powf(SECONDS_WEIGHT)
}

const ACCURACY_EXPONENT: f64 = 0.7;

/// The overall score from accuracy and efficiency, each from 0 to 100.
pub fn overall(accuracy: f64, efficiency: f64) -> f64 {
    100.0
        * (accuracy / 100.0).max(0.0).powf(ACCURACY_EXPONENT)
        * (efficiency / 100.0).max(0.0).powf(1.0 - ACCURACY_EXPONENT)
}

#[cfg(test)]
mod tests;
