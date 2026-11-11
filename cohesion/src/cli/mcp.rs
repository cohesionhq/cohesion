//! `cohesion mcp`: a local MCP server on stdin and stdout, for an MCP client such as
//! Claude Code to start. It speaks JSON-RPC 2.0, one message per line, and offers one
//! tool, `code`, which runs a script on cohesiond through this client's connection,
//! and one resource, the script API's declarations, which are too long for the tool's
//! description.

use cohesion_core::protocol::{DocumentationRequest, RunScriptRequest};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::{cli_context::Context, cli_error::CliError};

#[cfg(test)]
mod tests;

/// The MCP versions this server speaks, newest first. A client asking for one of
/// them gets it back; any other gets the newest.
const PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// The resource holding the script API's declarations, as `cohesion script new`
/// writes them next to a script.
const API_URI: &str = "cohesion://cohesion.d.ts";

/// The `code` tool's description.
const TOOL_DESCRIPTION: &str = "Run JavaScript on cohesiond against the user's Cohesion \
    data and return the value of its last expression as JSON, e.g. \
    `cohesion.workspaces().map(w => w.title)`. Plain synchronous JavaScript: no imports, \
    require or async. Reach the web with the blocking `network.fetch`. The API is declared \
    in TypeScript in the resource cohesion://cohesion.d.ts. Read it before writing a \
    script.";

/// The `code` field's description.
const CODE_FIELD: &str = "The script to run. Its last expression is the tool's result.";

/// What the MCP server needs from cohesiond, so tests can stand in for it.
trait Scripts {
    /// The JSON text of `script`'s completion value, or why it failed.
    async fn run(&self, script: String) -> Result<String, String>;
    /// The script API's TypeScript declarations.
    async fn documentation(&self) -> Result<String, String>;
}

impl Scripts for Context {
    async fn run(&self, script: String) -> Result<String, String> {
        self.call(RunScriptRequest { script })
            .await
            .map(|result| result.0)
            .map_err(|err| err.to_string())
    }

    async fn documentation(&self) -> Result<String, String> {
        self.call(DocumentationRequest)
            .await
            .map(|reply| reply.documentation)
            .map_err(|err| err.to_string())
    }
}

/// Answers each message read from stdin on stdout until stdin closes.
pub async fn run(ctx: &Context) -> Result<(), CliError> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    while let Some(line) = lines.next_line().await.map_err(CliError::CannotReadStdin)? {
        if line.trim().is_empty() {
            continue;
        }
        if let Some(reply) = handle(ctx, &line).await {
            let mut reply = reply.to_string();
            reply.push('\n');
            stdout
                .write_all(reply.as_bytes())
                .await
                .map_err(CliError::CannotWriteStdout)?;
            stdout.flush().await.map_err(CliError::CannotWriteStdout)?;
        }
    }
    Ok(())
}

/// The reply to one JSON-RPC message, or `None` for a notification.
async fn handle(scripts: &impl Scripts, line: &str) -> Option<Value> {
    let Ok(message) = serde_json::from_str::<Value>(line) else {
        return Some(error(Value::Null, -32700, "Parse error."));
    };
    // A message without an id is a notification, answered by nothing.
    let id = message.get("id")?.clone();
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let result = match message.get("method").and_then(Value::as_str) {
        Some("initialize") => initialize(&params),
        Some("ping") => json!({}),
        Some("tools/list") => json!({ "tools": [code_tool()] }),
        Some("tools/call") => match call_tool(scripts, &params).await {
            Ok(result) => result,
            Err(message) => return Some(error(id, -32602, &message)),
        },
        Some("resources/list") => json!({ "resources": [api_resource()] }),
        Some("resources/read") => match read_resource(scripts, &params).await {
            Ok(result) => result,
            Err((code, message)) => return Some(error(id, code, &message)),
        },
        Some(method) => return Some(error(id, -32601, &format!("No method {method}."))),
        None => return Some(error(id, -32600, "The request has no method.")),
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn initialize(params: &Value) -> Value {
    let requested = params.get("protocolVersion").and_then(Value::as_str);
    let version = requested
        .filter(|version| PROTOCOL_VERSIONS.contains(version))
        .unwrap_or(PROTOCOL_VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {}, "resources": {} },
        "serverInfo": { "name": "cohesion", "version": env!("CARGO_PKG_VERSION") },
    })
}

fn code_tool() -> Value {
    json!({
        "name": "code",
        "description": TOOL_DESCRIPTION,
        "inputSchema": {
            "type": "object",
            "properties": { "code": { "type": "string", "description": CODE_FIELD } },
            "required": ["code"],
        },
    })
}

fn api_resource() -> Value {
    json!({
        "uri": API_URI,
        "name": "cohesion.d.ts",
        "description": "The script API the code tool runs against, as TypeScript declarations.",
        "mimeType": "application/typescript",
    })
}

/// The script API as cohesiond declares it now, installed services included, or a
/// JSON-RPC error code and message.
async fn read_resource(scripts: &impl Scripts, params: &Value) -> Result<Value, (i64, String)> {
    match params.get("uri").and_then(Value::as_str) {
        Some(API_URI) => {}
        Some(uri) => return Err((-32002, format!("No resource {uri}."))),
        None => return Err((-32602, "The read names no resource.".to_string())),
    }
    let documentation = scripts
        .documentation()
        .await
        .map_err(|message| (-32603, message))?;
    Ok(json!({
        "contents": [{
            "uri": API_URI,
            "mimeType": "application/typescript",
            "text": documentation,
        }],
    }))
}

/// Runs a `tools/call`. A script that fails is a tool result marked as an error, for
/// the model to read; a call this server can't make sense of is a protocol error.
async fn call_tool(scripts: &impl Scripts, params: &Value) -> Result<Value, String> {
    match params.get("name").and_then(Value::as_str) {
        Some("code") => {}
        Some(name) => return Err(format!("No tool {name}.")),
        None => return Err("The call names no tool.".to_string()),
    }
    let Some(code) = params
        .pointer("/arguments/code")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return Err("The code tool needs a `code` string.".to_string());
    };
    let (text, is_error) = match scripts.run(code).await {
        Ok(output) => (output, false),
        Err(message) => (message, true),
    };
    Ok(json!({
        "content": [{ "type": "text", "text": text }],
        "isError": is_error,
    }))
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
}
