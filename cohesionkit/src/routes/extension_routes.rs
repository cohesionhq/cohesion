//! Extension requests: installing extensions, granting their permissions and
//! hooks, finding new ones and updates through registries, and calling a
//! `service` extension's `serviceRequest` as the client asking. The extension manager does the
//! work that touches an extension's scope.

use std::time::Duration;

use cohesion_core::log::Module;
use cohesion_core::protocol::{
    AcceptedResponse, CallExtensionRequest, CheckExtensionUpdatesRequest, ExtensionCallResult,
    ExtensionPreview, ExtensionUpdate, FromClient, ListExtensionUpdatesRequest, Local,
    PreviewExtensionRequest, PreviewExtensionScriptRequest, RegisterExtensionRequest,
    RegistrySection, Route, SearchExtensionRegistriesRequest, ServerError, SetExtensionHookRequest,
    SetExtensionPermissionRequest, UninstallExtensionRequest,
};
use cohesion_core::{log_error, log_info};
use cohesion_db::{CommandError, GetExtensionCommand, GetExtensionQuery};

use crate::client_request::ClientRequest;
use crate::cohesion_context::CohesionContext;
use crate::extension_icon::accepted_icon;
use crate::extension_manager::{
    ExtensionError, ExtensionFn, RegistryResultsFn, ServiceCaller, script_fault,
};

impl Route<Local, RegisterExtensionRequest> for CohesionContext {
    async fn route(
        &self,
        request: RegisterExtensionRequest,
    ) -> Result<AcceptedResponse, ServerError> {
        let extension = self
            .extensions
            .install(request.script, request.allow_overwrite)
            .await?;
        log_info!(
            Module::Extensions,
            "Registered extension '{}' with version '{}'.",
            extension.name,
            extension.version
        );
        Ok(AcceptedResponse {})
    }
}

impl Route<Local, SetExtensionPermissionRequest> for CohesionContext {
    async fn route(
        &self,
        request: SetExtensionPermissionRequest,
    ) -> Result<AcceptedResponse, ServerError> {
        self.extensions.set_permission(request).await?;
        Ok(AcceptedResponse {})
    }
}

impl Route<Local, SetExtensionHookRequest> for CohesionContext {
    async fn route(
        &self,
        request: SetExtensionHookRequest,
    ) -> Result<AcceptedResponse, ServerError> {
        self.extensions.set_hook(request).await?;
        Ok(AcceptedResponse {})
    }
}

impl Route<Local, UninstallExtensionRequest> for CohesionContext {
    async fn route(
        &self,
        request: UninstallExtensionRequest,
    ) -> Result<AcceptedResponse, ServerError> {
        self.extensions.uninstall(request.extension_id).await?;
        Ok(AcceptedResponse {})
    }
}

impl Route<Local, SearchExtensionRegistriesRequest> for CohesionContext {
    async fn route(
        &self,
        request: SearchExtensionRegistriesRequest,
    ) -> Result<Vec<RegistrySection>, ServerError> {
        self.extensions
            .call_each(RegistryResultsFn::REQUIRES, "Registry search", |registry| {
                let extensions = self.extensions.clone();
                let query = request.query.clone();
                async move {
                    let searched = extensions
                        .call(registry.id, RegistryResultsFn { query })
                        .await;
                    let (results, error) = match searched {
                        Ok(mut results) => {
                            for result in &mut results {
                                result.manifest.icon = accepted_icon(result.manifest.icon.take());
                            }
                            (results, None)
                        }
                        Err(e) => (Vec::new(), Some(registry_error(&registry.name, e))),
                    };
                    RegistrySection {
                        extension_id: registry.id,
                        name: registry.name,
                        results,
                        error,
                    }
                }
            })
            .await
    }
}

impl Route<Local, ListExtensionUpdatesRequest> for CohesionContext {
    async fn route(
        &self,
        _: ListExtensionUpdatesRequest,
    ) -> Result<Vec<ExtensionUpdate>, ServerError> {
        self.extensions.updates().await
    }
}

impl Route<Local, CheckExtensionUpdatesRequest> for CohesionContext {
    async fn route(
        &self,
        _: CheckExtensionUpdatesRequest,
    ) -> Result<Vec<ExtensionUpdate>, ServerError> {
        self.extensions.check_updates().await
    }
}

impl Route<Local, PreviewExtensionRequest> for CohesionContext {
    async fn route(
        &self,
        request: PreviewExtensionRequest,
    ) -> Result<ExtensionPreview, ServerError> {
        let invalid_url = || {
            ServerError::validation(
                Some("source_url".to_string()),
                "Extensions can only be downloaded from an http or https URL.",
            )
        };
        let url = reqwest::Url::parse(&request.source_url).map_err(|_| invalid_url())?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(invalid_url());
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent("cohesion")
            .build()
            .map_err(|e| {
                log_error!(Module::Extensions, "Building an HTTP client failed: {e}");
                ServerError::internal()
            })?;
        let download_failed = |e: reqwest::Error| {
            ServerError::upstream(format!("Couldn’t download the extension: {e}"))
        };
        let response = client.get(url).send().await.map_err(download_failed)?;
        let status = response.status();
        if !status.is_success() {
            return Err(ServerError::upstream(format!(
                "Couldn’t download the extension. The server answered {status}."
            )));
        }
        let script = response.text().await.map_err(download_failed)?;
        self.preview_extension(script).await
    }
}

impl Route<Local, PreviewExtensionScriptRequest> for CohesionContext {
    async fn route(
        &self,
        request: PreviewExtensionScriptRequest,
    ) -> Result<ExtensionPreview, ServerError> {
        self.preview_extension(request.script).await
    }
}

/// The longest extension identifier `callExtension` takes, in characters.
const MAX_CALL_IDENTIFIER: usize = 256;

/// Calls the service as the client asking, which `serviceRequest` is told.
impl Route<FromClient, CallExtensionRequest> for ClientRequest<'_> {
    async fn route(
        &self,
        request: CallExtensionRequest,
    ) -> Result<ExtensionCallResult, ServerError> {
        let context = self.context;
        let CallExtensionRequest {
            identifier,
            name,
            args,
        } = request;
        if identifier.is_empty() || identifier.chars().count() > MAX_CALL_IDENTIFIER {
            return Err(ServerError::validation(
                Some("identifier".to_string()),
                format!("An extension identifier must be 1 to {MAX_CALL_IDENTIFIER} characters."),
            ));
        }
        let extension_id = context
            .database
            .send(GetExtensionCommand {
                query: GetExtensionQuery::ScriptIdentifier(identifier),
            })
            .await?
            .id;
        let caller = ServiceCaller::Client {
            client_id: self.client.id,
        };
        let result = context
            .extensions
            .call_service(extension_id, caller, name, args)
            .await?;
        Ok(ExtensionCallResult { result })
    }
}

impl CohesionContext {
    /// Reads `script`'s manifest and finds the installed extension it would
    /// replace.
    async fn preview_extension(&self, script: String) -> Result<ExtensionPreview, ServerError> {
        let manifest = self.extensions.read_manifest(script.clone()).await?;
        let installed = match self
            .database
            .send(GetExtensionCommand {
                query: GetExtensionQuery::ScriptIdentifier(manifest.identifier.clone()),
            })
            .await
        {
            Ok(extension) => Some(extension.into()),
            Err(CommandError::NotFound(_)) => None,
            Err(e) => return Err(e.into()),
        };
        Ok(ExtensionPreview {
            script,
            manifest: Box::new(manifest),
            installed,
        })
    }
}

/// What a registry section shows for a failed search.
fn registry_error(name: &str, error: ExtensionError) -> String {
    match error {
        ExtensionError::Script(error) => {
            let function = RegistryResultsFn::NAME;
            script_fault(name, function, error, |message| {
                format!("{function} didn't return RegistryResult[]: {message}")
            })
        }
        .unwrap_or_else(|| "The registry failed to run.".to_string()),
        error => ServerError::from(error).message,
    }
}
