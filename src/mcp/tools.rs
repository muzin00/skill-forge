use std::time::Duration;

use serde_json::{Value, json};

const DEFAULT_LLM_MODEL: &str = "claude-sonnet-4-6";
const DEFAULT_TIMEOUT_SECS: u64 = 60;

pub fn tool_specs() -> Value {
    json!([
        {
            "name": "callLlm",
            "description": "Run a real LLM call to observe the behavior you plan to encode in the generated skill (e.g. summarization, classification).",
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
            "description": "Run a real command on the host to observe behavior (e.g. `gh issue view`, `which`, `--help`). Use sparingly and prefer read-only commands.",
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

pub fn handle_call(name: &str, args: &Value) -> Result<Value, String> {
    match name {
        "callLlm" => match call_llm(args) {
            Ok(text) => Ok(text_result(&text, false)),
            Err(msg) => Ok(text_result(&format!("callLlm error: {msg}"), true)),
        },
        "execCmd" => match exec_cmd(args) {
            Ok(stdout) => Ok(text_result(&stdout, false)),
            Err(msg) => Ok(text_result(&format!("execCmd error: {msg}"), true)),
        },
        "submit_generated_code" => Ok(text_result("ok", false)),
        other => Err(format!("unknown tool: {other}")),
    }
}

fn call_llm(args: &Value) -> Result<String, String> {
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
    let timeout_secs = std::env::var("SKILL_FORGE_TIMEOUT")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(DEFAULT_TIMEOUT_SECS);
    crate::host_call_llm(
        prompt,
        &input_json,
        &model,
        &api_key,
        Duration::from_secs(timeout_secs),
    )
}

fn exec_cmd(args: &Value) -> Result<String, String> {
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
    crate::exec_cmd_impl(cmd, &cmd_args)
}

fn text_result(text: &str, is_error: bool) -> Value {
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": is_error
    })
}
