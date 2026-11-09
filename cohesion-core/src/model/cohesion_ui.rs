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

impl Component {
    /// Reads what a script returned as an interface, or says why it isn't
    /// one: the wrong shape, or a link to anything but a web or mail
    /// address.
    pub fn from_script(value: serde_json::Value) -> Result<Self, String> {
        let component: Self = serde_json::from_value(value).map_err(|e| e.to_string())?;
        component.blocks.iter().try_for_each(Block::check)?;
        Ok(component)
    }
}

impl Block {
    /// Whether this block, and any it holds, may be shown.
    fn check(&self) -> Result<(), String> {
        match self {
            Self::Overview(overview) => overview.child.check(),
            Self::Link(link) => {
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
            Self::Text(_) | Self::Checkbox(_) | Self::Heading(_) => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests;
