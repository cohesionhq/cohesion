//! Script requests: running a script and the API documentation it runs against.

use cohesion_core::log::Module;
use cohesion_core::log_error;
use cohesion_core::protocol::{
    DocumentationRequest, DocumentationResponse, Route, RunScriptRequest, Script, ScriptResult,
    ServerError,
};
use cohesion_db::ListServicesCommand;
use cohesion_js::script_api::TS_DECLARATIONS;
use cohesion_js::{ScriptError, ScriptOutput, service_declarations};

use crate::CohesionContext;

/// Evaluates the script on the engine; an exception is the caller's error.
impl Route<Script, RunScriptRequest> for CohesionContext {
    async fn route(&self, request: RunScriptRequest) -> Result<ScriptResult, ServerError> {
        match self.script_engine.general_purpose_run(request.script).await {
            Ok(ScriptOutput(json)) => Ok(ScriptResult(json)),
            Err(ScriptError::Exception { message, .. }) => Err(ServerError::bad_request(message)),
            Err(error) => {
                log_error!(
                    Module::Script,
                    "JavaScript runtime encountered an unrecoverable error: {error}"
                );
                Err(ServerError::internal())
            }
        }
    }
}
/// The script API's `.d.ts`, then the installed services merged into its
/// `ServiceMap`, which change as extensions are installed.
impl Route<Script, DocumentationRequest> for CohesionContext {
    async fn route(
        &self,
        _request: DocumentationRequest,
    ) -> Result<<DocumentationRequest as cohesion_core::protocol::Call>::Reply, ServerError> {
        let services = self.database.send(ListServicesCommand {}).await?;
        let mut documentation = TS_DECLARATIONS.to_string();
        let services = service_declarations(&services);
        if !services.is_empty() {
            documentation.push('\n');
            documentation.push_str(&services);
            documentation.push('\n');
        }
        Ok(DocumentationResponse { documentation })
    }
}
