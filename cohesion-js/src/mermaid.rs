//! Mermaid source to SVG with Pele (`vendor/pele`), in a QuickJS runtime of
//! its own with nothing mounted: Pele needs no DOM, and measures text from its
//! built-in metrics when it has no canvas.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Instant;

use cohesion_core::protocol::{MermaidRefusal, RenderedMermaid};
use rquickjs::{CatchResultExt, CaughtError, Context, Function, Object, Runtime};

use crate::sandbox::{INTERRUPTED, SandboxOptions};

const PELE: &str = include_str!("../vendor/pele/pele.global.js");

/// A render as plain values, and a refusal of Pele's as one too. Anything else
/// it throws, such as running out of memory, goes on up.
const RENDER: &str = r#"
globalThis.renderMermaid = (source, options) => {
  try {
    const { svg, width, height } = pele.render(source, options);
    return { svg, width, height };
  } catch (error) {
    if (error instanceof pele.PeleError) return { code: error.code, message: error.message };
    throw error;
  }
};
"#;

/// A render the runtime didn't come through intact: it stopped for time or
/// memory, or threw what Pele doesn't. The caller replies with `refused` and
/// starts another runtime.
#[derive(Debug)]
pub(crate) struct MermaidStop {
    pub(crate) refused: RenderedMermaid,
}

/// One runtime with Pele loaded. It lives on one worker's thread.
pub(crate) struct MermaidRenderer {
    runtime: Runtime,
    context: Context,
    options: SandboxOptions,
}

impl MermaidRenderer {
    /// Starts a runtime and loads Pele, which takes about 50 ms.
    pub(crate) fn new(options: SandboxOptions) -> Result<Self, MermaidStop> {
        let failed = |_| MermaidStop {
            refused: refused(MermaidRefusal::Failed, FAILED),
        };
        let runtime = Runtime::new().map_err(failed)?;
        runtime.set_memory_limit(options.memory_limit_bytes);
        let context = Context::full(&runtime).map_err(failed)?;
        context.with(|ctx| {
            ctx.eval::<(), _>(PELE)
                .and_then(|()| ctx.eval::<(), _>(RENDER))
                .catch(&ctx)
                .map_err(stop_for)
        })?;
        Ok(Self {
            runtime,
            context,
            options,
        })
    }

    /// Draws `source` for `width`, under the time limit. Pele's refusals are
    /// replies; `Err` means this runtime is done.
    pub(crate) fn render(
        &self,
        source: &str,
        width: Option<u32>,
    ) -> Result<RenderedMermaid, MermaidStop> {
        let deadline = Instant::now() + self.options.time_limit;
        self.runtime
            .set_interrupt_handler(Some(Box::new(move || Instant::now() > deadline)));
        self.context.with(|ctx| {
            let run = || -> rquickjs::Result<RenderedMermaid> {
                let options = Object::new(ctx.clone())?;
                if let Some(width) = width {
                    options.set("maxWidth", width)?;
                }
                // What the web passes; a client's `--pele-font` still wins.
                options.set("fontFamily", "system-ui")?;
                options.set("idPrefix", id_prefix(source, width))?;
                let render: Function = ctx.globals().get("renderMermaid")?;
                let out: Object = render.call((source, options))?;
                Ok(match out.get::<_, Option<String>>("svg")? {
                    Some(svg) => RenderedMermaid::Svg {
                        svg,
                        width: out.get("width")?,
                        height: out.get("height")?,
                    },
                    None => RenderedMermaid::Refused {
                        reason: refusal_for(&out.get::<_, String>("code")?),
                        message: out.get("message")?,
                    },
                })
            };
            run().catch(&ctx).map_err(stop_for)
        })
    }
}

/// Where this diagram's SVG ids start, since a page holds several diagrams in
/// one id namespace, and Pele only draws what needs an id (a Sankey's
/// gradients) when it has a prefix. The same text at the same width draws the
/// same thing, so a page showing it twice loses nothing by sharing the ids.
fn id_prefix(source: &str, width: Option<u32>) -> String {
    let mut hasher = DefaultHasher::new();
    (source, width).hash(&mut hasher);
    format!("d{:x}", hasher.finish())
}

fn refusal_for(code: &str) -> MermaidRefusal {
    match code {
        "unsupported-diagram" => MermaidRefusal::Unsupported,
        "syntax" => MermaidRefusal::Syntax,
        "semantic" => MermaidRefusal::Semantic,
        "limit" => MermaidRefusal::TooLarge,
        _ => MermaidRefusal::Failed,
    }
}

const FAILED: &str = "This diagram couldn’t be drawn.";

pub(crate) fn refused(reason: MermaidRefusal, message: &str) -> RenderedMermaid {
    RenderedMermaid::Refused {
        reason,
        message: message.to_string(),
    }
}

/// What a throw that isn't Pele's refusal means. Out of memory, QuickJS can
/// throw `null`, having no room left for an error.
fn stop_for(error: CaughtError<'_>) -> MermaidStop {
    let too_slow = || {
        refused(
            MermaidRefusal::TooSlow,
            "This diagram took too long to lay out.",
        )
    };
    let too_large = || {
        refused(
            MermaidRefusal::TooLarge,
            "This diagram is too large to draw.",
        )
    };
    let refused = match &error {
        CaughtError::Value(value) if value.is_null() => too_large(),
        CaughtError::Exception(exception) => {
            let name = exception.get::<_, String>("name").unwrap_or_default();
            let message = exception.message().unwrap_or_default();
            match (name.as_str(), message.as_str()) {
                thrown if thrown == INTERRUPTED => too_slow(),
                ("InternalError", "out of memory") | ("RangeError", _) => too_large(),
                _ => refused(MermaidRefusal::Failed, FAILED),
            }
        }
        _ => refused(MermaidRefusal::Failed, FAILED),
    };
    MermaidStop { refused }
}
