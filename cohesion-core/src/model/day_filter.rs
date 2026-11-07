use cohesion_macros::{TsInterface, TsWire};
use serde::{Deserialize, Serialize};

use crate::ts_documentation::TsDocumentation;

/// Which days a date filter matches, each day as `"YYYY-MM-DD"`:
/// - a day: only that day.
/// - an array of days: any of them.
/// - a `DayRange`: the days between its bounds.
#[derive(Serialize, Deserialize, Clone, Debug, TsWire)]
pub enum DayFilter {
    #[serde(untagged)]
    Day(String),
    #[serde(untagged)]
    Days(Vec<String>),
    #[serde(untagged)]
    Range(DayRange),
}

impl TsDocumentation for DayFilter {
    const TS: &'static str = "/**
 * Which days a date filter matches, each day as `\"YYYY-MM-DD\"`: one day
 * (`\"2026-10-20\"`), any of several (`[\"2026-10-20\", \"2026-10-21\"]`), or
 * the days between two bounds (`{ after: \"2026-10-01\" }`).
 */
type DayFilter = string | string[] | DayRange;";
    const TS_BRIEF: &'static str = Self::TS;
}

/// The days strictly between two `"YYYY-MM-DD"` bounds; a bound left out leaves
/// that side open.
///
/// A task with no date never matches.
#[derive(Serialize, Deserialize, Clone, Debug, Default, TsWire, TsInterface)]
#[ts_wire(input)]
pub struct DayRange {
    /// Only days before this one.
    pub before: Option<String>,
    /// Only days after this one.
    pub after: Option<String>,
}
