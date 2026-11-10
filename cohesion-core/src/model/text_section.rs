//! Markdown split where it shows items as cards: a `cohesion://` link alone
//! on its line, outside code, is an embed; everything else stays Markdown,
//! its links links. Every client splits by this rule, so a link shows as a
//! card in each of them or in none.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

use super::{Embed, ItemLink};

/// One piece of Markdown text as `resolveEmbeds` returns it, tagged by
/// `kind`, in the order the text holds them.
#[derive(Clone, Serialize, Deserialize, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TextSection {
    /// Markdown to render as it is; a link in it stays a link.
    Markdown { text: String },
    /// A `cohesion://` link alone on its line, outside code: the item it
    /// names, shown as a card.
    Embed {
        /// The link as the text has it.
        link: String,
        /// The link's text, to show while it resolves or once it can't.
        label: String,
        /// What the link names, as `resolveLink` returns it; `null` when it
        /// names an item deleted or never made.
        embed: Option<Box<Embed>>,
    },
}

/// A piece of text before its links are resolved.
#[derive(Debug, PartialEq, Eq)]
pub enum TextPiece {
    Markdown(String),
    Embed {
        link: ItemLink,
        href: String,
        label: String,
    },
}

/// `text` split into its Markdown and its embeds: each line that is only a
/// `[label](cohesion://…)` link to an item, outside a fenced or indented
/// code block, is an embed. Blank lines around an embed go with it, and
/// Markdown of only blank lines is dropped.
pub fn text_pieces(text: &str) -> Vec<TextPiece> {
    let mut pieces = Vec::new();
    let mut markdown: Vec<&str> = Vec::new();
    let mut fenced = false;
    for line in text.lines() {
        let fence = line.trim_start();
        if fence.starts_with("```") || fence.starts_with("~~~") {
            fenced = !fenced;
        }
        match (!fenced).then(|| embed_line(line)).flatten() {
            Some((label, href, link)) => {
                push_markdown(&mut pieces, &mut markdown);
                pieces.push(TextPiece::Embed {
                    link,
                    href: href.to_string(),
                    label: label.to_string(),
                });
            }
            None => markdown.push(line),
        }
    }
    push_markdown(&mut pieces, &mut markdown);
    pieces
}

/// Ends the Markdown gathered so far, without its blank lines at either end.
fn push_markdown(pieces: &mut Vec<TextPiece>, lines: &mut Vec<&str>) {
    let start = lines.iter().position(|l| !l.trim().is_empty());
    let end = lines.iter().rposition(|l| !l.trim().is_empty());
    if let (Some(start), Some(end)) = (start, end) {
        pieces.push(TextPiece::Markdown(lines[start..=end].join("\n")));
    }
    lines.clear();
}

/// The label, href and link of `line` when it is only a link to an item
/// (spaces around it aside, but not a code block's indent of four).
fn embed_line(line: &str) -> Option<(&str, &str, ItemLink)> {
    let indent = &line[..line.len() - line.trim_start().len()];
    if indent.contains('\t') || indent.len() >= 4 {
        return None;
    }
    let link = line.trim().strip_prefix('[')?.strip_suffix(')')?;
    let middle = link.rfind("](")?;
    let (label, href) = (&link[..middle], &link[middle + 2..]);
    Some((label, href, href.parse().ok()?))
}

#[cfg(test)]
mod tests;
