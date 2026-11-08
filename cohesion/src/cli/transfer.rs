//! `cohesion export` and `cohesion import`: moving every bit of data from
//! one cohesiond to another through this client, in parts.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use cohesion_core::private_files::open_private;
use cohesion_core::protocol::{
    BeginDatabaseImportRequest, ExportDatabaseRequest, FinishDatabaseImportRequest,
    ReadDatabaseExportRequest, TransferBytes, WriteDatabaseImportRequest,
};

use super::terminal_io::{confirmed, spin};
use crate::{cli_context::Context, cli_error::CliError};

/// How much of the file one request sends: 8 MB, a third again in base64.
const PART: usize = 8 * 1024 * 1024;

/// Copies everything on the paired cohesiond to `file`, which mustn't exist.
pub async fn export(ctx: &Context, file: PathBuf) -> Result<(), CliError> {
    if file.exists() {
        return Err(CliError::ExportExists(file));
    }
    let bytes = spin("Exporting…", download(ctx, &file)).await;
    if bytes.is_err() {
        let _ = std::fs::remove_file(&file);
    }
    println!(
        "Exported {} to {} ({}). To import it, pair with the other server and run `cohesion import {}`.",
        ctx.target(),
        file.display(),
        megabytes(bytes?),
        file.display()
    );
    Ok(())
}

/// Replaces everything on the paired cohesiond with `file`, after asking.
pub async fn import(ctx: &Context, file: PathBuf, yes: bool) -> Result<(), CliError> {
    let mut source = File::open(&file).map_err(CliError::CannotReadExport)?;
    let question = format!(
        "Replace all data on {} with {}? This can’t be undone.",
        ctx.target(),
        file.display()
    );
    if !confirmed(&question, yes)? {
        return Ok(());
    }
    spin("Importing…", upload(ctx, &mut source)).await?;
    println!(
        "Imported {}. cohesiond is restarting with it. This server kept its own key and paired clients, so clients paired with the other server need to pair with this one.",
        file.display()
    );
    Ok(())
}

/// Writes the export to `file`, a part at a time; answers its size.
async fn download(ctx: &Context, file: &Path) -> Result<i64, CliError> {
    let export = ctx.call(ExportDatabaseRequest).await?;
    let mut out = open_private(file).map_err(CliError::CannotWriteExport)?;
    let mut offset = 0;
    while offset < export.bytes {
        let part = ctx
            .call(ReadDatabaseExportRequest {
                id: export.id.clone(),
                offset,
            })
            .await?;
        if part.data.0.is_empty() {
            return Err(CliError::UnexpectedReply);
        }
        out.write_all(&part.data.0)
            .map_err(CliError::CannotWriteExport)?;
        offset += part.data.0.len() as i64;
    }
    out.sync_all().map_err(CliError::CannotWriteExport)?;
    Ok(export.bytes)
}

/// Sends `source` to a new import, a part at a time, then finishes it.
async fn upload(ctx: &Context, source: &mut File) -> Result<(), CliError> {
    let import = ctx.call(BeginDatabaseImportRequest).await?;
    let mut offset = 0;
    loop {
        let mut data = Vec::with_capacity(PART);
        source
            .take(PART as u64)
            .read_to_end(&mut data)
            .map_err(CliError::CannotReadExport)?;
        if data.is_empty() {
            break;
        }
        let length = data.len() as i64;
        ctx.call(WriteDatabaseImportRequest {
            id: import.id.clone(),
            offset,
            data: TransferBytes(data),
        })
        .await?;
        offset += length;
    }
    ctx.call(FinishDatabaseImportRequest { id: import.id })
        .await?;
    Ok(())
}

/// `12.3 MB`.
fn megabytes(bytes: i64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_000_000.0)
}
