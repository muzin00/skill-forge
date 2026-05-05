use std::io::{self, BufRead, Write};
use std::sync::OnceLock;

use serde_json::{Value, json};

const PROTOCOL_VERSION: &str = "2024-11-05";
const DEFAULT_LLM_MODEL: &str = "claude-sonnet-4-6";

#[derive(Copy, Clone, Eq, PartialEq)]
enum Mode {
    Mock,
    Real,
}

fn current_mode() -> Mode {
    match std::env::var("MCP_POC_MODE").as_deref() {
        Ok("mock") => Mode::Mock,
        _ => Mode::Real,
    }
}

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
    let result = match name {
        "callLlm" => match current_mode() {
            Mode::Mock => tool_text_result("mock-llm-response", false),
            Mode::Real => match real_call_llm(&args) {
                Ok(text) => tool_text_result(&text, false),
                Err(msg) => tool_text_result(&format!("callLlm error: {msg}"), true),
            },
        },
        "execCmd" => match current_mode() {
            Mode::Mock => tool_text_result("mock-exec-response", false),
            Mode::Real => match real_exec_cmd(&args) {
                Ok(stdout) => tool_text_result(&stdout, false),
                Err(msg) => tool_text_result(&format!("execCmd error: {msg}"), true),
            },
        },
        "submit_generated_code" => {
            // dump captured input for the verification script (debug). Extraction is
            // primarily done by the script via stream-json from claude.
            eprintln!("SUBMIT:{args}");
            tool_text_result("ok", false)
        }
        _ => {
            respond_error(
                out,
                id.unwrap_or(Value::Null),
                -32602,
                &format!("unknown tool: {name}"),
            );
            return;
        }
    };
    respond_request(out, id, result);
}

fn tool_text_result(text: &str, is_error: bool) -> Value {
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": is_error
    })
}

fn real_call_llm(args: &Value) -> Result<String, String> {
    let prompt = args
        .get("prompt")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string field 'prompt'".to_string())?;
    let input_json = match args.get("input") {
        Some(v) => v.to_string(),
        None => "{}".to_string(),
    };
    let model = std::env::var("MCP_LLM_MODEL").unwrap_or_else(|_| DEFAULT_LLM_MODEL.to_string());
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| "ANTHROPIC_API_KEY env var not set".to_string())?;
    let body = json!({
        "model": model,
        "max_tokens": 4096,
        "system": prompt,
        "messages": [{ "role": "user", "content": input_json }],
    })
    .to_string();
    let raw = anthropic_messages_blocking(&body, &api_key)?;
    let parsed: Value = serde_json::from_str(&raw).map_err(|e| format!("parse: {e}"))?;
    let content = parsed
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| "response missing content array".to_string())?;
    let mut text = String::new();
    for block in content {
        if block.get("type").and_then(Value::as_str) == Some("text") {
            if let Some(t) = block.get("text").and_then(Value::as_str) {
                text.push_str(t);
            }
        }
    }
    Ok(text)
}

fn anthropic_messages_blocking(body: &str, api_key: &str) -> Result<String, String> {
    static HTTP: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    let client = HTTP.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .build()
            .expect("failed to build reqwest client")
    });
    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("content-type", "application/json")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .body(body.to_string())
        .send()
        .map_err(|e| format!("network: {e}"))?;
    let status = resp.status();
    let text = resp.text().map_err(|e| format!("network: {e}"))?;
    if !status.is_success() {
        return Err(format!("HTTP {}: {text}", status.as_u16()));
    }
    Ok(text)
}

fn real_exec_cmd(args: &Value) -> Result<String, String> {
    let cmd = args
        .get("cmd")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string field 'cmd'".to_string())?;
    let cmd_args: Vec<String> = match args.get("args") {
        Some(v) => v
            .as_array()
            .ok_or_else(|| "field 'args' is not an array".to_string())?
            .iter()
            .map(|x| x.as_str().unwrap_or("").to_string())
            .collect(),
        None => vec![],
    };
    let output = std::process::Command::new(cmd)
        .args(&cmd_args)
        .output()
        .map_err(|e| format!("spawn {cmd}: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("{cmd} exit {}: {stderr}", output.status));
    }
    String::from_utf8(output.stdout).map_err(|e| format!("utf8: {e}"))
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
