use std::io::{self, BufRead, Write};

use serde_json::{Value, json};

const PROTOCOL_VERSION: &str = "2024-11-05";

fn main() {
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
}

fn handle(out: &mut impl Write, method: &str, id: Option<Value>, params: Option<Value>) {
    match method {
        "initialize" => {
            let result = json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "mcp-poc-server", "version": "0.1.0" }
            });
            respond_request(out, id, result);
        }
        "notifications/initialized" => {
            // notification: no response
        }
        "tools/list" => {
            let result = json!({ "tools": tool_specs() });
            respond_request(out, id, result);
        }
        "tools/call" => {
            let params = params.unwrap_or(json!({}));
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            handle_tools_call(out, id, name, args);
        }
        _ => {
            if id.is_some() {
                respond_error(out, id.unwrap_or(Value::Null), -32601, "method not found");
            }
        }
    }
}

fn handle_tools_call(out: &mut impl Write, id: Option<Value>, name: &str, args: Value) {
    match name {
        "callLlm" => {
            let result = json!({
                "content": [{ "type": "text", "text": "mock-llm-response" }],
                "isError": false
            });
            respond_request(out, id, result);
        }
        "execCmd" => {
            let result = json!({
                "content": [{ "type": "text", "text": "mock-exec-response" }],
                "isError": false
            });
            respond_request(out, id, result);
        }
        "submit_generated_code" => {
            // Dump the captured input to stderr so the verification script can extract it.
            // Single-line ndjson form: SUBMIT:<json>
            eprintln!("SUBMIT:{}", args);
            let result = json!({
                "content": [{ "type": "text", "text": "ok" }],
                "isError": false
            });
            respond_request(out, id, result);
        }
        _ => {
            respond_error(
                out,
                id.unwrap_or(Value::Null),
                -32602,
                &format!("unknown tool: {name}"),
            );
        }
    }
}

fn tool_specs() -> Value {
    json!([
        {
            "name": "callLlm",
            "description": "Run a real LLM call to observe behavior you plan to encode in the skill. Use this to verify the summarization / naming step.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "prompt": { "type": "string" },
                    "input": { "type": "object" }
                },
                "required": ["prompt"]
            }
        },
        {
            "name": "execCmd",
            "description": "Run a real command to observe behavior. Use this to verify external command behavior such as `gh issue view` and `git checkout -b`.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "cmd": { "type": "string" },
                    "args": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["cmd", "args"]
            }
        },
        {
            "name": "submit_generated_code",
            "description": "Submit the final generated skill. Call exactly once at the end. Do not produce free-form text after.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "code": { "type": "string" },
                    "capabilities": { "type": "array", "items": { "type": "string" } },
                    "schema": { "type": "object" }
                },
                "required": ["code", "capabilities", "schema"]
            }
        }
    ])
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
