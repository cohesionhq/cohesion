//! Every rule Coherence decides, in order, and the hints its finder weighs,
//! written as JSON in `coherence/rules/*.json` (read in file-name order;
//! when two rules pick the same element, the first decides it) and
//! `coherence/hints.json`, and built into `static` slices of [`Rule`] and
//! [`Hint`] by `build.rs`. A new case is one more entry in the file it
//! belongs in, `{ "about", "trigger", "action" }` with the trigger and
//! action spelled as [`Trigger`] and the [`Rule`] constructors in snake
//! case; a case no trigger can say is a new [`Trigger`] variant here and in
//! `build.rs`. `cohesion-internet-harness` shows what a change does to a
//! real page or feed.

use super::rule::{Address, Embed, Hint, Name, Phrase, Rule, Trigger};

include!(concat!(env!("OUT_DIR"), "/coherence_rules.rs"));

/// Words that make a `class` or `id` name say what state the page is in
/// rather than what the element is, opening it (`has-sidebar`,
/// `is-paywalled`, `no-comments`) or closing it (`comments-open`). Rules
/// going by names read nothing into such a name, so a layout's
/// `has-sidebar` container keeps the article it holds.
pub(crate) static STATE_WORDS: &[&str] = &[
    "has", "is", "no", "not", "with", "without", "show", "hide", "can", "open", "closed",
    "enabled", "disabled", "active", "inactive", "visible", "hidden", "on", "off",
];
