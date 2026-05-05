use std::io::{self, BufRead, Write};

use serde_json::{Value, json};

use super::tools;

const PROTOCOL_VERSION: &str = "2024-11-05";

pub fn run() -> anyhow::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => {
                respond_error(&mut out, Value::Null, -32700, "parse error");
                continue;
            }
        };
        let method = req.get("method").and_then(Value::as_str).unwrap_or("");
        let id = req.get("id").cloned();
        handle(&mut out, method, id, req.get("params").cloned());
    }
    Ok(())
}

fn handle(out: &mut impl Write, method: &str, id: Option<Value>, params: Option<Value>) {
    match method {
        "initialize" => {
            let result = json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "skill-forge-mcp", "version": env!("CARGO_PKG_VERSION") }
            });
            respond_request(out, id, result);
        }
        "notifications/initialized" => {
            // notification: no response
        }
        "tools/list" => {
            let result = json!({ "tools": tools::tool_specs() });
            respond_request(out, id, result);
        }
        "tools/call" => {
            let params = params.unwrap_or(json!({}));
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            match tools::handle_call(name, &args) {
                Ok(result) => respond_request(out, id, result),
                Err(msg) => respond_error(out, id.unwrap_or(Value::Null), -32602, &msg),
            }
        }
        _ => {
            if id.is_some() {
                respond_error(out, id.unwrap_or(Value::Null), -32601, "method not found");
            }
        }
    }
}

fn respond_request(out: &mut impl Write, id: Option<Value>, result: Value) {
    let id = id.unwrap_or(Value::Null);
    let resp = json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    });
    let _ = writeln!(out, "{resp}");
    let _ = out.flush();
}

fn respond_error(out: &mut impl Write, id: Value, code: i32, message: &str) {
    let resp = json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    });
    let _ = writeln!(out, "{resp}");
    let _ = out.flush();
}
