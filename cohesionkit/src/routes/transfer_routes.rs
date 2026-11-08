//! Moving the database between daemons through a client: exporting it and
//! reading the export out, and writing an import in and finishing it, which
//! restarts the daemon onto it (`PendingImport`).

use cohesion_core::log::Module;
use cohesion_core::protocol::{
    AcceptedResponse, BeginDatabaseImportRequest, DatabaseExport, DatabaseExportPart,
    DatabaseImport, ExportDatabaseRequest, FinishDatabaseImportRequest, Local,
    ReadDatabaseExportRequest, Route, ServerError, WriteDatabaseImportRequest,
};
use cohesion_core::{log_error, log_info};
use cohesion_db::TransferError;

use crate::cohesion_context::CohesionContext;

impl Route<Local, ExportDatabaseRequest> for CohesionContext {
    async fn route(&self, _: ExportDatabaseRequest) -> Result<DatabaseExport, ServerError> {
        self.database.export().await.map_err(refused)
    }
}

impl Route<Local, ReadDatabaseExportRequest> for CohesionContext {
    async fn route(
        &self,
        request: ReadDatabaseExportRequest,
    ) -> Result<DatabaseExportPart, ServerError> {
        self.database
            .read_export(request.id, request.offset)
            .await
            .map_err(refused)
    }
}

impl Route<Local, BeginDatabaseImportRequest> for CohesionContext {
    async fn route(&self, _: BeginDatabaseImportRequest) -> Result<DatabaseImport, ServerError> {
        self.database.begin_import().await.map_err(refused)
    }
}

impl Route<Local, WriteDatabaseImportRequest> for CohesionContext {
    async fn route(
        &self,
        request: WriteDatabaseImportRequest,
    ) -> Result<AcceptedResponse, ServerError> {
        self.database
            .write_import(request.id, request.offset, request.data)
            .await
            .map(AcceptedResponse::from)
            .map_err(refused)
    }
}

impl Route<Local, FinishDatabaseImportRequest> for CohesionContext {
    async fn route(
        &self,
        request: FinishDatabaseImportRequest,
    ) -> Result<AcceptedResponse, ServerError> {
        let staged = self
            .database
            .stage_import(request.id)
            .await
            .map_err(refused)?;
        log_info!(
            Module::Database,
            "An import is ready; restarting to put it in place."
        );
        self.pending_import.restart_onto(staged);
        Ok(AcceptedResponse)
    }
}

/// What the client hears: why, when it can do something about it.
fn refused(error: TransferError) -> ServerError {
    match error {
        TransferError::Unknown | TransferError::WrongOffset { .. } | TransferError::NotCohesion => {
            ServerError::bad_request(error.to_string())
        }
        TransferError::InUse | TransferError::Command(_) | TransferError::Database(_) => {
            log_error!(Module::Database, "A transfer failed: {error}");
            ServerError::internal()
        }
    }
}
