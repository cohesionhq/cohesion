//! Requests that call extensions' connector functions: describing their
//! services and adding and removing accounts (`oauth`), and browsing and importing items
//! and refreshing resources (`connector`).

use std::collections::HashSet;

use cohesion_core::model::{
    Component, ConnectorResourceFilters, ExtensionPermissionKind, OrganizationUnit, Resource,
};
use cohesion_core::protocol::{
    BeginConnectorAuthenticationRequest, CompleteConnectorAuthenticationRequest,
    ConnectorAuthentication, ConnectorService, DeleteConnectorAccountRequest,
    ImportResourceRequest, ImportableResource, ListConnectorOrganizationUnitsRequest,
    ListConnectorServicesRequest, Local, RedactedConnectorAccount, RefreshResourcesRequest,
    RenderResourceRequest, ResourceRefreshSummary, Route, SearchConnectorResourcesRequest,
    ServerError,
};
use cohesion_db::{
    DeleteConnectorAccountCommand, GetAccountConnectorCommand, GetResourceCommand,
    GetWorkspaceCommand, ImportResourceCommand, ListAccountResourcesCommand,
};

use crate::cohesion_context::CohesionContext;
use crate::connectors::{
    AuthenticationUrlFn, ServiceDescriptionFn, account_items, account_organization_units,
    account_removed, call_connector, complete_connector_authentication, fetch_resource,
    refresh_resources, render_resource,
};
use crate::extension_manager::ExtensionFn;

impl Route<Local, ListConnectorServicesRequest> for CohesionContext {
    async fn route(
        &self,
        _: ListConnectorServicesRequest,
    ) -> Result<Vec<ConnectorService>, ServerError> {
        self.extensions
            .call_each(
                ExtensionPermissionKind::OAuth,
                "Describing a connector",
                |extension| {
                    let ctx = self.clone();
                    async move {
                        match call_connector(&ctx.extensions, &extension, ServiceDescriptionFn)
                            .await
                        {
                            Ok(service) => ConnectorService {
                                extension_id: extension.id,
                                name: extension.name,
                                title: service.title,
                                unit: service.unit,
                                icon: extension.icon,
                                error: None,
                                sign_in_hint: service.sign_in_hint,
                            },
                            Err(e) => ConnectorService {
                                extension_id: extension.id,
                                title: extension.name.clone(),
                                name: extension.name,
                                unit: String::new(),
                                icon: extension.icon,
                                error: Some(e.message),
                                sign_in_hint: None,
                            },
                        }
                    }
                },
            )
            .await
    }
}

impl Route<Local, BeginConnectorAuthenticationRequest> for CohesionContext {
    async fn route(
        &self,
        request: BeginConnectorAuthenticationRequest,
    ) -> Result<ConnectorAuthentication, ServerError> {
        let extension = self
            .extensions
            .granted(request.extension_id, AuthenticationUrlFn::REQUIRES)
            .await?;
        let pkce = self.sign_ins.begin(extension.id);
        Ok(ConnectorAuthentication {
            url: call_connector(&self.extensions, &extension, AuthenticationUrlFn { pkce }).await?,
        })
    }
}

impl Route<Local, CompleteConnectorAuthenticationRequest> for CohesionContext {
    async fn route(
        &self,
        request: CompleteConnectorAuthenticationRequest,
    ) -> Result<RedactedConnectorAccount, ServerError> {
        complete_connector_authentication(
            &self.extensions,
            &self.database,
            request.extension_id,
            request.code,
            self.sign_ins.take(request.extension_id),
        )
        .await
    }
}

impl Route<Local, DeleteConnectorAccountRequest> for CohesionContext {
    async fn route(
        &self,
        request: DeleteConnectorAccountRequest,
    ) -> Result<RedactedConnectorAccount, ServerError> {
        let account = self
            .database
            .send(DeleteConnectorAccountCommand {
                account_id: request.account_id,
            })
            .await?;
        account_removed(&self.extensions, &account).await;
        Ok(account)
    }
}

impl Route<Local, ListConnectorOrganizationUnitsRequest> for CohesionContext {
    async fn route(
        &self,
        request: ListConnectorOrganizationUnitsRequest,
    ) -> Result<Vec<OrganizationUnit>, ServerError> {
        account_organization_units(&self.extensions, &self.database, request.account_id).await
    }
}

impl Route<Local, SearchConnectorResourcesRequest> for CohesionContext {
    async fn route(
        &self,
        request: SearchConnectorResourcesRequest,
    ) -> Result<Vec<ImportableResource>, ServerError> {
        let connector = self
            .database
            .send(GetAccountConnectorCommand {
                account_id: request.account_id,
            })
            .await?;
        let Some(kind) = connector.kinds.iter().find(|k| k.id == request.kind_id) else {
            return Err(ServerError::validation(
                Some("kind_id".to_string()),
                format!("{} has no such kind.", connector.title),
            ));
        };
        let filters = ConnectorResourceFilters {
            organization_unit_id: request.organization_unit_id,
            search: request.search,
        };
        let previews = account_items(
            &self.extensions,
            &self.database,
            request.account_id,
            filters,
        )
        .await?;
        let imported: HashSet<String> = self
            .database
            .send(ListAccountResourcesCommand {
                account_id: request.account_id,
                workspace_id: Some(request.workspace_id),
            })
            .await?
            .into_iter()
            .map(|resource| resource.identifier)
            .collect();
        if let Some(stray) = previews.iter().find(|preview| {
            !connector
                .kinds
                .iter()
                .any(|k| k.identifier == preview.kind_identifier)
        }) {
            return Err(ServerError::upstream(format!(
                "{} listed an item of kind \"{}\", which it doesn't declare.",
                connector.title, stray.kind_identifier
            )));
        }
        Ok(previews
            .into_iter()
            .filter(|preview| preview.kind_identifier == kind.identifier)
            .map(|preview| ImportableResource {
                imported: imported.contains(&preview.identifier),
                identifier: preview.identifier,
                title: preview.title,
                // An extension that predates `null` reports 0 for "unknown".
                last_updated: preview.last_updated.filter(|seconds| *seconds > 0),
            })
            .collect())
    }
}

impl Route<Local, ImportResourceRequest> for CohesionContext {
    async fn route(&self, request: ImportResourceRequest) -> Result<Resource, ServerError> {
        let fetched = fetch_resource(
            &self.extensions,
            &self.database,
            request.account_id,
            request.identifier,
        )
        .await?;
        Ok(self
            .database
            .send(ImportResourceCommand {
                workspace_id: request.workspace_id,
                account_id: request.account_id,
                resource: fetched,
            })
            .await?)
    }
}

impl Route<Local, RenderResourceRequest> for CohesionContext {
    async fn route(
        &self,
        request: RenderResourceRequest,
    ) -> Result<Option<Component>, ServerError> {
        let resource = self
            .database
            .send(GetResourceCommand {
                workspace_id: request.workspace_id,
                resource_id: request.resource_id,
            })
            .await?;
        render_resource(&self.extensions, &self.database, resource).await
    }
}

impl Route<Local, RefreshResourcesRequest> for CohesionContext {
    async fn route(
        &self,
        request: RefreshResourcesRequest,
    ) -> Result<ResourceRefreshSummary, ServerError> {
        self.database
            .send(GetWorkspaceCommand {
                id: request.workspace_id,
            })
            .await?;
        Ok(refresh_resources(self, Some(request.workspace_id)).await)
    }
}
