//! Drawing a Mermaid diagram on the daemon, so a client needs no renderer.

use cohesion_macros::TsWire;
use serde::{Deserialize, Serialize};

/// What a diagram is drawn as.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TsWire)]
#[serde(rename_all = "camelCase")]
pub enum MermaidFormat {
    /// An SVG that takes its colours from `--pele-*` CSS variables and has no
    /// background, so a client themes it with no new request, and that scales
    /// to its container, so a resize needs none either.
    Svg,
}

/// Draws a Mermaid diagram.
#[derive(Serialize, Deserialize, Debug, Clone, TsWire)]
#[ts_wire(input)]
pub struct RenderMermaidRequest {
    /// The diagram's Mermaid source: a fenced block's text, without the fence.
    pub source: String,
    /// The width to lay out for, in points. A new width is a new layout, so a
    /// client asks once per width it shows rather than as it resizes. Left
    /// out, the diagram takes the size its content wants.
    pub width: Option<u32>,
    pub format: MermaidFormat,
}

/// Why a diagram wasn't drawn. A client shows the source as code either way.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TsWire)]
#[serde(rename_all = "camelCase")]
pub enum MermaidRefusal {
    /// Not a kind of diagram the renderer knows, or not a diagram at all.
    Unsupported,
    /// The source doesn't parse: mistyped, or still streaming in.
    Syntax,
    /// It parses, but what it says makes no sense.
    Semantic,
    /// Over the renderer's limits on source, edges or output, or out of memory.
    TooLarge,
    /// Its layout ran past the time limit.
    TooSlow,
    /// Too many diagrams were already waiting. Asking again later may work.
    Busy,
    /// The renderer failed. Not the diagram's fault.
    Failed,
}

/// A diagram as drawn, or why it wasn't.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TsWire)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RenderedMermaid {
    Svg {
        svg: String,
        /// The size it was laid out at, which its `viewBox` scales from.
        width: f64,
        height: f64,
    },
    Refused {
        reason: MermaidRefusal,
        /// Says what's wrong, in the renderer's words for a bad source.
        message: String,
    },
}
