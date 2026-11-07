//! Drawing a Mermaid diagram, which happens on the script engine's own workers.

use cohesion_core::protocol::{
    Local, MermaidFormat, RenderMermaidRequest, RenderedMermaid, Route, ServerError, ShuttingDown,
};

use crate::cohesion_context::CohesionContext;

impl Route<Local, RenderMermaidRequest> for CohesionContext {
    async fn route(&self, request: RenderMermaidRequest) -> Result<RenderedMermaid, ServerError> {
        let render = match request.format {
            MermaidFormat::Svg => self
                .script_engine
                .render_mermaid(request.source, request.width),
        };
        // A diagram is only worth its reply, so a shutdown doesn't wait for one
        // or for those queued behind it; a job whose reply is gone doesn't run.
        tokio::select! {
            rendered = render => Ok(rendered),
            () = self.work.closed() => Err(ShuttingDown.into()),
        }
    }
}
