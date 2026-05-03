use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use wasmtime::component::{Component, Linker, ResourceTable, bindgen};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView};

bindgen!({
    path: "wit",
    world: "skill-runtime",
    trappable_imports: true,
});

use skill_forge::runtime::anthropic_host::Host as AnthropicHost;
use skill_forge::runtime::exec_host::Host as ExecHost;
use skill_forge::runtime::invoke_host::Host as InvokeHost;
use skill_forge::runtime::llm_host::Host as LlmHost;
use skill_forge::runtime::skill_loader_host::Host as SkillLoaderHost;
use skill_forge::runtime::types::{ErrorCode, Host as TypesHost};

const RUNTIME_CWASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/skill-runtime.cwasm"));

#[allow(dead_code)]
const SKILL_CALL_LLM_JS: &str = include_str!("../agent/dist/skills/call-llm.js");
const SKILL_INTERPRET_JS: &str = include_str!("../agent/dist/skills/interpret.js");
const SKILL_GENERATE_CODE_JS: &str = include_str!("../agent/dist/skills/generate-code.js");
const SKILL_GENERATE_CODE_FROM_SIGNATURE_JS: &str =
    include_str!("../agent/dist/skills/generate-code-from-signature.js");
const SKILL_ECHO_JS: &str = include_str!("../agent/dist/skills/echo.js");
const SKILL_ERROR_JS: &str = include_str!("../agent/dist/skills/error.js");
const SKILL_COMPOSE_JS: &str = include_str!("../agent/dist/skills/compose.js");

const MAX_INVOKE_DEPTH: usize = 8;

const BUILTIN_SKILLS: &[(&str, &str)] = &[
    ("call-llm", SKILL_CALL_LLM_JS),
    ("interpret", SKILL_INTERPRET_JS),
    ("generate-code", SKILL_GENERATE_CODE_JS),
    (
        "generate-code-from-signature",
        SKILL_GENERATE_CODE_FROM_SIGNATURE_JS,
    ),
    ("echo", SKILL_ECHO_JS),
    ("error", SKILL_ERROR_JS),
    ("compose", SKILL_COMPOSE_JS),
];

fn lookup_builtin_skill(name: &str) -> Option<&'static str> {
    BUILTIN_SKILLS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, src)| *src)
}

fn trace_enabled() -> bool {
    env::var("SKILL_FORGE_TRACE")
        .map(|v| !v.is_empty() && v != "0")
        .unwrap_or(false)
}

fn log_trace(name: &str, start: Instant) {
    if trace_enabled() {
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        eprintln!("[trace] {name}: {ms:.3}ms");
    }
}

#[derive(Parser, Debug)]
#[command(name = "skill-forge", about = "skill-forge host")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Run {
        #[arg(long)]
        skill: PathBuf,
        #[arg(long, default_value = "{}")]
        args: String,
        #[arg(long)]
        model: String,
    },
    Generate {
        #[arg(long)]
        prompt: String,
        #[arg(long)]
        model: String,
        #[arg(long)]
        signature_file: Option<PathBuf>,
    },
    Interpret {
        #[arg(long)]
        prompt: String,
        #[arg(long)]
        model: String,
    },
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum Profile {
    User,
    Builtin,
}

#[derive(Clone)]
struct LlmConfig {
    model: String,
    api_key: String,
}

struct SkillState {
    ctx: WasiCtx,
    table: ResourceTable,
    skill_source: String,
    profile: Profile,
    llm_config: Option<LlmConfig>,
    engine: Engine,
    component: Component,
    depth: usize,
}

impl WasiView for SkillState {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.ctx
    }
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

impl SkillLoaderHost for SkillState {
    fn get_source(&mut self) -> wasmtime::Result<String> {
        Ok(self.skill_source.clone())
    }
}

impl TypesHost for SkillState {}

impl LlmHost for SkillState {
    fn call_llm(
        &mut self,
        prompt: String,
        input_json: String,
    ) -> wasmtime::Result<std::result::Result<String, String>> {
        let cfg = match self.llm_config.as_ref() {
            Some(c) => c,
            None => {
                return Ok(Err(
                    "capability-denied: call-llm is not configured".to_string()
                ));
            }
        };
        Ok(host_call_llm(
            &prompt,
            &input_json,
            &cfg.model,
            &cfg.api_key,
        ))
    }
}

impl ExecHost for SkillState {
    fn exec_cmd(
        &mut self,
        cmd: String,
        args: Vec<String>,
    ) -> wasmtime::Result<std::result::Result<String, String>> {
        Ok(exec_cmd_impl(&cmd, &args))
    }
}

impl InvokeHost for SkillState {
    fn invoke(
        &mut self,
        skill_name: String,
        args_json: String,
    ) -> wasmtime::Result<std::result::Result<String, SkillError>> {
        let next_depth = self.depth + 1;
        if next_depth > MAX_INVOKE_DEPTH {
            return Ok(Err(SkillError {
                code: ErrorCode::DepthExceeded,
                message: format!("depth-exceeded: invoke depth limit {MAX_INVOKE_DEPTH} exceeded"),
                stack: None,
            }));
        }
        let source = match lookup_builtin_skill(&skill_name) {
            Some(s) => s,
            None => {
                return Ok(Err(SkillError {
                    code: ErrorCode::CapabilityDenied,
                    message: format!("capability-denied: unknown skill: {skill_name}"),
                    stack: None,
                }));
            }
        };
        let engine = self.engine.clone();
        let component = self.component.clone();
        let llm_config = self.llm_config.clone();
        let linker = build_linker(&engine)
            .map_err(|e| anyhow::anyhow!("failed to build linker for invoke: {e}"))?;
        let (mut store, runtime) = instantiate(
            &engine,
            &component,
            &linker,
            source.to_string(),
            Profile::Builtin,
            llm_config,
            next_depth,
        )
        .map_err(|e| anyhow::anyhow!("failed to instantiate skill for invoke: {e}"))?;
        let started = Instant::now();
        let r = runtime.call_run(&mut store, &args_json)?;
        log_trace("invoke run()", started);
        Ok(r)
    }
}

impl AnthropicHost for SkillState {
    fn messages(
        &mut self,
        body_json: String,
        api_key: String,
    ) -> wasmtime::Result<std::result::Result<String, String>> {
        if self.profile != Profile::Builtin {
            return Err(anyhow::anyhow!(
                "capability-denied: anthropic-host is not available to user skills"
            ));
        }
        let started = Instant::now();
        let r = anthropic_messages_blocking(&body_json, &api_key);
        log_trace("anthropic-host roundtrip", started);
        Ok(r)
    }
}

fn host_call_llm(
    prompt: &str,
    input_json: &str,
    model: &str,
    api_key: &str,
) -> std::result::Result<String, String> {
    const BENCH_MOCK_PROMPT: &str = "__BENCH_MOCK__";
    if prompt == BENCH_MOCK_PROMPT {
        return Ok(input_json.to_string());
    }
    if api_key.is_empty() {
        return Err("spec-violation: api-key argument is empty".into());
    }
    let body = serde_json::json!({
        "model": model,
        "max_tokens": 4096,
        "system": prompt,
        "messages": [{"role": "user", "content": input_json}],
    })
    .to_string();
    let raw = anthropic_messages_blocking(&body, api_key)?;
    let resp: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("parse-error: {e}"))?;
    let content = resp
        .get("content")
        .and_then(|c| c.as_array())
        .ok_or_else(|| "spec-violation: response missing content array".to_string())?;
    let mut text = String::new();
    for block in content {
        if block.get("type").and_then(|t| t.as_str()) == Some("text") {
            if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                text.push_str(t);
            }
        }
    }
    Ok(text)
}

fn exec_cmd_impl(cmd: &str, args: &[String]) -> std::result::Result<String, String> {
    let output = std::process::Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("exec-error: failed to spawn {cmd}: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "exec-error: {cmd} exited with status {}: {stderr}",
            output.status
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|e| format!("exec-error: stdout is not valid UTF-8: {e}"))
}

static HTTP: OnceLock<reqwest::blocking::Client> = OnceLock::new();

fn anthropic_messages_blocking(body: &str, api_key: &str) -> std::result::Result<String, String> {
    let client = HTTP.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .build()
            .expect("failed to construct reqwest client")
    });
    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("content-type", "application/json")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .body(body.to_string())
        .send()
        .map_err(|e| format!("network-error: {e}"))?;
    let status = resp.status();
    let text = resp.text().map_err(|e| format!("network-error: {e}"))?;
    if !status.is_success() {
        return Err(format!("HTTP {}: {}", status.as_u16(), text));
    }
    Ok(text)
}

fn main() -> Result<()> {
    let _ = dotenvy::dotenv();

    let args = Args::parse();

    let started = Instant::now();
    let mut config = Config::new();
    config.cache_config_load_default()?;
    let engine = Engine::new(&config)?;
    log_trace("engine new", started);

    match args.command {
        Command::Run {
            skill,
            args: args_json,
            model,
        } => run_skill_run(&engine, &skill, args_json, model),
        Command::Generate {
            prompt,
            model,
            signature_file,
        } => run_generate(&engine, &prompt, &model, signature_file.as_ref()),
        Command::Interpret { prompt, model } => run_interpret(&engine, &prompt, &model),
    }
}

fn deserialize_runtime_component(engine: &Engine) -> Result<Component> {
    let started = Instant::now();
    // SAFETY: cwasm bytes are produced by build.rs with the same wasmtime version
    // (Engine::precompile_component) embedded into this binary at compile time.
    let component = unsafe { Component::deserialize(engine, RUNTIME_CWASM) }
        .context("failed to deserialize embedded skill-runtime cwasm")?;
    log_trace("runtime deserialize", started);
    Ok(component)
}

fn build_linker(engine: &Engine) -> Result<Linker<SkillState>> {
    let mut linker: Linker<SkillState> = Linker::new(engine);
    wasmtime_wasi::add_to_linker_sync(&mut linker)?;
    SkillRuntime::add_to_linker(&mut linker, |s| s)?;
    Ok(linker)
}

fn instantiate(
    engine: &Engine,
    component: &Component,
    linker: &Linker<SkillState>,
    skill_source: String,
    profile: Profile,
    llm_config: Option<LlmConfig>,
    depth: usize,
) -> Result<(Store<SkillState>, SkillRuntime)> {
    let state = SkillState {
        ctx: WasiCtxBuilder::new().inherit_stdio().build(),
        table: ResourceTable::new(),
        skill_source,
        profile,
        llm_config,
        engine: engine.clone(),
        component: component.clone(),
        depth,
    };
    let mut store = Store::new(engine, state);
    let started = Instant::now();
    let runtime = SkillRuntime::instantiate(&mut store, component, linker)?;
    log_trace("runtime instantiate (incl. main.js eval)", started);
    Ok((store, runtime))
}

fn run_skill_run(
    engine: &Engine,
    skill_path: &PathBuf,
    args_json: String,
    model: String,
) -> Result<()> {
    let api_key = env::var("ANTHROPIC_API_KEY")
        .context("ANTHROPIC_API_KEY environment variable is required")?;

    let source = fs::read_to_string(skill_path)
        .with_context(|| format!("failed to read skill source: {}", skill_path.display()))?;

    let component = deserialize_runtime_component(engine)?;
    let linker = build_linker(engine)?;
    let (mut store, runtime) = instantiate(
        engine,
        &component,
        &linker,
        source,
        Profile::User,
        Some(LlmConfig { model, api_key }),
        0,
    )?;

    let started = Instant::now();
    let r = runtime.call_run(&mut store, &args_json)?;
    log_trace(
        "run() (incl. JSON.parse + skill load + run + stringify)",
        started,
    );

    match r {
        Ok(json) => println!("{json}"),
        Err(err) => {
            print_skill_error(&err);
            std::process::exit(1);
        }
    }

    Ok(())
}

fn run_builtin_skill(
    engine: &Engine,
    skill_source: &str,
    args_json: &str,
) -> Result<std::result::Result<String, SkillError>> {
    let component = deserialize_runtime_component(engine)?;
    let linker = build_linker(engine)?;
    let (mut store, runtime) = instantiate(
        engine,
        &component,
        &linker,
        skill_source.to_string(),
        Profile::Builtin,
        None,
        0,
    )?;
    let started = Instant::now();
    let r = runtime.call_run(&mut store, args_json)?;
    log_trace("builtin run()", started);
    Ok(r)
}

fn run_generate(
    engine: &Engine,
    prompt: &str,
    model: &str,
    signature_file: Option<&PathBuf>,
) -> Result<()> {
    let api_key = env::var("ANTHROPIC_API_KEY")
        .context("ANTHROPIC_API_KEY environment variable is required")?;

    let (skill_source, args_json) = match signature_file {
        Some(path) => {
            let signature = match load_signature_value(path) {
                Ok(sig) => sig,
                Err(msg) => {
                    eprintln!("agent error: {msg}");
                    std::process::exit(1);
                }
            };
            let args = serde_json::json!({
                "prompt": prompt,
                "signature": signature,
                "model": model,
                "apiKey": api_key,
            });
            (SKILL_GENERATE_CODE_FROM_SIGNATURE_JS, args.to_string())
        }
        None => {
            let args = serde_json::json!({
                "prompt": prompt,
                "model": model,
                "apiKey": api_key,
            });
            (SKILL_GENERATE_CODE_JS, args.to_string())
        }
    };

    let r = run_builtin_skill(engine, skill_source, &args_json)?;
    match r {
        Ok(json) => print_generated(&json)?,
        Err(err) => {
            eprintln!("agent error: [{:?}] {}", err.code, err.message);
            std::process::exit(1);
        }
    }

    Ok(())
}

fn print_generated(json: &str) -> Result<()> {
    let v: serde_json::Value = serde_json::from_str(json)
        .with_context(|| format!("failed to parse builtin skill output as JSON: {json}"))?;
    let code = v.get("code").and_then(|c| c.as_str()).unwrap_or("");
    let capabilities = v
        .get("capabilities")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    println!("code:");
    println!("{code}");
    println!("capabilities: {}", capabilities.join(", "));
    Ok(())
}

fn load_signature_value(path: &PathBuf) -> std::result::Result<serde_json::Value, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("failed to read signature file {}: {e}", path.display()))?;
    let parsed: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("parse-error: invalid JSON in signature file: {e}"))?;
    let array = parsed
        .as_array()
        .ok_or_else(|| "parse-error: signature root is not an array".to_string())?;
    for (i, entry) in array.iter().enumerate() {
        let obj = entry
            .as_object()
            .ok_or_else(|| format!("parse-error: signature entry {i} is not an object"))?;
        for key in ["tool", "input", "output"] {
            if !obj.get(key).map(|v| v.is_string()).unwrap_or(false) {
                return Err(format!(
                    "parse-error: signature entry {i} is missing string field \"{key}\""
                ));
            }
        }
    }
    Ok(parsed)
}

fn run_interpret(engine: &Engine, prompt: &str, model: &str) -> Result<()> {
    let api_key = env::var("ANTHROPIC_API_KEY")
        .context("ANTHROPIC_API_KEY environment variable is required")?;

    let args = serde_json::json!({
        "prompt": prompt,
        "model": model,
        "apiKey": api_key,
    });

    let r = run_builtin_skill(engine, SKILL_INTERPRET_JS, &args.to_string())?;
    match r {
        Ok(json) => print_interpreted(&json)?,
        Err(err) => {
            eprintln!("agent error: [{:?}] {}", err.code, err.message);
            std::process::exit(1);
        }
    }

    Ok(())
}

fn print_interpreted(json: &str) -> Result<()> {
    let v: serde_json::Value = serde_json::from_str(json)
        .with_context(|| format!("failed to parse builtin skill output as JSON: {json}"))?;
    let final_answer = v.get("finalAnswer").and_then(|c| c.as_str()).unwrap_or("");
    let signature = v
        .get("signature")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();
    println!("final-answer: {final_answer}");
    println!("signature:");
    print!("[");
    for (i, entry) in signature.iter().enumerate() {
        if i > 0 {
            print!(",");
        }
        println!();
        let tool = entry.get("tool").and_then(|c| c.as_str()).unwrap_or("");
        let input = entry.get("input").and_then(|c| c.as_str()).unwrap_or("");
        let output = entry.get("output").and_then(|c| c.as_str()).unwrap_or("");
        print!(
            "  {{\"tool\": {}, \"input\": {}, \"output\": {}}}",
            json_escape_string(tool),
            json_escape_string(input),
            json_escape_string(output)
        );
    }
    if !signature.is_empty() {
        println!();
    }
    println!("]");
    Ok(())
}

fn json_escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\x08' => out.push_str("\\b"),
            '\x0c' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn print_skill_error(err: &SkillError) {
    eprintln!("[{:?}] {}", err.code, err.message);
    if let Some(stack) = &err.stack {
        eprintln!("stack:\n{stack}");
    }
}
