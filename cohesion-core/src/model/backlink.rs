//! Backlinks: which items link to which, so a client can ask what links to
//! an item. They are kept from the `cohesion://` links in an item's text,
//! by the rule here: an item with text says what it is and what its text
//! is ([`LinkSource`]), and the database keeps one [`Backlink`] per item its
//! text links to, rewritten whenever the item is written. The item itself
//! never deals with them.

use std::collections::HashSet;

use cohesion_macros::Model;
use serde::{Deserialize, Serialize};

use super::text_section::is_fence;
use super::{Document, ItemLink, LinkKind, LinkTarget};

/// One link from an item's text to an item: the source links to the target,
/// as `link`. Kept for each item the source's text links to, once however
/// often it does, never to the source itself.
#[derive(Clone, Debug, Serialize, Deserialize, Model)]
#[model(table = "backlinks", backup = Included)]
pub struct Backlink {
    pub id: i64,
    /// The item whose text holds the link: only a document so far.
    pub source_kind: LinkKind,
    pub source_id: i64,
    /// The item the link names.
    pub target_kind: LinkKind,
    pub target_id: i64,
    /// The link as the source's text first writes it, `cohesion://task/3/42`
    /// or, without its workspace, `cohesion://task/42`.
    pub link: String,
}

impl Backlink {
    /// The item whose text holds the link.
    pub fn source(&self) -> LinkTarget {
        LinkTarget {
            kind: self.source_kind,
            id: self.source_id,
        }
    }

    /// The item the link names.
    pub fn target(&self) -> LinkTarget {
        LinkTarget {
            kind: self.target_kind,
            id: self.target_id,
        }
    }
}

/// An item whose text can link to others: what backlinks are kept from.
pub trait LinkSource {
    /// The item, as a link names it.
    fn link_target(&self) -> LinkTarget;
    /// The text whose links it makes; `None` once the item is deleted, when
    /// it links to nothing.
    fn linking_text(&self) -> Option<&str>;
}

impl LinkSource for Document {
    fn link_target(&self) -> LinkTarget {
        LinkTarget {
            kind: LinkKind::Document,
            id: self.id,
        }
    }

    fn linking_text(&self) -> Option<&str> {
        (!self.deleted).then_some(self.contents.as_str())
    }
}

/// A link an item's text makes to another item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutgoingLink {
    pub target: LinkTarget,
    /// The link as the text first writes it.
    pub link: String,
}

/// The items `text`, `source`'s own, links to ([`item_links`]): each once,
/// in the order the text first links to it, and never `source` itself.
pub fn outgoing_links(source: LinkTarget, text: &str) -> Vec<OutgoingLink> {
    let mut links: Vec<OutgoingLink> = Vec::new();
    let mut known = HashSet::from([source]);
    for (link, written) in item_links(text) {
        let target = link.target();
        if known.insert(target) {
            links.push(OutgoingLink {
                target,
                link: written.to_string(),
            });
        }
    }
    links
}

/// Every `cohesion://` link to an item in Markdown `text`, with how it is
/// written, in order: an inline link's target (`[label](cohesion://…)`, a
/// title after it or not), an autolink (`<cohesion://…>`) and a reference
/// definition (`[label]: cohesion://…`), whether alone on its line (an
/// embed) or in a sentence. A link in a fenced code block or a code span is
/// code, not a link, and text that isn't a link to an item
/// (`cohesion://task/x`) is skipped.
pub fn item_links(text: &str) -> Vec<(ItemLink, &str)> {
    const SCHEME: &str = "cohesion://";
    let mut links = Vec::new();
    let mut fenced = false;
    for line in text.lines() {
        if is_fence(line) {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        // Spans and matches both come in order, so each span is passed once: a
        // line of many spans and many links is read in one pass, not one per link.
        let mut code = code_spans(line).into_iter().peekable();
        for (at, _) in line.match_indices(SCHEME) {
            while code.next_if(|span| span.end <= at).is_some() {}
            if code.peek().is_some_and(|span| span.contains(&at)) {
                continue;
            }
            let before = &line[..at];
            let linked = before.ends_with("](")
                || before.ends_with('<')
                || before.trim_end().ends_with("]:");
            if !linked {
                continue;
            }
            let rest = &line[at..];
            let end = rest
                .find(|c: char| c.is_whitespace() || matches!(c, ')' | '>' | '"' | '\''))
                .unwrap_or(rest.len());
            let written = &rest[..end];
            if let Ok(link) = written.parse() {
                links.push((link, written));
            }
        }
    }
    links
}

/// The byte ranges of `line`'s code spans: a run of backticks to the next
/// run of as many. A run with no match is literal backticks.
fn code_spans(line: &str) -> Vec<std::ops::Range<usize>> {
    let bytes = line.as_bytes();
    let run_end = |from: usize| {
        let mut end = from;
        while end < bytes.len() && bytes[end] == b'`' {
            end += 1;
        }
        end
    };
    let mut spans = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] != b'`' {
            at += 1;
            continue;
        }
        let open = run_end(at);
        let width = open - at;
        let mut probe = open;
        let mut close = None;
        while probe < bytes.len() {
            if bytes[probe] == b'`' {
                let end = run_end(probe);
                if end - probe == width {
                    close = Some(end);
                    break;
                }
                probe = end;
            } else {
                probe += 1;
            }
        }
        match close {
            Some(end) => {
                spans.push(at..end);
                at = end;
            }
            None => at = open,
        }
    }
    spans
}

#[cfg(test)]
mod tests;
