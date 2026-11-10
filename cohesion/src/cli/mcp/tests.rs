use serde_json::{Value, json};

use super::{Scripts, handle};

/// Stands in for cohesiond: echoes a script back as its result, fails one that says
/// `throw`, and declares a one-line API unless `offline`.
struct FakeScripts {
    offline: bool,
}

impl Scripts for FakeScripts {
    async fn run(&self, script: String) -> Result<String, String> {
        if script.contains("throw") {
            Err("cohesiond: boom".to_string())
        } else {
            Ok(serde_json::to_string(&script).unwrap())
        }
    }

    async fn documentation(&self) -> Result<String, String> {
        if self.offline {
            Err("unreachable".to_string())
        } else {
            Ok("declare const cohesion: Cohesion;".to_string())
        }
    }
}

fn reply(message: Value) -> Option<Value> {
    reply_from(&FakeScripts { offline: false }, message)
}

fn reply_from(scripts: &FakeScripts, message: Value) -> Option<Value> {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(handle(scripts, &message.to_string()))
}

fn call(arguments: Value) -> Value {
    reply(json!({
        "jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": { "name": "code", "arguments": arguments },
    }))
    .unwrap()
}

#[test]
fn initialize_agrees_on_a_version() {
    let known = reply(json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2025-03-26", "capabilities": {} },
    }))
    .unwrap();
    assert_eq!(known["id"], 1);
    assert_eq!(known["result"]["protocolVersion"], "2025-03-26");
    assert!(known["result"]["capabilities"]["tools"].is_object());
    assert_eq!(known["result"]["serverInfo"]["name"], "cohesion");

    let unknown = reply(json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "1999-01-01" },
    }))
    .unwrap();
    assert_eq!(unknown["result"]["protocolVersion"], "2025-11-25");
}

#[test]
fn notifications_get_no_reply() {
    let initialized = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    assert_eq!(reply(initialized), None);
}

#[test]
fn lists_only_the_code_tool() {
    let listed = reply(json!({ "jsonrpc": "2.0", "id": "a", "method": "tools/list" })).unwrap();
    let tools = listed["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], "code");
    assert_eq!(tools[0]["inputSchema"]["required"], json!(["code"]));
}

#[test]
fn the_api_is_a_resource() {
    let listed = reply(json!({ "jsonrpc": "2.0", "id": 1, "method": "resources/list" })).unwrap();
    let uri = listed["result"]["resources"][0]["uri"].clone();
    let tools = reply(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).unwrap();
    let description = tools["result"]["tools"][0]["description"].as_str().unwrap();
    assert!(description.contains(uri.as_str().unwrap()));

    let read = json!({
        "jsonrpc": "2.0", "id": 3, "method": "resources/read", "params": { "uri": uri },
    });
    let api = reply(read.clone()).unwrap();
    assert_eq!(
        api["result"]["contents"][0]["text"],
        "declare const cohesion: Cohesion;"
    );
    let offline = reply_from(&FakeScripts { offline: true }, read).unwrap();
    assert_eq!(offline["error"]["message"], "unreachable");

    let other = reply(json!({
        "jsonrpc": "2.0", "id": 4, "method": "resources/read",
        "params": { "uri": "cohesion://other" },
    }))
    .unwrap();
    assert_eq!(other["error"]["code"], -32002);
}

#[test]
fn code_returns_the_script_result() {
    let result = call(json!({ "code": "1 + 1" }));
    assert_eq!(result["id"], 3);
    assert_eq!(result["result"]["isError"], false);
    assert_eq!(result["result"]["content"][0]["type"], "text");
    assert_eq!(result["result"]["content"][0]["text"], "\"1 + 1\"");
}

#[test]
fn a_failing_script_is_an_error_result() {
    let result = call(json!({ "code": "throw 1" }));
    assert_eq!(result["result"]["isError"], true);
    assert_eq!(result["result"]["content"][0]["text"], "cohesiond: boom");
}

#[test]
fn malformed_calls_are_protocol_errors() {
    assert_eq!(call(json!({}))["error"]["code"], -32602);
    let other_tool = reply(json!({
        "jsonrpc": "2.0", "id": 4, "method": "tools/call",
        "params": { "name": "other", "arguments": {} },
    }))
    .unwrap();
    assert_eq!(other_tool["error"]["code"], -32602);
    let unknown = reply(json!({ "jsonrpc": "2.0", "id": 5, "method": "prompts/list" }));
    assert_eq!(unknown.unwrap()["error"]["code"], -32601);
    let garbage = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(handle(&FakeScripts { offline: false }, "{not json"))
        .unwrap();
    assert_eq!(garbage["error"]["code"], -32700);
    assert_eq!(garbage["id"], Value::Null);
}
