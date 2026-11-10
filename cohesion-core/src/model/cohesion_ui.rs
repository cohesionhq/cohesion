//! CohesionUI: an interface a script describes and each client draws its own
//! way. A client picks the look; the meaning holds everywhere, so a heading
//! is a heading and an overview is a labelled value on every platform.

use cohesion_macros::{TsInterface, TsWire};
use serde::{Deserialize, Serialize};

use crate::ts_documentation::TsDocumentation;

/// An interface in CohesionUI, such as what a connector's
/// `renderResource(resource, kind)` returns to show a resource in place of
/// its raw contents: its blocks, top to bottom.
///
/// ```js
/// function renderResource(resource, kind) {
///   const row = JSON.parse(resource.contents);
///   return {
///     blocks: [
///       { kind: "heading", level: 1, text: resource.title },
///       { kind: "overview", title: "Status", child: { kind: "text", text: row.status } },
///       { kind: "checkbox", checked: row.done, text: "Done" },
///       { kind: "link", text: "Open in Notion", destination: row.url },
///     ],
///   };
/// }
/// ```
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire, TsInterface)]
pub struct Component {
    /// What the interface shows, top to bottom.
    pub blocks: Vec<Block>,
}

/// One element of a CohesionUI interface, told apart by `kind`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire, TsInterface)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Block {
    Text(TextBlock),
    Checkbox(CheckboxBlock),
    Overview(OverviewBlock),
    Heading(HeadingBlock),
    Link(LinkBlock),
}

/// `kind: "text"`: a paragraph of plain text.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire, TsInterface)]
pub struct TextBlock {
    /// The text, shown as written; line breaks are kept.
    pub text: String,
}

/// `kind: "checkbox"`: a yes or no, shown as a checkbox the user can't
/// change.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire, TsInterface)]
pub struct CheckboxBlock {
    /// Whether it is ticked.
    pub checked: bool,
    /// A label beside it, such as a to-do's text. Leave it out inside an
    /// overview, whose title labels it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// `kind: "overview"`: one labelled value, such as a status or an
/// assignee, as the details at the top of a task show them. Overviews in a
/// row line up as one list.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire, TsInterface)]
pub struct OverviewBlock {
    /// The label, such as `"Status"`.
    pub title: String,
    /// The value: usually a `text` block, or a `checkbox` or `link`.
    pub child: Box<Block>,
}

/// `kind: "heading"`: a title over what follows.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire, TsInterface)]
pub struct HeadingBlock {
    /// 1 is the largest, 3 the smallest.
    pub level: HeadingLevel,
    /// The heading's text.
    pub text: String,
}

/// `kind: "link"`: text that opens `destination`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TsWire, TsInterface)]
pub struct LinkBlock {
    /// What the link says.
    pub text: String,
    /// Where it goes: an `https://`, `http://` or `mailto:` URL. Any other
    /// address makes the whole interface invalid.
    pub destination: String,
}

/// How large a heading is: 1, 2 or 3, largest first.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TsWire)]
#[serde(try_from = "u8", into = "u8")]
#[ts_wire(type = "1 | 2 | 3")]
pub struct HeadingLevel(u8);

impl HeadingLevel {
    /// Its number, 1 to 3.
    pub fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for HeadingLevel {
    type Error = String;
    fn try_from(level: u8) -> Result<Self, Self::Error> {
        match level {
            1..=3 => Ok(Self(level)),
            _ => Err(format!("a heading's level is 1, 2 or 3, not {level}")),
        }
    }
}

impl From<HeadingLevel> for u8 {
    fn from(level: HeadingLevel) -> Self {
        level.0
    }
}

impl TsDocumentation for HeadingLevel {
    const TS: &'static str = "/** How large a heading is: 1, 2 or 3, largest first. */\n\
        type HeadingLevel = 1 | 2 | 3;";
    const TS_BRIEF: &'static str = Self::TS;
}

/// The schemes a link may open.
const LINK_SCHEMES: [&str; 3] = ["https://", "http://", "mailto:"];

/// The most blocks an interface may hold in all, counting those inside an
/// overview.
pub const MAX_BLOCKS: usize = 2_000;
/// The deepest blocks may nest: a top-level block is 1, an overview's
/// child 2.
pub const MAX_DEPTH: usize = 8;
/// The most characters one piece of text, title or link address may hold.
pub const MAX_TEXT_CHARS: usize = 100_000;

impl Component {
    /// Reads what a script returned as an interface, or says why it isn't
    /// one: the wrong shape, more than [`MAX_BLOCKS`] blocks, nesting past
    /// [`MAX_DEPTH`], a string over [`MAX_TEXT_CHARS`], or a link to
    /// anything but a web or mail address.
    pub fn from_script(value: serde_json::Value) -> Result<Self, String> {
        // Refuse a deep value before reading it, which recurses as deep.
        if !within_depth(&value, MAX_DEPTH * 2 + 2) {
            return Err(format!("blocks may nest at most {MAX_DEPTH} deep"));
        }
        let component: Self = serde_json::from_value(value).map_err(|e| e.to_string())?;
        let mut blocks = 0;
        component
            .blocks
            .iter()
            .try_for_each(|block| block.check(1, &mut blocks))?;
        Ok(component)
    }
}

/// Whether `value` nests no deeper than `limit`, looking no deeper than that.
fn within_depth(value: &serde_json::Value, limit: usize) -> bool {
    match value {
        serde_json::Value::Array(items) => {
            limit > 0 && items.iter().all(|v| within_depth(v, limit - 1))
        }
        serde_json::Value::Object(fields) => {
            limit > 0 && fields.values().all(|v| within_depth(v, limit - 1))
        }
        _ => true,
    }
}

fn check_text(text: &str) -> Result<(), String> {
    if text.chars().count() > MAX_TEXT_CHARS {
        return Err(format!(
            "a block's text may hold at most {MAX_TEXT_CHARS} characters"
        ));
    }
    Ok(())
}

impl Block {
    /// Whether this block, and any it holds, may be shown. `depth` is its
    /// nesting; `count` the blocks seen so far in the interface.
    fn check(&self, depth: usize, count: &mut usize) -> Result<(), String> {
        *count += 1;
        if *count > MAX_BLOCKS {
            return Err(format!("an interface may hold at most {MAX_BLOCKS} blocks"));
        }
        if depth > MAX_DEPTH {
            return Err(format!("blocks may nest at most {MAX_DEPTH} deep"));
        }
        match self {
            Self::Overview(overview) => {
                check_text(&overview.title)?;
                overview.child.check(depth + 1, count)
            }
            Self::Link(link) => {
                check_text(&link.text)?;
                check_text(&link.destination)?;
                let destination = link.destination.to_ascii_lowercase();
                if LINK_SCHEMES.iter().any(|s| destination.starts_with(s)) {
                    Ok(())
                } else {
                    Err(format!(
                        "a link's destination must be an https, http or mailto URL, not {:?}",
                        link.destination
                    ))
                }
            }
            Self::Text(block) => check_text(&block.text),
            Self::Heading(block) => check_text(&block.text),
            Self::Checkbox(block) => block.text.as_deref().map_or(Ok(()), check_text),
        }
    }
}

#[cfg(test)]
mod tests;
