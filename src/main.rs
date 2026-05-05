use std::env;
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use wasmtime::component::{Component, Linker, ResourceTable, bindgen};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView};

mod generated_schema;
mod mcp;
mod skill_args;
mod validator;

bindgen!({
    path: "wit",
    world: "skill-runtime",
    trappable_imports: true,
});

use skill_forge::runtime::anthropic_host::Host as AnthropicHost;
use skill_forge::runtime::exec_host::Host as ExecHost;
use skill_forge::runtime::invoke_host::Host as InvokeHost;
use skill_forge::runtime::llm_host::Host as LlmHost;
use skill_forge::runtime::schema_loader_host::Host as SchemaLoaderHost;
use skill_forge::runtime::skill_loader_host::Host as SkillLoaderHost;
use skill_forge::runtime::types::{ErrorCode, Host as TypesHost};

const RUNTIME_CWASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/skill-runtime.cwasm"));

#[allow(dead_code)]
const SKILL_CALL_LLM_JS: &str = include_str!("../agent/dist/skills/call-llm/skill.js");
const SKILL_GENERATE_SKILL_CODE_JS: &str =
    include_str!("../agent/dist/skills/generate-skill-code/skill.js");
const SKILL_ECHO_JS: &str = include_str!("../agent/dist/skills/echo/skill.js");
const SKILL_ERROR_JS: &str = include_str!("../agent/dist/skills/error/skill.js");
const SKILL_COMPOSE_JS: &str = include_str!("../agent/dist/skills/compose/skill.js");

const SCHEMA_CALL_LLM_JS: &str = include_str!("../agent/dist/skills/call-llm/schema.js");
const SCHEMA_GENERATE_SKILL_CODE_JS: &str =
    include_str!("../agent/dist/skills/generate-skill-code/schema.js");
const SCHEMA_ECHO_JS: &str = include_str!("../agent/dist/skills/echo/schema.js");
const SCHEMA_ERROR_JS: &str = include_str!("../agent/dist/skills/error/schema.js");
const SCHEMA_COMPOSE_JS: &str = include_str!("../agent/dist/skills/compose/schema.js");

const MAX_INVOKE_DEPTH: usize = 8;

const BUILTIN_SKILLS: &[(&str, &str, &str)] = &[
    ("call-llm", SKILL_CALL_LLM_JS, SCHEMA_CALL_LLM_JS),
    (
        "generate-skill-code",
        SKILL_GENERATE_SKILL_CODE_JS,
        SCHEMA_GENERATE_SKILL_CODE_JS,
    ),
    ("echo", SKILL_ECHO_JS, SCHEMA_ECHO_JS),
    ("error", SKILL_ERROR_JS, SCHEMA_ERROR_JS),
    ("compose", SKILL_COMPOSE_JS, SCHEMA_COMPOSE_JS),
];

fn lookup_builtin_skill(name: &str) -> Option<(&'static str, &'static str)> {
    BUILTIN_SKILLS
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, src, schema)| (*src, *schema))
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
    #[command(disable_help_flag = true)]
    Run {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true, num_args = 0..)]
        argv: Vec<String>,
    },
    Generate {
        #[arg(long)]
        prompt: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        model: String,
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    #[command(hide = true)]
    McpServer,
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
    schema_source: String,
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

impl SchemaLoaderHost for SkillState {
    fn get_schema_source(&mut self) -> wasmtime::Result<String> {
        Ok(self.schema_source.clone())
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
        let (source, schema_source) = match lookup_builtin_skill(&skill_name) {
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
            schema_source.to_string(),
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

pub(crate) fn host_call_llm(
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

pub(crate) fn exec_cmd_impl(cmd: &str, args: &[String]) -> std::result::Result<String, String> {
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
        Command::Run { argv } => run_skill_run(&engine, argv),
        Command::Generate {
            prompt,
            name,
            model,
            force,
        } => run_generate(&engine, &prompt, &name, &model, force),
        Command::McpServer => mcp::run(),
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
    schema_source: String,
    profile: Profile,
    llm_config: Option<LlmConfig>,
    depth: usize,
) -> Result<(Store<SkillState>, SkillRuntime)> {
    let state = SkillState {
        ctx: WasiCtxBuilder::new().inherit_stdio().build(),
        table: ResourceTable::new(),
        skill_source,
        schema_source,
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

fn run_skill_run(engine: &Engine, raw_argv: Vec<String>) -> Result<()> {
    let (skill_path, model, skill_flag_argv) = parse_run_argv(raw_argv);

    let api_key = env::var("ANTHROPIC_API_KEY")
        .context("ANTHROPIC_API_KEY environment variable is required")?;

    let source = fs::read_to_string(&skill_path)
        .with_context(|| format!("failed to read skill source: {}", skill_path.display()))?;
    let schema_path = schema_path_for(&skill_path);
    let schema_source = fs::read_to_string(&schema_path)
        .with_context(|| format!("failed to read schema source: {}", schema_path.display()))?;

    let component = deserialize_runtime_component(engine)?;
    let linker = build_linker(engine)?;
    let (mut store, runtime) = instantiate(
        engine,
        &component,
        &linker,
        source,
        schema_source,
        Profile::User,
        Some(LlmConfig { model, api_key }),
        0,
    )?;

    let schema_started = Instant::now();
    let schema_result = runtime.call_get_schema(&mut store)?;
    log_trace("get-schema() (incl. schema load)", schema_started);
    let schema_json = match schema_result {
        Ok(json) => json,
        Err(err) => {
            print_skill_error(&err);
            std::process::exit(1);
        }
    };

    let (input_schema, output_schema) = parse_schema_envelope(&schema_json)?;

    let args_json = build_input_args_json(&input_schema, &skill_flag_argv)?;

    let started = Instant::now();
    let r = runtime.call_run(&mut store, &args_json)?;
    log_trace(
        "run() (incl. JSON.parse + skill load + run + stringify)",
        started,
    );

    match r {
        Ok(json) => {
            if let Some(schema) = output_schema.as_ref() {
                let value: serde_json::Value = match serde_json::from_str(&json) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("output validation: invalid JSON from skill: {e}");
                        std::process::exit(1);
                    }
                };
                if let Err(msg) = validator::validate_output(&value, schema) {
                    eprintln!("{msg}");
                    std::process::exit(1);
                }
            }
            println!("{json}");
        }
        Err(err) => {
            print_skill_error(&err);
            std::process::exit(1);
        }
    }

    Ok(())
}

fn parse_schema_envelope(
    schema_json: &str,
) -> Result<(serde_json::Value, Option<serde_json::Value>)> {
    let envelope: serde_json::Value = serde_json::from_str(schema_json)
        .with_context(|| format!("failed to parse schema JSON: {schema_json}"))?;
    let input = envelope
        .get("input")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("schema envelope missing 'input': {schema_json}"))?;
    let output = match envelope.get("output") {
        Some(serde_json::Value::Null) | None => None,
        Some(v) => Some(v.clone()),
    };
    Ok((input, output))
}

fn build_input_args_json(
    input_schema: &serde_json::Value,
    skill_flag_argv: &[String],
) -> Result<String> {
    let stdin_is_tty = io::stdin().is_terminal();
    let result = if stdin_is_tty {
        skill_args::build_args_json(input_schema, skill_flag_argv)
    } else {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .context("failed to read stdin")?;
        let stdin_value: serde_json::Value = if buf.trim().is_empty() {
            serde_json::Value::Object(serde_json::Map::new())
        } else {
            match serde_json::from_str(&buf) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("Error: stdin: invalid JSON: {e}");
                    std::process::exit(2);
                }
            }
        };
        skill_args::build_args_json_with_stdin(input_schema, skill_flag_argv, &stdin_value)
    };
    match result {
        Ok(json) => Ok(json),
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(2);
        }
    }
}

fn parse_run_argv(argv: Vec<String>) -> (PathBuf, String, Vec<String>) {
    let mut skill: Option<PathBuf> = None;
    let mut name: Option<String> = None;
    let mut model: Option<String> = None;
    let mut skill_flags: Vec<String> = Vec::new();

    let mut i = 0;
    if let Some(first) = argv.first()
        && !first.starts_with("--")
    {
        name = Some(first.clone());
        i = 1;
    }
    while i < argv.len() {
        let token = &argv[i];
        match token.as_str() {
            "--skill" => {
                let value = match argv.get(i + 1) {
                    Some(v) => v.clone(),
                    None => {
                        eprintln!("Error: --skill: missing value");
                        std::process::exit(2);
                    }
                };
                skill = Some(PathBuf::from(value));
                i += 2;
            }
            "--model" => {
                let value = match argv.get(i + 1) {
                    Some(v) => v.clone(),
                    None => {
                        eprintln!("Error: --model: missing value");
                        std::process::exit(2);
                    }
                };
                model = Some(value);
                i += 2;
            }
            t if t == "--args" || t.starts_with("--args=") => {
                eprintln!("Error: --args: unknown flag");
                std::process::exit(2);
            }
            _ => {
                skill_flags.push(token.clone());
                i += 1;
            }
        }
    }

    let skill_path = match (skill, name) {
        (Some(_), Some(_)) => {
            eprintln!("Error: <skill-name> and --skill are mutually exclusive");
            std::process::exit(2);
        }
        (Some(path), None) => path,
        (None, Some(n)) => {
            if let Err(msg) = validate_skill_name(&n) {
                eprintln!("Error: <skill-name>: {msg}");
                std::process::exit(2);
            }
            match skill_dir_for_name(&n) {
                Ok(dir) => dir.join("skill.js"),
                Err(e) => {
                    eprintln!("Error: <skill-name>: {e}");
                    std::process::exit(2);
                }
            }
        }
        (None, None) => {
            eprintln!("Error: <skill-name> or --skill: required");
            std::process::exit(2);
        }
    };

    let model = model.unwrap_or_else(|| {
        eprintln!("Error: --model: required");
        std::process::exit(2);
    });

    (skill_path, model, skill_flags)
}

fn schema_path_for(skill_path: &PathBuf) -> PathBuf {
    skill_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(PathBuf::new)
        .join("schema.js")
}

fn run_builtin_skill(
    engine: &Engine,
    skill_source: &str,
    schema_source: &str,
    args_json: &str,
    llm_config: Option<LlmConfig>,
) -> Result<std::result::Result<String, SkillError>> {
    let component = deserialize_runtime_component(engine)?;
    let linker = build_linker(engine)?;
    let (mut store, runtime) = instantiate(
        engine,
        &component,
        &linker,
        skill_source.to_string(),
        schema_source.to_string(),
        Profile::Builtin,
        llm_config,
        0,
    )?;
    let started = Instant::now();
    let r = runtime.call_run(&mut store, args_json)?;
    log_trace("builtin run()", started);
    Ok(r)
}

fn run_generate(engine: &Engine, prompt: &str, name: &str, model: &str, force: bool) -> Result<()> {
    if let Err(msg) = validate_skill_name(name) {
        eprintln!("Error: --name: {msg}");
        std::process::exit(2);
    }
    let resolved_prompt = match resolve_prompt(prompt) {
        Ok(p) => p,
        Err((msg, code)) => {
            eprintln!("Error: {msg}");
            std::process::exit(code);
        }
    };
    let prompt = resolved_prompt.as_str();

    let api_key = env::var("ANTHROPIC_API_KEY")
        .context("ANTHROPIC_API_KEY environment variable is required")?;

    let skill_dir = skill_dir_for_name(name)?;
    if skill_dir.exists() && !force {
        eprintln!(
            "Error: skill directory already exists: {} (use --force to overwrite)",
            skill_dir.display()
        );
        std::process::exit(1);
    }

    println!("generating skill code...");
    let args_json = serde_json::json!({
        "prompt": prompt,
        "model": model,
        "apiKey": api_key,
    })
    .to_string();

    let r = run_builtin_skill(
        engine,
        SKILL_GENERATE_SKILL_CODE_JS,
        SCHEMA_GENERATE_SKILL_CODE_JS,
        &args_json,
        Some(LlmConfig {
            model: model.to_string(),
            api_key: api_key.clone(),
        }),
    )?;
    let json = match r {
        Ok(j) => j,
        Err(err) => {
            eprintln!("agent error: [{:?}] {}", err.code, err.message);
            std::process::exit(1);
        }
    };

    let (code, capabilities, schema) = parse_generated(&json)?;
    if let Err(msg) = generated_schema::validate(&schema) {
        eprintln!("Error: generated schema invalid: {msg}");
        std::process::exit(1);
    }

    if skill_dir.exists() {
        fs::remove_dir_all(&skill_dir).with_context(|| {
            format!(
                "failed to remove existing skill directory: {}",
                skill_dir.display()
            )
        })?;
    }

    write_generated_skill(&skill_dir, prompt, &code, &capabilities, &schema)?;
    print_generate_summary(&skill_dir, &capabilities, name, model);

    Ok(())
}

fn validate_skill_name(name: &str) -> std::result::Result<(), String> {
    if name.is_empty() {
        return Err("skill name must not be empty".into());
    }
    if name == "." || name == ".." {
        return Err("skill name must not be '.' or '..'".into());
    }
    if name.contains('/') || name.contains('\\') {
        return Err("skill name must not contain path separators".into());
    }
    Ok(())
}

fn skill_dir_for_name(name: &str) -> Result<PathBuf> {
    let home = dirs::home_dir().context("failed to determine home directory")?;
    Ok(home.join(".skill-forge").join("skills").join(name))
}

fn parse_generated(json: &str) -> Result<(String, Vec<String>, serde_json::Value)> {
    let v: serde_json::Value = serde_json::from_str(json)
        .with_context(|| format!("failed to parse builtin skill output as JSON: {json}"))?;
    let code = v
        .get("code")
        .and_then(|c| c.as_str())
        .ok_or_else(|| anyhow::anyhow!("generated output missing 'code' string field"))?
        .to_string();
    let capabilities = v
        .get("capabilities")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let schema = v
        .get("schema")
        .ok_or_else(|| anyhow::anyhow!("generated output missing 'schema' field"))?
        .clone();
    if !schema.is_object() {
        anyhow::bail!("generated output 'schema' is not a JSON object");
    }
    Ok((code, capabilities, schema))
}

fn write_generated_skill(
    skill_dir: &Path,
    prompt: &str,
    code: &str,
    capabilities: &[String],
    schema: &serde_json::Value,
) -> Result<()> {
    fs::create_dir_all(skill_dir)
        .with_context(|| format!("failed to create skill directory: {}", skill_dir.display()))?;

    let header = format!("// capabilities: {}\n", capabilities.join(", "));
    let skill_js = format!("{header}{code}");
    let skill_path = skill_dir.join("skill.js");
    fs::write(&skill_path, skill_js)
        .with_context(|| format!("failed to write {}", skill_path.display()))?;

    let schema_json = serde_json::to_string_pretty(schema)
        .context("failed to serialize generated schema as JSON")?;
    let schema_js = format!("defineSchema({schema_json});\n");
    let schema_path = skill_dir.join("schema.js");
    fs::write(&schema_path, schema_js)
        .with_context(|| format!("failed to write {}", schema_path.display()))?;

    let prompt_path = skill_dir.join("PROMPT.md");
    fs::write(&prompt_path, prompt)
        .with_context(|| format!("failed to write {}", prompt_path.display()))?;

    Ok(())
}

fn print_generate_summary(skill_dir: &Path, capabilities: &[String], name: &str, model: &str) {
    println!("wrote {}", skill_dir.join("skill.js").display());
    println!("wrote {}", skill_dir.join("schema.js").display());
    println!("wrote {}", skill_dir.join("PROMPT.md").display());
    println!("capabilities: {}", capabilities.join(", "));
    println!("run with: skill-forge run {name} --model {model}");
}

fn resolve_prompt(raw: &str) -> std::result::Result<String, (String, i32)> {
    if let Some(rest) = raw.strip_prefix("@@") {
        return Ok(format!("@{rest}"));
    }
    if let Some(path_str) = raw.strip_prefix('@') {
        if path_str.is_empty() {
            return Err(("--prompt: path is empty after '@'".to_string(), 2));
        }
        let path = Path::new(path_str);
        let bytes = fs::read(path).map_err(|e| {
            (
                format!("--prompt: failed to read {}: {e}", path.display()),
                1,
            )
        })?;
        let content = String::from_utf8(bytes).map_err(|_| {
            (
                format!("--prompt: file is not valid UTF-8: {}", path.display()),
                1,
            )
        })?;
        if content.trim().is_empty() {
            return Err((format!("--prompt: file is empty: {}", path.display()), 1));
        }
        return Ok(content);
    }
    Ok(raw.to_string())
}

fn print_skill_error(err: &SkillError) {
    eprintln!("[{:?}] {}", err.code, err.message);
    if let Some(stack) = &err.stack {
        eprintln!("stack:\n{stack}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_generated_returns_code_capabilities_and_schema() {
        let json = json!({
            "code": "defineSkill(async () => 'ok');",
            "capabilities": ["callLlm"],
            "schema": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }
        })
        .to_string();
        let (code, caps, schema) = parse_generated(&json).unwrap();
        assert_eq!(code, "defineSkill(async () => 'ok');");
        assert_eq!(caps, vec!["callLlm".to_string()]);
        assert_eq!(schema.get("type").and_then(|v| v.as_str()), Some("object"));
    }

    #[test]
    fn parse_generated_errors_when_schema_missing() {
        let json = json!({
            "code": "defineSkill(async () => 'ok');",
            "capabilities": []
        })
        .to_string();
        let err = parse_generated(&json).unwrap_err().to_string();
        assert!(err.contains("schema"), "unexpected error: {err}");
    }

    #[test]
    fn parse_generated_errors_when_schema_not_object() {
        let json = json!({
            "code": "defineSkill(async () => 'ok');",
            "capabilities": [],
            "schema": "not-an-object"
        })
        .to_string();
        let err = parse_generated(&json).unwrap_err().to_string();
        assert!(err.contains("not a JSON object"), "unexpected error: {err}");
    }

    #[test]
    fn write_generated_skill_writes_schema_with_define_schema_wrapper() {
        let tmp = std::env::temp_dir().join(format!("skill-forge-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let schema = json!({
            "type": "object",
            "properties": { "userName": { "type": "string" } },
            "required": ["userName"],
            "additionalProperties": false
        });
        write_generated_skill(&tmp, "prompt", "code", &["callLlm".into()], &schema).unwrap();
        let schema_js = fs::read_to_string(tmp.join("schema.js")).unwrap();
        assert!(schema_js.starts_with("defineSchema("));
        assert!(schema_js.trim_end().ends_with(");"));
        assert!(schema_js.contains("\"userName\""));
        assert!(schema_js.contains("\"additionalProperties\": false"));
        let _ = fs::remove_dir_all(&tmp);
    }

    fn resolve_prompt_tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "skill-forge-resolve-prompt-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn resolve_prompt_returns_literal_string() {
        assert_eq!(resolve_prompt("hello").unwrap(), "hello");
    }

    #[test]
    fn resolve_prompt_reads_file_content() {
        let dir = resolve_prompt_tmp_dir();
        let path = dir.join("p.md");
        fs::write(&path, "line1\nline2\n").unwrap();
        let raw = format!("@{}", path.display());
        assert_eq!(resolve_prompt(&raw).unwrap(), "line1\nline2\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_prompt_double_at_is_literal_with_single_at() {
        assert_eq!(resolve_prompt("@@literal").unwrap(), "@literal");
        assert_eq!(resolve_prompt("@@").unwrap(), "@");
    }

    #[test]
    fn resolve_prompt_empty_path_after_at_errors_with_exit_2() {
        let (msg, code) = resolve_prompt("@").unwrap_err();
        assert_eq!(code, 2);
        assert!(msg.contains("path is empty"), "unexpected error: {msg}");
    }

    #[test]
    fn resolve_prompt_missing_file_errors_with_exit_1() {
        let dir = resolve_prompt_tmp_dir();
        let path = dir.join("nope.md");
        let raw = format!("@{}", path.display());
        let (msg, code) = resolve_prompt(&raw).unwrap_err();
        assert_eq!(code, 1);
        assert!(msg.contains("failed to read"), "unexpected error: {msg}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_prompt_empty_file_errors_with_exit_1() {
        let dir = resolve_prompt_tmp_dir();
        let path = dir.join("empty.md");
        fs::write(&path, "").unwrap();
        let raw = format!("@{}", path.display());
        let (msg, code) = resolve_prompt(&raw).unwrap_err();
        assert_eq!(code, 1);
        assert!(msg.contains("empty"), "unexpected error: {msg}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_prompt_whitespace_only_file_errors_with_exit_1() {
        let dir = resolve_prompt_tmp_dir();
        let path = dir.join("ws.md");
        fs::write(&path, "  \n\t\n").unwrap();
        let raw = format!("@{}", path.display());
        let (msg, code) = resolve_prompt(&raw).unwrap_err();
        assert_eq!(code, 1);
        assert!(msg.contains("empty"), "unexpected error: {msg}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_prompt_non_utf8_file_errors_with_exit_1() {
        let dir = resolve_prompt_tmp_dir();
        let path = dir.join("bin.md");
        fs::write(&path, [0xff, 0xfe, 0xfd]).unwrap();
        let raw = format!("@{}", path.display());
        let (msg, code) = resolve_prompt(&raw).unwrap_err();
        assert_eq!(code, 1);
        assert!(msg.contains("UTF-8"), "unexpected error: {msg}");
        let _ = fs::remove_dir_all(&dir);
    }
}
