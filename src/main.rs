use std::env;
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use wasmtime::component::{Component, Linker, ResourceTable, bindgen};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView};

mod export;
mod generated_args;
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
use skill_forge::runtime::instruction_loader_host::Host as InstructionLoaderHost;
use skill_forge::runtime::invoke_host::Host as InvokeHost;
use skill_forge::runtime::llm_host::Host as LlmHost;
use skill_forge::runtime::log_host::Host as LogHost;
use skill_forge::runtime::schema_loader_host::Host as SchemaLoaderHost;
use skill_forge::runtime::skill_loader_host::Host as SkillLoaderHost;
use skill_forge::runtime::types::{ErrorCode, Host as TypesHost};

const RUNTIME_CWASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/skill-runtime.cwasm"));

#[allow(dead_code)]
const CODEGEN_SYSTEM_PROMPT: &str = include_str!("../agent/src/lib/SYSTEM_PROMPT.md");

// Tool sources (decisional JS run functions).
const TOOL_CALL_LLM_JS: &str = include_str!("../agent/dist/tools/call-llm/tool.js");
const TOOL_GENERATE_SKILL_CODE_JS: &str =
    include_str!("../agent/dist/tools/generate-skill-code/tool.js");
const TOOL_ECHO_JS: &str = include_str!("../agent/dist/tools/echo/tool.js");
const TOOL_ERROR_JS: &str = include_str!("../agent/dist/tools/error/tool.js");
const TOOL_COMPOSE_JS: &str = include_str!("../agent/dist/tools/compose/tool.js");
const TOOL_VERIFY_REFERENCES_JS: &str =
    include_str!("../agent/dist/tools/verify-references/tool.js");
const TOOL_READ_FILE_JS: &str = include_str!("../agent/dist/tools/read-file/tool.js");
const TOOL_GREP_FILE_JS: &str = include_str!("../agent/dist/tools/grep-file/tool.js");
const TOOL_LOOP_LLM_JS: &str = include_str!("../agent/dist/tools/loop-llm/tool.js");
const TOOL_VIEW_ISSUE_JS: &str = include_str!("../agent/dist/tools/view-issue/tool.js");
const TOOL_VALIDATE_BRANCH_NAME_JS: &str =
    include_str!("../agent/dist/tools/validate-branch-name/tool.js");
const TOOL_PR_MERGE_JS: &str = include_str!("../agent/dist/tools/pr-merge/tool.js");
const TOOL_READ_CONTEXT_JS: &str = include_str!("../agent/dist/tools/read-context/tool.js");
const TOOL_EXPORT_CLAUDE_CODE_SKILL_JS: &str =
    include_str!("../agent/dist/tools/export-claude-code-skill/tool.js");
const TOOL_RENDER_SKILL_MD_JS: &str =
    include_str!("../agent/dist/tools/render-skill-md/tool.js");

const SCHEMA_TOOL_CALL_LLM_JS: &str = include_str!("../agent/dist/tools/call-llm/schema.js");
const SCHEMA_TOOL_GENERATE_SKILL_CODE_JS: &str =
    include_str!("../agent/dist/tools/generate-skill-code/schema.js");
const SCHEMA_TOOL_ECHO_JS: &str = include_str!("../agent/dist/tools/echo/schema.js");
const SCHEMA_TOOL_ERROR_JS: &str = include_str!("../agent/dist/tools/error/schema.js");
const SCHEMA_TOOL_COMPOSE_JS: &str = include_str!("../agent/dist/tools/compose/schema.js");
const SCHEMA_TOOL_VERIFY_REFERENCES_JS: &str =
    include_str!("../agent/dist/tools/verify-references/schema.js");
const SCHEMA_TOOL_READ_FILE_JS: &str = include_str!("../agent/dist/tools/read-file/schema.js");
const SCHEMA_TOOL_GREP_FILE_JS: &str = include_str!("../agent/dist/tools/grep-file/schema.js");
const SCHEMA_TOOL_LOOP_LLM_JS: &str = include_str!("../agent/dist/tools/loop-llm/schema.js");
const SCHEMA_TOOL_VIEW_ISSUE_JS: &str = include_str!("../agent/dist/tools/view-issue/schema.js");
const SCHEMA_TOOL_VALIDATE_BRANCH_NAME_JS: &str =
    include_str!("../agent/dist/tools/validate-branch-name/schema.js");
const SCHEMA_TOOL_PR_MERGE_JS: &str = include_str!("../agent/dist/tools/pr-merge/schema.js");
const SCHEMA_TOOL_READ_CONTEXT_JS: &str =
    include_str!("../agent/dist/tools/read-context/schema.js");
const SCHEMA_TOOL_EXPORT_CLAUDE_CODE_SKILL_JS: &str =
    include_str!("../agent/dist/tools/export-claude-code-skill/schema.js");
const SCHEMA_TOOL_RENDER_SKILL_MD_JS: &str =
    include_str!("../agent/dist/tools/render-skill-md/schema.js");

const DESC_TOOL_CALL_LLM: &str = include_str!("../agent/src/tools/call-llm/DESCRIPTION.md");
const DESC_TOOL_GENERATE_SKILL_CODE: &str =
    include_str!("../agent/src/tools/generate-skill-code/DESCRIPTION.md");
const DESC_TOOL_ECHO: &str = include_str!("../agent/src/tools/echo/DESCRIPTION.md");
const DESC_TOOL_ERROR: &str = include_str!("../agent/src/tools/error/DESCRIPTION.md");
const DESC_TOOL_COMPOSE: &str = include_str!("../agent/src/tools/compose/DESCRIPTION.md");
const DESC_TOOL_VERIFY_REFERENCES: &str =
    include_str!("../agent/src/tools/verify-references/DESCRIPTION.md");
const DESC_TOOL_READ_FILE: &str = include_str!("../agent/src/tools/read-file/DESCRIPTION.md");
const DESC_TOOL_GREP_FILE: &str = include_str!("../agent/src/tools/grep-file/DESCRIPTION.md");
const DESC_TOOL_LOOP_LLM: &str = include_str!("../agent/src/tools/loop-llm/DESCRIPTION.md");
const DESC_TOOL_VIEW_ISSUE: &str = include_str!("../agent/src/tools/view-issue/DESCRIPTION.md");
const DESC_TOOL_VALIDATE_BRANCH_NAME: &str =
    include_str!("../agent/src/tools/validate-branch-name/DESCRIPTION.md");
const DESC_TOOL_PR_MERGE: &str = include_str!("../agent/src/tools/pr-merge/DESCRIPTION.md");
const DESC_TOOL_READ_CONTEXT: &str =
    include_str!("../agent/src/tools/read-context/DESCRIPTION.md");
const DESC_TOOL_EXPORT_CLAUDE_CODE_SKILL: &str =
    include_str!("../agent/src/tools/export-claude-code-skill/DESCRIPTION.md");
const DESC_TOOL_RENDER_SKILL_MD: &str =
    include_str!("../agent/src/tools/render-skill-md/DESCRIPTION.md");

// Skill sources (LLM-loop entries).
const SKILL_IMPLEMENTATION_CHECK_JS: &str =
    include_str!("../agent/dist/skills/implementation-check/skill.js");
const SKILL_ECHO_TASK_JS: &str = include_str!("../agent/dist/skills/echo-task/skill.js");
const SKILL_ISSUE_CHECKOUT_JS: &str =
    include_str!("../agent/dist/skills/issue-checkout/skill.js");
const SKILL_PR_CREATE_JS: &str = include_str!("../agent/dist/skills/pr-create/skill.js");

const SCHEMA_SKILL_IMPLEMENTATION_CHECK_JS: &str =
    include_str!("../agent/dist/skills/implementation-check/schema.js");
const SCHEMA_SKILL_ECHO_TASK_JS: &str = include_str!("../agent/dist/skills/echo-task/schema.js");
const SCHEMA_SKILL_ISSUE_CHECKOUT_JS: &str =
    include_str!("../agent/dist/skills/issue-checkout/schema.js");
const SCHEMA_SKILL_PR_CREATE_JS: &str = include_str!("../agent/dist/skills/pr-create/schema.js");

const DESC_SKILL_IMPLEMENTATION_CHECK: &str =
    include_str!("../agent/src/skills/implementation-check/DESCRIPTION.md");
const DESC_SKILL_ECHO_TASK: &str = include_str!("../agent/src/skills/echo-task/DESCRIPTION.md");
const DESC_SKILL_ISSUE_CHECKOUT: &str =
    include_str!("../agent/src/skills/issue-checkout/DESCRIPTION.md");
const DESC_SKILL_PR_CREATE: &str = include_str!("../agent/src/skills/pr-create/DESCRIPTION.md");

const INSTRUCTION_IMPLEMENTATION_CHECK: &str =
    include_str!("../agent/src/skills/implementation-check/INSTRUCTION.md");
const INSTRUCTION_ISSUE_CHECKOUT: &str =
    include_str!("../agent/src/skills/issue-checkout/INSTRUCTION.md");
const INSTRUCTION_PR_CREATE: &str =
    include_str!("../agent/src/skills/pr-create/INSTRUCTION.md");

const MAX_INVOKE_DEPTH: usize = 8;

const BUILTIN_TOOLS: &[(&str, &str, &str, &str, &str)] = &[
    (
        "call-llm",
        TOOL_CALL_LLM_JS,
        SCHEMA_TOOL_CALL_LLM_JS,
        DESC_TOOL_CALL_LLM,
        "",
    ),
    (
        "generate-skill-code",
        TOOL_GENERATE_SKILL_CODE_JS,
        SCHEMA_TOOL_GENERATE_SKILL_CODE_JS,
        DESC_TOOL_GENERATE_SKILL_CODE,
        "",
    ),
    ("echo", TOOL_ECHO_JS, SCHEMA_TOOL_ECHO_JS, DESC_TOOL_ECHO, ""),
    (
        "error",
        TOOL_ERROR_JS,
        SCHEMA_TOOL_ERROR_JS,
        DESC_TOOL_ERROR,
        "",
    ),
    (
        "compose",
        TOOL_COMPOSE_JS,
        SCHEMA_TOOL_COMPOSE_JS,
        DESC_TOOL_COMPOSE,
        "",
    ),
    (
        "verify-references",
        TOOL_VERIFY_REFERENCES_JS,
        SCHEMA_TOOL_VERIFY_REFERENCES_JS,
        DESC_TOOL_VERIFY_REFERENCES,
        "",
    ),
    (
        "read-file",
        TOOL_READ_FILE_JS,
        SCHEMA_TOOL_READ_FILE_JS,
        DESC_TOOL_READ_FILE,
        "",
    ),
    (
        "grep-file",
        TOOL_GREP_FILE_JS,
        SCHEMA_TOOL_GREP_FILE_JS,
        DESC_TOOL_GREP_FILE,
        "",
    ),
    (
        "loop-llm",
        TOOL_LOOP_LLM_JS,
        SCHEMA_TOOL_LOOP_LLM_JS,
        DESC_TOOL_LOOP_LLM,
        "",
    ),
    (
        "view-issue",
        TOOL_VIEW_ISSUE_JS,
        SCHEMA_TOOL_VIEW_ISSUE_JS,
        DESC_TOOL_VIEW_ISSUE,
        "",
    ),
    (
        "validate-branch-name",
        TOOL_VALIDATE_BRANCH_NAME_JS,
        SCHEMA_TOOL_VALIDATE_BRANCH_NAME_JS,
        DESC_TOOL_VALIDATE_BRANCH_NAME,
        "",
    ),
    (
        "pr-merge",
        TOOL_PR_MERGE_JS,
        SCHEMA_TOOL_PR_MERGE_JS,
        DESC_TOOL_PR_MERGE,
        "",
    ),
    (
        "read-context",
        TOOL_READ_CONTEXT_JS,
        SCHEMA_TOOL_READ_CONTEXT_JS,
        DESC_TOOL_READ_CONTEXT,
        "",
    ),
    (
        "export-claude-code-skill",
        TOOL_EXPORT_CLAUDE_CODE_SKILL_JS,
        SCHEMA_TOOL_EXPORT_CLAUDE_CODE_SKILL_JS,
        DESC_TOOL_EXPORT_CLAUDE_CODE_SKILL,
        "",
    ),
    (
        "render-skill-md",
        TOOL_RENDER_SKILL_MD_JS,
        SCHEMA_TOOL_RENDER_SKILL_MD_JS,
        DESC_TOOL_RENDER_SKILL_MD,
        "",
    ),
];

const BUILTIN_SKILLS: &[(&str, &str, &str, &str, &str)] = &[
    (
        "implementation-check",
        SKILL_IMPLEMENTATION_CHECK_JS,
        SCHEMA_SKILL_IMPLEMENTATION_CHECK_JS,
        DESC_SKILL_IMPLEMENTATION_CHECK,
        INSTRUCTION_IMPLEMENTATION_CHECK,
    ),
    (
        "echo-task",
        SKILL_ECHO_TASK_JS,
        SCHEMA_SKILL_ECHO_TASK_JS,
        DESC_SKILL_ECHO_TASK,
        "",
    ),
    (
        "issue-checkout",
        SKILL_ISSUE_CHECKOUT_JS,
        SCHEMA_SKILL_ISSUE_CHECKOUT_JS,
        DESC_SKILL_ISSUE_CHECKOUT,
        INSTRUCTION_ISSUE_CHECKOUT,
    ),
    (
        "pr-create",
        SKILL_PR_CREATE_JS,
        SCHEMA_SKILL_PR_CREATE_JS,
        DESC_SKILL_PR_CREATE,
        INSTRUCTION_PR_CREATE,
    ),
];

fn lookup_builtin_tool(name: &str) -> Option<(&'static str, &'static str, &'static str)> {
    BUILTIN_TOOLS
        .iter()
        .find(|(n, _, _, _, _)| *n == name)
        .map(|(_, src, schema, _, instruction)| (*src, *schema, *instruction))
}

fn lookup_builtin_skill(name: &str) -> Option<(&'static str, &'static str, &'static str)> {
    BUILTIN_SKILLS
        .iter()
        .find(|(n, _, _, _, _)| *n == name)
        .map(|(_, src, schema, _, instruction)| (*src, *schema, *instruction))
}

/// Look up by name in tools first, then skills. Used by host functions
/// (invokeSkill, schema loader) that can reach either registry.
fn lookup_builtin_entry(name: &str) -> Option<(&'static str, &'static str, &'static str)> {
    lookup_builtin_tool(name).or_else(|| lookup_builtin_skill(name))
}

fn lookup_builtin_description(name: &str) -> Option<&'static str> {
    BUILTIN_TOOLS
        .iter()
        .chain(BUILTIN_SKILLS.iter())
        .find(|(n, _, _, _, _)| *n == name)
        .map(|(_, _, _, desc, _)| *desc)
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
#[command(name = "forge", about = "forge host")]
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
        #[arg(long, value_enum)]
        backend: Option<Backend>,
        #[arg(long)]
        timeout: Option<u64>,
    },
    #[command(hide = true)]
    McpServer {
        #[arg(long, value_enum, default_value_t = McpMode::Codegen)]
        mode: McpMode,
    },
    List,
    Export {
        #[arg(
            long,
            value_delimiter = ',',
            default_value = "claude-code",
            help = "Comma-separated target names (default: claude-code)"
        )]
        target: Vec<String>,
        #[arg(long, default_value_t = false)]
        force: bool,
    },
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, ValueEnum)]
enum Backend {
    Api,
    Claude,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, ValueEnum)]
pub enum McpMode {
    Codegen,
    Skills,
}

const DEFAULT_TIMEOUT_SECS: u64 = 60;

#[derive(Copy, Clone, Eq, PartialEq)]
enum Profile {
    User,
    Builtin,
}

#[derive(Clone)]
struct LlmConfig {
    model: String,
    api_key: String,
    backend: Backend,
    timeout: Duration,
}

struct SkillState {
    ctx: WasiCtx,
    table: ResourceTable,
    skill_source: String,
    schema_source: String,
    instruction_source: String,
    profile: Profile,
    llm_config: Option<LlmConfig>,
    engine: Engine,
    component: Component,
    depth: usize,
    verbose: bool,
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

    fn get_description(&mut self, skill_name: String) -> wasmtime::Result<String> {
        match lookup_builtin_description(&skill_name) {
            Some(desc) => Ok(desc.to_string()),
            None => Err(anyhow::anyhow!(
                "unknown skill: {} (no description registered)",
                skill_name
            )),
        }
    }
}

impl SchemaLoaderHost for SkillState {
    fn get_schema_source(&mut self) -> wasmtime::Result<String> {
        Ok(self.schema_source.clone())
    }

    fn get_input_schema_json(&mut self, skill_name: String) -> wasmtime::Result<String> {
        let (source, schema_source, instruction_source) = match lookup_builtin_entry(&skill_name) {
            Some(s) => s,
            None => {
                return Err(anyhow::anyhow!(
                    "unknown skill: {} (cannot resolve input schema)",
                    skill_name
                ));
            }
        };
        let envelope = load_skill_schema_envelope(
            &self.engine,
            &self.component,
            &skill_name,
            source.to_string(),
            schema_source.to_string(),
            instruction_source.to_string(),
            Profile::Builtin,
            self.llm_config.clone(),
            self.depth + 1,
            self.verbose,
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        let json = serde_json::to_string(&envelope.input)
            .map_err(|e| anyhow::anyhow!("failed to serialize input schema: {e}"))?;
        Ok(json)
    }
}

impl InstructionLoaderHost for SkillState {
    fn get_instruction(&mut self) -> wasmtime::Result<String> {
        Ok(self.instruction_source.clone())
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
        let model = normalize_model(&cfg.model);
        let result = match cfg.backend {
            Backend::Api => host_call_llm(&prompt, &input_json, &model, &cfg.api_key, cfg.timeout),
            Backend::Claude => claude_call_llm(&prompt, &input_json, &model, cfg.timeout),
        };
        Ok(result)
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
        let (source, schema_source, instruction_source) = match lookup_builtin_entry(&skill_name) {
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
        let verbose = self.verbose;
        let (mut store, runtime) = instantiate(
            &engine,
            &component,
            &linker,
            source.to_string(),
            schema_source.to_string(),
            instruction_source.to_string(),
            Profile::Builtin,
            llm_config,
            next_depth,
            verbose,
        )
        .map_err(|e| anyhow::anyhow!("failed to instantiate skill for invoke: {e}"))?;
        let started = Instant::now();
        let r = runtime.call_run(&mut store, &args_json, None)?;
        log_trace("invoke run()", started);
        Ok(r)
    }
}

impl LogHost for SkillState {
    fn log(&mut self, message: String) -> wasmtime::Result<()> {
        if self.verbose {
            eprintln!("{message}");
        }
        Ok(())
    }
}

impl AnthropicHost for SkillState {
    fn messages(
        &mut self,
        body_json: String,
    ) -> wasmtime::Result<std::result::Result<String, String>> {
        if self.profile != Profile::Builtin {
            return Err(anyhow::anyhow!(
                "capability-denied: anthropic-host is not available to user skills"
            ));
        }
        let cfg = match self.llm_config.as_ref() {
            Some(c) => c,
            None => {
                return Ok(Err(
                    "capability-denied: anthropic-host is not configured".to_string()
                ));
            }
        };
        let started = Instant::now();
        let r = anthropic_messages_blocking(&body_json, &cfg.api_key, cfg.timeout);
        log_trace("anthropic-host roundtrip", started);
        Ok(r)
    }
}

pub(crate) fn host_call_llm(
    prompt: &str,
    input_json: &str,
    model: &str,
    api_key: &str,
    timeout: Duration,
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
    let raw = anthropic_messages_blocking(&body, api_key, timeout)?;
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

fn claude_call_llm(
    prompt: &str,
    input_json: &str,
    model: &str,
    timeout: Duration,
) -> std::result::Result<String, String> {
    const BENCH_MOCK_PROMPT: &str = "__BENCH_MOCK__";
    if prompt == BENCH_MOCK_PROMPT {
        return Ok(input_json.to_string());
    }
    let combined = format!("[system]\n{prompt}\n\n[user]\n{input_json}");
    let args = vec![
        "-p".to_string(),
        "--model".to_string(),
        model.to_string(),
        combined,
    ];
    run_with_timeout("claude", &args, timeout)
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

fn anthropic_messages_blocking(
    body: &str,
    api_key: &str,
    timeout: Duration,
) -> std::result::Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| format!("network-error: failed to build client: {e}"))?;
    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("content-type", "application/json")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .body(body.to_string())
        .send()
        .map_err(|e| {
            if e.is_timeout() {
                format!(
                    "timeout: anthropic-host did not respond within {:?}",
                    timeout
                )
            } else {
                format!("network-error: {e}")
            }
        })?;
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
            backend,
            timeout,
        } => {
            let backend = resolve_backend(backend);
            let timeout = resolve_timeout(timeout);
            run_generate(&engine, &prompt, &name, &model, force, backend, timeout)
        }
        Command::McpServer { mode } => mcp::run(mode, &engine),
        Command::List => run_list(),
        Command::Export { target, force } => run_export(&engine, target, force),
    }
}

const LIST_BULLET: &str = "• ";
const LIST_DESC_SEP: &str = "  - ";
const LIST_TRUNCATION_MARKER: &str = "...";

fn run_list() -> Result<()> {
    let mut entries: Vec<(String, &'static str, String)> = BUILTIN_SKILLS
        .iter()
        .map(|(name, _, _, desc, _)| {
            ((*name).to_string(), "builtin", first_line(desc).to_string())
        })
        .collect();

    for name in collect_user_skill_names()? {
        entries.push((name, "user", String::new()));
    }

    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let max_label_width = entries
        .iter()
        .map(|(name, kind, _)| label_width(name, kind))
        .max()
        .unwrap_or(0);

    let term_width = if io::stdout().is_terminal() {
        terminal_size::terminal_size().map(|(terminal_size::Width(w), _)| w as usize)
    } else {
        None
    };

    for (name, kind, desc) in entries {
        let label = format!("{LIST_BULLET}{name} ({kind})");
        if desc.is_empty() {
            println!("{label}");
            continue;
        }
        let pad = max_label_width.saturating_sub(label_width(&name, kind));
        let padding: String = " ".repeat(pad);
        let desc_to_print = match term_width {
            Some(width) => {
                let prefix_chars = max_label_width + LIST_DESC_SEP.chars().count();
                let available = width.saturating_sub(prefix_chars);
                truncate_with_marker(&desc, available)
            }
            None => desc.clone(),
        };
        println!("{label}{padding}{LIST_DESC_SEP}{desc_to_print}");
    }

    Ok(())
}

fn label_width(name: &str, kind: &str) -> usize {
    LIST_BULLET.chars().count() + name.chars().count() + 2 + kind.chars().count() + 1
}

fn truncate_with_marker(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        return s.to_string();
    }
    let marker_len = LIST_TRUNCATION_MARKER.chars().count();
    if max_chars <= marker_len {
        return s.chars().take(max_chars).collect();
    }
    let mut out: String = s.chars().take(max_chars - marker_len).collect();
    out.push_str(LIST_TRUNCATION_MARKER);
    out
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("").trim()
}

fn collect_user_skill_names() -> Result<Vec<String>> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Ok(Vec::new()),
    };
    let dir = home.join(".skill-forge").join("skills");
    let read_dir = match fs::read_dir(&dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(anyhow::Error::from(e))
                .with_context(|| format!("failed to read user skills dir: {}", dir.display()));
        }
    };

    let mut names = Vec::new();
    for entry in read_dir {
        let entry = entry
            .with_context(|| format!("failed to read entry under {}", dir.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to stat {}", entry.path().display()))?;
        if !file_type.is_dir() {
            continue;
        }
        if !entry.path().join("skill.js").is_file() {
            continue;
        }
        if let Some(name) = entry.file_name().to_str() {
            names.push(name.to_string());
        }
    }
    Ok(names)
}

/// Source of a skill entry being exported. Builtin entries live in the binary
/// (`BUILTIN_SKILLS`); user entries live under `~/.skill-forge/skills/<name>/`.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub(crate) enum ExportSkillKind {
    Builtin,
    User,
}

fn exports_root() -> Result<PathBuf> {
    let home = dirs::home_dir().context("failed to determine home directory")?;
    Ok(home.join(".skill-forge").join("exports"))
}

fn manifest_path() -> Result<PathBuf> {
    Ok(exports_root()?.join(".manifest.json"))
}

fn validate_target_name(name: &str) -> std::result::Result<(), String> {
    if name.is_empty() {
        return Err("target name must not be empty".into());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!(
            "target name '{name}' must be ASCII alphanumeric, '-' or '_'"
        ));
    }
    Ok(())
}

/// Collect every skill that `forge export` should process, merging builtin
/// (compiled-in) skills with user skills under `~/.skill-forge/skills/`.
///
/// Errors out on builtin × user name collision, matching the spec in #158:
/// `forge run` lookup remains builtin-priority but export surfaces collisions
/// loudly so the silent shadowing is not propagated to the symlinks.
fn collect_export_set() -> Result<Vec<(String, ExportSkillKind)>> {
    use std::collections::BTreeMap;

    let mut entries: BTreeMap<String, ExportSkillKind> = BTreeMap::new();
    for (name, _, _, _, _) in BUILTIN_SKILLS {
        entries.insert((*name).to_string(), ExportSkillKind::Builtin);
    }

    let mut collisions: Vec<String> = Vec::new();
    for name in collect_user_skill_names()? {
        if entries.contains_key(&name) {
            collisions.push(name);
        } else {
            entries.insert(name, ExportSkillKind::User);
        }
    }

    if !collisions.is_empty() {
        anyhow::bail!(
            "skill name collision between builtin and user: {}\n\
             rename your user skill(s) under ~/.skill-forge/skills/ to resolve",
            collisions.join(", ")
        );
    }

    Ok(entries.into_iter().collect())
}

fn run_export(engine: &Engine, target: Vec<String>, force: bool) -> Result<()> {
    let targets: Vec<String> = target.into_iter().map(|t| t.trim().to_string()).collect();
    if targets.is_empty() {
        anyhow::bail!("--target: at least one target name required");
    }
    for t in &targets {
        if let Err(msg) = validate_target_name(t) {
            anyhow::bail!("--target: {msg}");
        }
    }

    let skills = collect_export_set()?;
    let root = exports_root()?;
    let component = deserialize_runtime_component(engine)?;
    let home = dirs::home_dir().context("failed to determine home directory")?;

    fs::create_dir_all(&root)
        .with_context(|| format!("failed to create exports root: {}", root.display()))?;

    let manifest_p = manifest_path()?;
    let prior_manifest = read_prior_manifest(&manifest_p)?;

    eprintln!("forge export → {}", root.display());
    eprintln!("  targets: {}", targets.join(", "));

    for (name, kind) in &skills {
        write_canonical_for_skill(engine, &component, &root, name, *kind)?;
        eprintln!("  exported skill: {name}");
    }

    let mut placements: Vec<export::ManifestPlacement> = Vec::new();
    for target in &targets {
        let tool_name = format!("export-{target}-skill");
        if lookup_builtin_tool(&tool_name).is_none() {
            anyhow::bail!(
                "no builtin tool '{tool_name}' for target '{target}'; \
                 add agent/src/tools/{tool_name}/ to support this target"
            );
        }
        for (skill_name, _) in &skills {
            let dest = resolve_target_destination(
                engine,
                &component,
                &tool_name,
                skill_name,
                &home,
            )?;
            let canonical = root.join(skill_name);
            place_symlink(&canonical, &dest, &root, force)?;
            placements.push(export::ManifestPlacement {
                target: target.clone(),
                name: skill_name.clone(),
                dest,
            });
        }
    }

    reconcile_orphans(&prior_manifest, &placements, &skills, &root, &targets)?;

    let manifest = export::ExportManifest {
        skills: skills.iter().map(|(n, _)| n.clone()).collect(),
        placements,
    };
    let manifest_json = serde_json::to_string_pretty(&manifest.to_json())
        .context("failed to serialize export manifest")?;
    fs::write(&manifest_p, manifest_json + "\n")
        .with_context(|| format!("failed to write manifest: {}", manifest_p.display()))?;
    eprintln!("  wrote manifest: {}", manifest_p.display());

    Ok(())
}

/// Read the prior `.manifest.json` if it exists. Returns `None` for first-time
/// runs (no file). Errors out on unknown/malformed manifest so users notice and
/// can clear it manually rather than getting silent half-states.
fn read_prior_manifest(path: &Path) -> Result<Option<export::ExportManifest>> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(Some(
            export::ExportManifest::from_json_str(&s).map_err(|e| anyhow::anyhow!("{e}"))?,
        )),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(anyhow::Error::from(e))
            .with_context(|| format!("failed to read manifest: {}", path.display())),
    }
}

/// Remove forge-owned artifacts (symlinks + canonical export dirs) that the
/// prior manifest knew about but the current run no longer covers.
///
/// Two distinct "orphan" sources:
///
/// 1. Prior placements whose `(target, name)` is not in the new placement set
///    — e.g. user deleted a user skill, or invoked `--target` with a smaller
///    subset of targets than last time. These have their forge-owned symlink
///    removed (real dirs / foreign symlinks are left untouched as a safety
///    net).
/// 2. Skills present in the prior manifest's `skills` field but absent from
///    the current `collect_export_set()` — their canonical `exports/<name>/`
///    dir is removed so stale SKILL.md/DESCRIPTION.md don't linger.
fn reconcile_orphans(
    prior: &Option<export::ExportManifest>,
    new_placements: &[export::ManifestPlacement],
    new_skills: &[(String, ExportSkillKind)],
    exports_root: &Path,
    active_targets: &[String],
) -> Result<()> {
    let Some(prior) = prior.as_ref() else {
        return Ok(());
    };

    let new_keys: std::collections::HashSet<(String, String)> = new_placements
        .iter()
        .map(|p| (p.target.clone(), p.name.clone()))
        .collect();
    let active_target_set: std::collections::HashSet<&str> =
        active_targets.iter().map(String::as_str).collect();

    for prior_p in &prior.placements {
        let key = (prior_p.target.clone(), prior_p.name.clone());
        if new_keys.contains(&key) {
            continue;
        }
        // Only touch this placement if the user actually asked us to manage
        // its target this run — otherwise leave it alone (the user can clean
        // it up by re-running with the missing target included).
        if !active_target_set.contains(prior_p.target.as_str()) {
            continue;
        }
        remove_forge_owned_symlink(&prior_p.dest, exports_root)?;
    }

    let new_skill_names: std::collections::HashSet<&str> =
        new_skills.iter().map(|(n, _)| n.as_str()).collect();
    for stale_name in prior.skills.iter() {
        if new_skill_names.contains(stale_name.as_str()) {
            continue;
        }
        let stale_dir = exports_root.join(stale_name);
        match fs::symlink_metadata(&stale_dir) {
            Ok(meta) if meta.is_dir() => {
                fs::remove_dir_all(&stale_dir).with_context(|| {
                    format!("failed to remove stale export dir: {}", stale_dir.display())
                })?;
                eprintln!("  reconciled: removed exports/{stale_name}/");
            }
            Ok(_) => {} // not a dir — leave alone
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(anyhow::Error::from(e)).with_context(|| {
                    format!("failed to stat stale export dir: {}", stale_dir.display())
                });
            }
        }
    }

    Ok(())
}

fn remove_forge_owned_symlink(dest: &Path, exports_root: &Path) -> Result<()> {
    #[cfg(not(unix))]
    {
        let _ = (dest, exports_root);
        return Ok(());
    }
    #[cfg(unix)]
    {
        match fs::symlink_metadata(dest) {
            Ok(meta) => {
                if !meta.file_type().is_symlink() {
                    return Ok(());
                }
                if !is_forge_owned_symlink(dest, exports_root) {
                    return Ok(());
                }
                fs::remove_file(dest).with_context(|| {
                    format!("failed to remove orphan symlink: {}", dest.display())
                })?;
                eprintln!("  reconciled: removed {}", dest.display());
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(anyhow::Error::from(e))
                    .with_context(|| format!("failed to stat orphan: {}", dest.display()));
            }
        }
        Ok(())
    }
}

fn resolve_target_destination(
    engine: &Engine,
    component: &Component,
    tool_name: &str,
    skill_name: &str,
    home: &Path,
) -> Result<PathBuf> {
    let input = serde_json::json!({
        "skillName": skill_name,
        "homeDir": home.to_string_lossy(),
    });
    let output_json = invoke_builtin_tool_run(engine, component, tool_name, &input.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&output_json)
        .with_context(|| format!("tool '{tool_name}' returned non-JSON output: {output_json}"))?;
    let dest = v
        .get("destPath")
        .and_then(|d| d.as_str())
        .ok_or_else(|| anyhow::anyhow!("tool '{tool_name}' output missing 'destPath' string"))?;
    Ok(PathBuf::from(dest))
}

fn invoke_builtin_tool_run(
    engine: &Engine,
    component: &Component,
    name: &str,
    input_json: &str,
) -> Result<String> {
    let (src, schema, instr) = lookup_builtin_tool(name)
        .ok_or_else(|| anyhow::anyhow!("builtin tool '{name}' not found"))?;
    let linker = build_linker(engine)
        .with_context(|| format!("failed to build linker to invoke '{name}'"))?;
    let (mut store, runtime) = instantiate(
        engine,
        component,
        &linker,
        src.to_string(),
        schema.to_string(),
        instr.to_string(),
        Profile::Builtin,
        None,
        0,
        false,
    )
    .with_context(|| format!("failed to instantiate '{name}'"))?;
    match runtime.call_run(&mut store, input_json, None)? {
        Ok(json) => Ok(json),
        Err(err) => anyhow::bail!("tool '{name}' failed: {}", err.message),
    }
}

/// Place a symlink at `dest` pointing at `canonical`. Existing entries are
/// handled per the conflict policy:
/// - forge-owned symlink (resolves into `exports_root`) → re-pointed silently
/// - real dir / file / foreign symlink → error unless `force`; with `force`
///   the existing entry is renamed to `<dest>.bak.<unix-nanos>` before the
///   new symlink is installed.
fn place_symlink(
    canonical: &Path,
    dest: &Path,
    exports_root: &Path,
    force: bool,
) -> Result<()> {
    #[cfg(not(unix))]
    {
        let _ = (canonical, dest, exports_root, force);
        anyhow::bail!("`forge export` symlink placement is not supported on this platform");
    }

    #[cfg(unix)]
    {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create symlink parent: {}", parent.display())
            })?;
        }

        match fs::symlink_metadata(dest) {
            Ok(meta) => {
                let file_type = meta.file_type();
                if file_type.is_symlink() && is_forge_owned_symlink(dest, exports_root) {
                    fs::remove_file(dest).with_context(|| {
                        format!("failed to remove old forge symlink: {}", dest.display())
                    })?;
                } else if force {
                    backup_existing(dest)?;
                } else {
                    anyhow::bail!(
                        "{} already exists and is not a forge-owned symlink; \
                         rerun with --force to back it up to <name>.bak.<ts>",
                        dest.display()
                    );
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(anyhow::Error::from(e))
                    .with_context(|| format!("failed to stat {}", dest.display()));
            }
        }

        std::os::unix::fs::symlink(canonical, dest).with_context(|| {
            format!(
                "failed to symlink {} -> {}",
                dest.display(),
                canonical.display()
            )
        })?;
        Ok(())
    }
}

#[cfg(unix)]
fn is_forge_owned_symlink(dest: &Path, exports_root: &Path) -> bool {
    let Ok(link_target) = fs::read_link(dest) else {
        return false;
    };
    let resolved = if link_target.is_absolute() {
        link_target.clone()
    } else if let Some(parent) = dest.parent() {
        parent.join(&link_target)
    } else {
        link_target.clone()
    };
    let canon_resolved = fs::canonicalize(&resolved).ok();
    let canon_root = fs::canonicalize(exports_root).ok();
    match (canon_resolved, canon_root) {
        (Some(r), Some(root)) => r.starts_with(&root),
        _ => false,
    }
}

#[cfg(unix)]
fn backup_existing(dest: &Path) -> Result<()> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let bak_name = format!(
        "{}.bak.{ts}",
        dest.file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "skill".to_string())
    );
    let bak_path = dest
        .parent()
        .map(|p| p.join(&bak_name))
        .unwrap_or_else(|| PathBuf::from(&bak_name));
    fs::rename(dest, &bak_path).with_context(|| {
        format!(
            "failed to back up {} -> {}",
            dest.display(),
            bak_path.display()
        )
    })?;
    eprintln!("  backed up existing {} → {}", dest.display(), bak_path.display());
    Ok(())
}

/// Render and persist `~/.skill-forge/exports/<name>/SKILL.md` and
/// `DESCRIPTION.md` for one skill. Overwrites unconditionally — exports/ is
/// owned by forge so no `--force` is needed here.
///
/// The rendering logic itself lives in the `render-skill-md` builtin tool
/// (TypeScript). The host gathers the skill's metadata + schema envelope,
/// invokes the tool, and writes the returned strings to disk.
fn write_canonical_for_skill(
    engine: &Engine,
    component: &Component,
    exports_root: &Path,
    name: &str,
    kind: ExportSkillKind,
) -> Result<()> {
    let description = read_skill_description(name, kind)?;
    let (src, schema_src, instr_src, profile) = load_export_skill_artifacts(name, kind)?;
    let envelope = load_skill_schema_envelope(
        engine,
        component,
        name,
        src,
        schema_src,
        instr_src,
        profile,
        None, // schema fetch does not require an LLM
        0,
        false,
    )?;
    let positional_prop = infer_positional_prop(envelope.args.as_ref());

    let mut input = serde_json::Map::new();
    input.insert("name".into(), serde_json::Value::String(name.to_string()));
    input.insert(
        "description".into(),
        serde_json::Value::String(description.clone()),
    );
    input.insert("inputSchema".into(), envelope.input.clone());
    if let Some(out_schema) = envelope.output.as_ref() {
        input.insert("outputSchema".into(), out_schema.clone());
    }
    if let Some(prop) = positional_prop.as_deref() {
        input.insert(
            "positionalProp".into(),
            serde_json::Value::String(prop.to_string()),
        );
    }
    let input_json = serde_json::Value::Object(input).to_string();

    let output_json =
        invoke_builtin_tool_run(engine, component, "render-skill-md", &input_json)?;
    let v: serde_json::Value = serde_json::from_str(&output_json).with_context(|| {
        format!("render-skill-md returned non-JSON output for {name}: {output_json}")
    })?;
    let skill_md = v
        .get("skillMd")
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow::anyhow!("render-skill-md output missing 'skillMd' for {name}"))?
        .to_string();
    let description_md = v
        .get("descriptionMd")
        .and_then(|x| x.as_str())
        .ok_or_else(|| {
            anyhow::anyhow!("render-skill-md output missing 'descriptionMd' for {name}")
        })?
        .to_string();

    let skill_dir = exports_root.join(name);
    fs::create_dir_all(&skill_dir)
        .with_context(|| format!("failed to create export dir: {}", skill_dir.display()))?;
    fs::write(skill_dir.join("SKILL.md"), skill_md)
        .with_context(|| format!("failed to write SKILL.md for {name}"))?;
    fs::write(skill_dir.join("DESCRIPTION.md"), description_md)
        .with_context(|| format!("failed to write DESCRIPTION.md for {name}"))?;
    Ok(())
}

fn read_skill_description(name: &str, kind: ExportSkillKind) -> Result<String> {
    match kind {
        ExportSkillKind::Builtin => Ok(lookup_builtin_description(name)
            .ok_or_else(|| anyhow::anyhow!("builtin skill not found: {name}"))?
            .to_string()),
        ExportSkillKind::User => {
            let dir = skill_dir_for_name(name)?;
            let path = dir.join("DESCRIPTION.md");
            fs::read_to_string(&path)
                .with_context(|| format!("failed to read user DESCRIPTION.md: {}", path.display()))
        }
    }
}

fn resolve_backend(flag: Option<Backend>) -> Backend {
    if let Some(b) = flag {
        return b;
    }
    if let Ok(value) = env::var("SKILL_FORGE_BACKEND") {
        return match value.as_str() {
            "api" => Backend::Api,
            "claude" => Backend::Claude,
            other => {
                eprintln!(
                    "Error: SKILL_FORGE_BACKEND: invalid value '{other}' (expected 'api' or 'claude')"
                );
                std::process::exit(2);
            }
        };
    }
    if which_claude_present() {
        Backend::Claude
    } else {
        Backend::Api
    }
}

fn which_claude_present() -> bool {
    std::process::Command::new("which")
        .arg("claude")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn resolve_timeout(flag: Option<u64>) -> Duration {
    let secs = flag
        .or_else(|| {
            env::var("SKILL_FORGE_TIMEOUT")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
        })
        .unwrap_or(DEFAULT_TIMEOUT_SECS);
    Duration::from_secs(secs)
}

fn normalize_model(model: &str) -> String {
    match model {
        "sonnet" => "claude-sonnet-4-6".to_string(),
        "opus" => "claude-opus-4-7".to_string(),
        "haiku" => "claude-haiku-4-5".to_string(),
        _ => model.to_string(),
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
    instruction_source: String,
    profile: Profile,
    llm_config: Option<LlmConfig>,
    depth: usize,
    verbose: bool,
) -> Result<(Store<SkillState>, SkillRuntime)> {
    let state = SkillState {
        ctx: WasiCtxBuilder::new().inherit_stdio().build(),
        table: ResourceTable::new(),
        skill_source,
        schema_source,
        instruction_source,
        profile,
        llm_config,
        engine: engine.clone(),
        component: component.clone(),
        depth,
        verbose,
    };
    let mut store = Store::new(engine, state);
    let started = Instant::now();
    let runtime = SkillRuntime::instantiate(&mut store, component, linker)?;
    log_trace("runtime instantiate (incl. main.js eval)", started);
    Ok((store, runtime))
}

fn run_skill_run(engine: &Engine, raw_argv: Vec<String>) -> Result<()> {
    let RunArgs {
        skill_source,
        model,
        backend,
        timeout,
        verbose,
        help,
        skill_flags: skill_flag_argv,
    } = parse_run_argv(raw_argv);

    if help {
        return run_help(engine, skill_source);
    }

    let skill_source = skill_source.expect("skill_source is None only when help is true");

    let api_key = match backend {
        Backend::Api => env::var("ANTHROPIC_API_KEY")
            .context("ANTHROPIC_API_KEY environment variable is required")?,
        Backend::Claude => env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
    };

    let (source, schema_source, instruction_source, profile) = load_skill_sources(&skill_source)?;

    let component = deserialize_runtime_component(engine)?;
    let linker = build_linker(engine)?;
    let (mut store, runtime) = instantiate(
        engine,
        &component,
        &linker,
        source,
        schema_source,
        instruction_source,
        profile,
        Some(LlmConfig {
            model,
            api_key,
            backend,
            timeout,
        }),
        0,
        verbose,
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

    let SchemaEnvelope {
        input: input_schema,
        output: output_schema,
        args: args_spec,
    } = parse_schema_envelope(&schema_json)?;

    if let Some(args_spec) = args_spec.as_ref() {
        if let Err(msg) = generated_args::validate(args_spec, &input_schema) {
            eprintln!("Error: {msg}");
            std::process::exit(1);
        }
    }
    let positional_prop = infer_positional_prop(args_spec.as_ref());

    let args_json = build_input_args_json(&input_schema, positional_prop.as_deref(), &skill_flag_argv)?;
    let context = read_stdin_context()?;

    let started = Instant::now();
    let r = runtime.call_run(&mut store, &args_json, context.as_deref())?;
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

struct SchemaEnvelope {
    input: serde_json::Value,
    output: Option<serde_json::Value>,
    args: Option<serde_json::Value>,
}

fn parse_schema_envelope(schema_json: &str) -> Result<SchemaEnvelope> {
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
    let args = match envelope.get("args") {
        Some(serde_json::Value::Null) | None => None,
        Some(v) => Some(v.clone()),
    };
    Ok(SchemaEnvelope {
        input,
        output,
        args,
    })
}

/// Read the `positional` property name from a generated-args spec, if present.
/// Single shared implementation used by `run_skill_run`, the `--help` formatter,
/// and `forge export` so the example invocations stay in sync.
fn infer_positional_prop(args_spec: Option<&serde_json::Value>) -> Option<String> {
    args_spec
        .and_then(|a| a.get("positional"))
        .and_then(|p| p.as_str())
        .map(|s| s.to_string())
}

/// Instantiate a fresh skill instance, call its `get_schema`, and parse the envelope.
/// Used by the `SchemaLoaderHost::get_input_schema_json` host fn and by `forge export`.
#[allow(clippy::too_many_arguments)]
fn load_skill_schema_envelope(
    engine: &Engine,
    component: &Component,
    skill_name: &str,
    skill_source: String,
    schema_source: String,
    instruction_source: String,
    profile: Profile,
    llm_config: Option<LlmConfig>,
    depth: usize,
    verbose: bool,
) -> Result<SchemaEnvelope> {
    let linker = build_linker(engine)
        .with_context(|| format!("failed to build linker for schema lookup of {skill_name}"))?;
    let (mut store, runtime) = instantiate(
        engine,
        component,
        &linker,
        skill_source,
        schema_source,
        instruction_source,
        profile,
        llm_config,
        depth,
        verbose,
    )
    .with_context(|| format!("failed to instantiate skill for schema lookup of {skill_name}"))?;
    let envelope_json = match runtime.call_get_schema(&mut store)? {
        Ok(json) => json,
        Err(err) => {
            anyhow::bail!("get_schema failed for {skill_name}: {}", err.message);
        }
    };
    parse_schema_envelope(&envelope_json)
}

/// Resolve a skill (builtin or user) to its (source, schema, instruction, profile)
/// artifacts so the wasm runtime can be instantiated for it.
#[allow(dead_code)] // wired in Phase 4
fn load_export_skill_artifacts(
    name: &str,
    kind: ExportSkillKind,
) -> Result<(String, String, String, Profile)> {
    match kind {
        ExportSkillKind::Builtin => {
            let (src, schema, instr) = lookup_builtin_skill(name)
                .ok_or_else(|| anyhow::anyhow!("builtin skill not found: {name}"))?;
            Ok((
                src.to_string(),
                schema.to_string(),
                instr.to_string(),
                Profile::Builtin,
            ))
        }
        ExportSkillKind::User => {
            let dir = skill_dir_for_name(name)?;
            let skill_path = dir.join("skill.js");
            let source = fs::read_to_string(&skill_path).with_context(|| {
                format!("failed to read user skill source: {}", skill_path.display())
            })?;
            let schema_path = schema_path_for(&skill_path);
            let schema_source = fs::read_to_string(&schema_path).with_context(|| {
                format!("failed to read user schema source: {}", schema_path.display())
            })?;
            let instruction_path = instruction_path_for(&skill_path);
            let instruction_source = match fs::read_to_string(&instruction_path) {
                Ok(s) => s,
                Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "failed to read user instruction source: {}: {e}",
                        instruction_path.display()
                    ));
                }
            };
            Ok((source, schema_source, instruction_source, Profile::User))
        }
    }
}

fn build_input_args_json(
    input_schema: &serde_json::Value,
    positional_prop: Option<&str>,
    skill_flag_argv: &[String],
) -> Result<String> {
    match skill_args::build_args_json(input_schema, positional_prop, skill_flag_argv) {
        Ok(json) => Ok(json),
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(2);
        }
    }
}

fn read_stdin_context() -> Result<Option<String>> {
    if io::stdin().is_terminal() {
        return Ok(None);
    }
    let mut buf = String::new();
    io::stdin()
        .read_to_string(&mut buf)
        .context("failed to read stdin")?;
    if buf.is_empty() {
        Ok(None)
    } else {
        Ok(Some(buf))
    }
}

enum SkillSource {
    Builtin(&'static str, &'static str, &'static str),
    Path(PathBuf),
}

struct RunArgs {
    skill_source: Option<SkillSource>,
    model: String,
    backend: Backend,
    timeout: Duration,
    verbose: bool,
    help: bool,
    skill_flags: Vec<String>,
}

fn parse_run_argv(argv: Vec<String>) -> RunArgs {
    let mut skill: Option<PathBuf> = None;
    let mut name: Option<String> = None;
    let mut model: Option<String> = None;
    let mut backend_flag: Option<Backend> = None;
    let mut timeout_flag: Option<u64> = None;
    let mut verbose = false;
    let mut help = false;
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
            "--backend" => {
                let value = match argv.get(i + 1) {
                    Some(v) => v.clone(),
                    None => {
                        eprintln!("Error: --backend: missing value");
                        std::process::exit(2);
                    }
                };
                backend_flag = Some(match value.as_str() {
                    "api" => Backend::Api,
                    "claude" => Backend::Claude,
                    other => {
                        eprintln!(
                            "Error: --backend: invalid value '{other}' (expected 'api' or 'claude')"
                        );
                        std::process::exit(2);
                    }
                });
                i += 2;
            }
            "--timeout" => {
                let value = match argv.get(i + 1) {
                    Some(v) => v.clone(),
                    None => {
                        eprintln!("Error: --timeout: missing value");
                        std::process::exit(2);
                    }
                };
                timeout_flag = Some(value.parse::<u64>().unwrap_or_else(|_| {
                    eprintln!("Error: --timeout: invalid integer '{value}'");
                    std::process::exit(2);
                }));
                i += 2;
            }
            "--verbose" => {
                verbose = true;
                i += 1;
            }
            "--help" | "-h" => {
                help = true;
                i += 1;
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

    let skill_source = match (skill, name) {
        (Some(_), Some(_)) => {
            eprintln!("Error: <skill-name> and --skill are mutually exclusive");
            std::process::exit(2);
        }
        (Some(path), None) => Some(SkillSource::Path(path)),
        (None, Some(n)) => {
            if let Err(msg) = validate_skill_name(&n) {
                eprintln!("Error: <skill-name>: {msg}");
                std::process::exit(2);
            }
            if let Some((src, schema, instruction)) = lookup_builtin_skill(&n) {
                Some(SkillSource::Builtin(src, schema, instruction))
            } else {
                let dir = match skill_dir_for_name(&n) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("Error: <skill-name>: {e}");
                        std::process::exit(2);
                    }
                };
                let skill_js = dir.join("skill.js");
                if skill_js.is_file() {
                    Some(SkillSource::Path(skill_js))
                } else {
                    eprintln!("Error: unknown skill: {n}");
                    std::process::exit(2);
                }
            }
        }
        (None, None) => {
            if !help {
                eprintln!("Error: <skill-name> or --skill: required");
                std::process::exit(2);
            }
            None
        }
    };

    let model = model.unwrap_or_else(|| "haiku".to_string());

    RunArgs {
        skill_source,
        model,
        backend: resolve_backend(backend_flag),
        timeout: resolve_timeout(timeout_flag),
        verbose,
        help,
        skill_flags,
    }
}

fn print_run_usage() {
    println!(
        "Usage: forge run [<skill-name> | --skill <path>] [OPTIONS] [-- <skill-flags>]"
    );
    println!();
    println!("Options:");
    println!("  <skill-name>          Built-in or user-installed skill name");
    println!("  --skill <path>        Path to a skill JS file (mutually exclusive with <skill-name>)");
    println!("  --model <model>       Model alias or full ID (default: haiku)");
    println!("  --backend <backend>   Backend: 'api' or 'claude'");
    println!("  --timeout <secs>      Timeout in seconds");
    println!("  --verbose             Print loop-llm tool call logs to stderr (off by default)");
    println!("  -h, --help            Print this help");
}

fn load_skill_sources(
    skill_source: &SkillSource,
) -> Result<(String, String, String, Profile)> {
    match skill_source {
        SkillSource::Builtin(src, schema, instruction) => Ok((
            (*src).to_string(),
            (*schema).to_string(),
            (*instruction).to_string(),
            Profile::Builtin,
        )),
        SkillSource::Path(skill_path) => {
            let src = fs::read_to_string(skill_path).with_context(|| {
                format!("failed to read skill source: {}", skill_path.display())
            })?;
            let schema_path = schema_path_for(skill_path);
            let schema = fs::read_to_string(&schema_path).with_context(|| {
                format!("failed to read schema source: {}", schema_path.display())
            })?;
            let instruction_path = instruction_path_for(skill_path);
            let instruction = match fs::read_to_string(&instruction_path) {
                Ok(s) => s,
                Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "failed to read instruction source: {}: {e}",
                        instruction_path.display()
                    ));
                }
            };
            Ok((src, schema, instruction, Profile::User))
        }
    }
}

fn run_help(engine: &Engine, skill_source: Option<SkillSource>) -> Result<()> {
    print_run_usage();
    let Some(source) = skill_source else {
        return Ok(());
    };

    let (skill_src, schema_src, instruction_src, _) = load_skill_sources(&source)?;
    let envelope = match evaluate_schema_for_mcp(engine, &skill_src, &schema_src, &instruction_src) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error: failed to load schema: {e}");
            std::process::exit(1);
        }
    };
    let input = envelope
        .get("input")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let args_spec = envelope.get("args");

    let lines = format_skill_arguments_lines(&input, args_spec);
    if !lines.is_empty() {
        println!();
        println!("Skill arguments:");
        for line in lines {
            println!("{line}");
        }
    }
    Ok(())
}

fn format_skill_arguments_lines(
    input_schema: &serde_json::Value,
    args_spec: Option<&serde_json::Value>,
) -> Vec<String> {
    let positional_prop = args_spec
        .and_then(|a| a.get("positional"))
        .and_then(|p| p.as_str());

    let Some(properties) = input_schema.get("properties").and_then(|p| p.as_object()) else {
        return Vec::new();
    };
    if properties.is_empty() {
        return Vec::new();
    }

    let required: Vec<&str> = input_schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    struct Entry {
        label: String,
        markers: String,
        description: Option<String>,
        extras: Vec<String>,
    }

    let mut entries: Vec<Entry> = Vec::new();
    for (name, prop) in properties {
        let is_positional = positional_prop == Some(name.as_str());
        let ty = prop.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let is_required = required.iter().any(|r| r == name);
        let req_str = if is_required { "required" } else { "optional" };

        let label = if is_positional {
            format!("<{name}>")
        } else {
            format!("--{} <{ty}>", validator::camel_to_kebab(name))
        };

        let markers = if is_positional {
            format!("(positional, {ty}, {req_str})")
        } else {
            format!("({req_str})")
        };

        let description = prop
            .get("description")
            .and_then(|d| d.as_str())
            .map(|s| s.to_string());

        let mut extras: Vec<String> = Vec::new();
        if let Some(values) = prop.get("enum").and_then(|e| e.as_array()) {
            let formatted = values
                .iter()
                .map(format_help_value)
                .collect::<Vec<_>>()
                .join("|");
            extras.push(format!("[enum: {formatted}]"));
        }
        if let Some(d) = prop.get("default") {
            extras.push(format!("[default: {}]", format_help_value(d)));
        }

        entries.push(Entry {
            label,
            markers,
            description,
            extras,
        });
    }

    let label_width = entries
        .iter()
        .map(|e| e.label.chars().count())
        .max()
        .unwrap_or(0);
    let marker_width = entries
        .iter()
        .map(|e| e.markers.chars().count())
        .max()
        .unwrap_or(0);

    let mut lines = Vec::with_capacity(entries.len());
    for entry in entries {
        let label_pad = label_width - entry.label.chars().count();
        let mut line = format!("  {}{}{}", entry.label, " ".repeat(label_pad + 2), entry.markers);
        if let Some(desc) = entry.description.as_ref() {
            let marker_pad = marker_width - entry.markers.chars().count();
            line.push_str(&" ".repeat(marker_pad + 2));
            line.push_str(desc);
        }
        for extra in &entry.extras {
            line.push(' ');
            line.push_str(extra);
        }
        lines.push(line);
    }
    lines
}

fn format_help_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Null => "null".to_string(),
        other => other.to_string(),
    }
}

fn schema_path_for(skill_path: &PathBuf) -> PathBuf {
    skill_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(PathBuf::new)
        .join("schema.js")
}

fn instruction_path_for(skill_path: &PathBuf) -> PathBuf {
    skill_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(PathBuf::new)
        .join("INSTRUCTION.md")
}

pub(crate) fn evaluate_schema_for_mcp(
    engine: &Engine,
    skill_source: &str,
    schema_source: &str,
    instruction_source: &str,
) -> Result<serde_json::Value> {
    let component = deserialize_runtime_component(engine)?;
    let linker = build_linker(engine)?;
    let (mut store, runtime) = instantiate(
        engine,
        &component,
        &linker,
        skill_source.to_string(),
        schema_source.to_string(),
        instruction_source.to_string(),
        Profile::User,
        None,
        0,
        false,
    )?;
    let schema_result = runtime.call_get_schema(&mut store)?;
    let schema_json = match schema_result {
        Ok(j) => j,
        Err(err) => anyhow::bail!("get-schema failed: [{:?}] {}", err.code, err.message),
    };
    let envelope: serde_json::Value = serde_json::from_str(&schema_json)
        .with_context(|| format!("failed to parse schema JSON: {schema_json}"))?;
    Ok(envelope)
}

pub(crate) fn run_skill_for_mcp(
    engine: &Engine,
    skill_source: &str,
    schema_source: &str,
    instruction_source: &str,
    args_json: &str,
) -> std::result::Result<String, String> {
    let component = deserialize_runtime_component(engine)
        .map_err(|e| format!("runtime component init failed: {e}"))?;
    let linker = build_linker(engine).map_err(|e| format!("linker build failed: {e}"))?;
    let llm_config = mcp_skills_llm_config();
    let (mut store, runtime) = instantiate(
        engine,
        &component,
        &linker,
        skill_source.to_string(),
        schema_source.to_string(),
        instruction_source.to_string(),
        Profile::User,
        llm_config,
        0,
        false,
    )
    .map_err(|e| format!("instantiate failed: {e}"))?;
    let r = runtime
        .call_run(&mut store, args_json, None)
        .map_err(|e| format!("call_run trapped: {e}"))?;
    match r {
        Ok(j) => Ok(j),
        Err(err) => Err(format!("[{:?}] {}", err.code, err.message)),
    }
}

fn mcp_skills_llm_config() -> Option<LlmConfig> {
    let api_key = env::var("ANTHROPIC_API_KEY").unwrap_or_default();
    let backend = resolve_backend(None);
    if matches!(backend, Backend::Api) && api_key.is_empty() {
        return None;
    }
    let model = env::var("SKILL_FORGE_MODEL").unwrap_or_else(|_| "claude-sonnet-4-6".to_string());
    let timeout = resolve_timeout(None);
    Some(LlmConfig {
        model,
        api_key,
        backend,
        timeout,
    })
}

fn run_builtin_skill(
    engine: &Engine,
    skill_source: &str,
    schema_source: &str,
    instruction_source: &str,
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
        instruction_source.to_string(),
        Profile::Builtin,
        llm_config,
        0,
        false,
    )?;
    let started = Instant::now();
    let r = runtime.call_run(&mut store, args_json, None)?;
    log_trace("builtin run()", started);
    Ok(r)
}

fn run_generate(
    engine: &Engine,
    prompt: &str,
    name: &str,
    model: &str,
    force: bool,
    backend: Backend,
    timeout: Duration,
) -> Result<()> {
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

    let api_key = match backend {
        Backend::Api => env::var("ANTHROPIC_API_KEY")
            .context("ANTHROPIC_API_KEY environment variable is required")?,
        Backend::Claude => env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
    };

    let skill_dir = skill_dir_for_name(name)?;
    if skill_dir.exists() && !force {
        eprintln!(
            "Error: skill directory already exists: {} (use --force to overwrite)",
            skill_dir.display()
        );
        std::process::exit(1);
    }

    println!("generating skill code...");
    let model_normalized = normalize_model(model);

    let (code, capabilities, schema, instruction) = match backend {
        Backend::Api => generate_via_api(engine, prompt, &model_normalized, &api_key, timeout)?,
        Backend::Claude => generate_via_claude(prompt, &model_normalized, &api_key, timeout)?,
    };
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

    write_generated_skill(&skill_dir, prompt, &code, &capabilities, &schema, &instruction)?;
    print_generate_summary(&skill_dir, &capabilities, &instruction, name, model);

    Ok(())
}

fn generate_via_api(
    engine: &Engine,
    prompt: &str,
    model: &str,
    api_key: &str,
    timeout: Duration,
) -> Result<(String, Vec<String>, serde_json::Value, String)> {
    let args_json = serde_json::json!({
        "prompt": prompt,
        "model": model,
    })
    .to_string();
    let r = run_builtin_skill(
        engine,
        TOOL_GENERATE_SKILL_CODE_JS,
        SCHEMA_TOOL_GENERATE_SKILL_CODE_JS,
        "",
        &args_json,
        Some(LlmConfig {
            model: model.to_string(),
            api_key: api_key.to_string(),
            backend: Backend::Api,
            timeout,
        }),
    )?;
    let json = match r {
        Ok(j) => j,
        Err(err) => {
            eprintln!("agent error: [{:?}] {}", err.code, err.message);
            std::process::exit(1);
        }
    };
    parse_generated(&json)
}

fn generate_via_claude(
    prompt: &str,
    model: &str,
    api_key: &str,
    timeout: Duration,
) -> Result<(String, Vec<String>, serde_json::Value, String)> {
    let cfg = mcp_config_for_self(api_key)?;
    let cfg_path = write_tmp_mcp_config(&cfg)?;

    let exe =
        env::current_exe().context("failed to determine current executable path for self-spawn")?;
    let exe_str = exe.to_string_lossy().to_string();

    let combined_prompt = format!("{CODEGEN_SYSTEM_PROMPT}\n\n# Task\n\n{prompt}");

    let args = vec![
        "-p".to_string(),
        "--bare".to_string(),
        "--strict-mcp-config".to_string(),
        "--mcp-config".to_string(),
        cfg_path.to_string_lossy().to_string(),
        "--disallowedTools".to_string(),
        "Bash Edit Read".to_string(),
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--verbose".to_string(),
        "--model".to_string(),
        model.to_string(),
        combined_prompt,
    ];

    let stdout = run_with_timeout("claude", &args, timeout)
        .map_err(|e| anyhow::anyhow!("claude backend: {e}"))?;
    let _ = fs::remove_file(&cfg_path);

    let submit = extract_submit_from_stream(&stdout, exe_str.as_str()).ok_or_else(|| {
        anyhow::anyhow!("claude backend: submit_generated_code not found in stream")
    })?;

    parse_submit_input(&submit)
}

fn mcp_config_for_self(api_key: &str) -> Result<String> {
    let exe =
        env::current_exe().context("failed to determine current executable path for self-spawn")?;
    let env_obj = if api_key.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::json!({ "ANTHROPIC_API_KEY": api_key })
    };
    Ok(serde_json::json!({
        "mcpServers": {
            "skill-forge": {
                "command": exe.to_string_lossy(),
                "args": ["mcp-server"],
                "env": env_obj,
            }
        }
    })
    .to_string())
}

fn write_tmp_mcp_config(content: &str) -> Result<PathBuf> {
    let dir = std::env::temp_dir();
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = dir.join(format!("skill-forge-mcp-{pid}-{nanos}.json"));
    fs::write(&path, content)
        .with_context(|| format!("failed to write tmp mcp-config to {}", path.display()))?;
    Ok(path)
}

fn extract_submit_from_stream(stdout: &str, _exe: &str) -> Option<serde_json::Value> {
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v.get("type").and_then(|t| t.as_str()) != Some("assistant") {
            continue;
        }
        let content = match v.pointer("/message/content").and_then(|c| c.as_array()) {
            Some(a) => a,
            None => continue,
        };
        for block in content {
            if block.get("type").and_then(|t| t.as_str()) != Some("tool_use") {
                continue;
            }
            let name = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
            if name.ends_with("submit_generated_code") {
                if let Some(input) = block.get("input") {
                    return Some(input.clone());
                }
            }
        }
    }
    None
}

fn parse_submit_input(
    input: &serde_json::Value,
) -> Result<(String, Vec<String>, serde_json::Value, String)> {
    let code = input
        .get("code")
        .and_then(|c| c.as_str())
        .ok_or_else(|| anyhow::anyhow!("submit_generated_code: missing 'code' string"))?
        .to_string();
    let capabilities = input
        .get("capabilities")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<String>>()
        })
        .ok_or_else(|| anyhow::anyhow!("submit_generated_code: missing 'capabilities' array"))?;
    let schema = input
        .get("schema")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("submit_generated_code: missing 'schema' object"))?;
    let instruction = input
        .get("instruction")
        .and_then(|c| c.as_str())
        .ok_or_else(|| anyhow::anyhow!("submit_generated_code: missing 'instruction' string"))?
        .to_string();
    Ok((code, capabilities, schema, instruction))
}

fn run_with_timeout(
    cmd: &str,
    args: &[String],
    timeout: Duration,
) -> std::result::Result<String, String> {
    use std::process::{Command, Stdio};

    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("spawn {cmd}: {e}"))?;

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = String::new();
                if let Some(mut out) = child.stdout.take() {
                    use std::io::Read;
                    let _ = out.read_to_string(&mut stdout);
                }
                if !status.success() {
                    return Err(format!("{cmd} exited with status {status}",));
                }
                return Ok(stdout);
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "timeout: {cmd} did not finish within {:?}",
                        timeout
                    ));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                return Err(format!("wait {cmd}: {e}"));
            }
        }
    }
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

fn parse_generated(json: &str) -> Result<(String, Vec<String>, serde_json::Value, String)> {
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
    let instruction = v
        .get("instruction")
        .and_then(|c| c.as_str())
        .ok_or_else(|| anyhow::anyhow!("generated output missing 'instruction' string field"))?
        .to_string();
    Ok((code, capabilities, schema, instruction))
}

fn write_generated_skill(
    skill_dir: &Path,
    prompt: &str,
    code: &str,
    capabilities: &[String],
    schema: &serde_json::Value,
    instruction: &str,
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

    if !instruction.is_empty() {
        let instruction_path = skill_dir.join("INSTRUCTION.md");
        fs::write(&instruction_path, instruction)
            .with_context(|| format!("failed to write {}", instruction_path.display()))?;
    }

    Ok(())
}

fn print_generate_summary(
    skill_dir: &Path,
    capabilities: &[String],
    instruction: &str,
    name: &str,
    model: &str,
) {
    println!("wrote {}", skill_dir.join("skill.js").display());
    println!("wrote {}", skill_dir.join("schema.js").display());
    println!("wrote {}", skill_dir.join("PROMPT.md").display());
    if !instruction.is_empty() {
        println!("wrote {}", skill_dir.join("INSTRUCTION.md").display());
    }
    println!("capabilities: {}", capabilities.join(", "));
    println!("run with: forge run {name} --model {model}");
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
    fn mcp_server_default_mode_is_codegen() {
        let parsed = Args::try_parse_from(["forge", "mcp-server"]).unwrap();
        match parsed.command {
            Command::McpServer { mode } => assert_eq!(mode, McpMode::Codegen),
            other => panic!("expected McpServer command, got {other:?}"),
        }
    }

    #[test]
    fn mcp_server_accepts_explicit_codegen_mode() {
        let parsed =
            Args::try_parse_from(["forge", "mcp-server", "--mode", "codegen"]).unwrap();
        match parsed.command {
            Command::McpServer { mode } => assert_eq!(mode, McpMode::Codegen),
            other => panic!("expected McpServer command, got {other:?}"),
        }
    }

    #[test]
    fn mcp_server_accepts_skills_mode() {
        let parsed =
            Args::try_parse_from(["forge", "mcp-server", "--mode", "skills"]).unwrap();
        match parsed.command {
            Command::McpServer { mode } => assert_eq!(mode, McpMode::Skills),
            other => panic!("expected McpServer command, got {other:?}"),
        }
    }

    #[test]
    fn mcp_server_rejects_unknown_mode() {
        let err = Args::try_parse_from(["forge", "mcp-server", "--mode", "bogus"])
            .expect_err("expected parse error");
        assert!(
            err.to_string().contains("bogus") || err.to_string().contains("invalid"),
            "expected error to mention invalid value; got: {err}"
        );
    }

    #[test]
    fn export_defaults_to_claude_code() {
        let parsed = Args::try_parse_from(["forge", "export"]).unwrap();
        match parsed.command {
            Command::Export { target, force } => {
                assert_eq!(target, vec!["claude-code".to_string()]);
                assert!(!force);
            }
            other => panic!("expected Export command, got {other:?}"),
        }
    }

    #[test]
    fn export_accepts_multiple_targets_comma_separated() {
        let parsed =
            Args::try_parse_from(["forge", "export", "--target", "claude-code,cursor"]).unwrap();
        match parsed.command {
            Command::Export { target, force } => {
                assert_eq!(
                    target,
                    vec!["claude-code".to_string(), "cursor".to_string()]
                );
                assert!(!force);
            }
            other => panic!("expected Export command, got {other:?}"),
        }
    }

    #[test]
    fn export_force_flag() {
        let parsed = Args::try_parse_from(["forge", "export", "--force"]).unwrap();
        match parsed.command {
            Command::Export { force, .. } => assert!(force),
            other => panic!("expected Export command, got {other:?}"),
        }
    }

    #[test]
    fn validate_target_name_accepts_alnum_dash_underscore() {
        assert!(validate_target_name("claude-code").is_ok());
        assert!(validate_target_name("cursor").is_ok());
        assert!(validate_target_name("ai_agent_42").is_ok());
    }

    #[test]
    fn validate_target_name_rejects_invalid() {
        assert!(validate_target_name("").is_err());
        assert!(validate_target_name("a/b").is_err());
        assert!(validate_target_name("a b").is_err());
        assert!(validate_target_name("a.b").is_err());
    }

    #[test]
    fn infer_positional_prop_returns_name_when_present() {
        let spec = json!({ "positional": "issueNumber" });
        assert_eq!(
            infer_positional_prop(Some(&spec)),
            Some("issueNumber".to_string())
        );
    }

    #[test]
    fn infer_positional_prop_none_when_missing() {
        assert_eq!(infer_positional_prop(None), None);
        let empty = json!({});
        assert_eq!(infer_positional_prop(Some(&empty)), None);
        let wrong_type = json!({ "positional": 42 });
        assert_eq!(infer_positional_prop(Some(&wrong_type)), None);
    }

    #[test]
    fn collect_export_set_includes_all_builtin_skills() {
        let set = collect_export_set().expect("collect_export_set should succeed");
        let names: Vec<&str> = set.iter().map(|(n, _)| n.as_str()).collect();
        for (name, _, _, _, _) in BUILTIN_SKILLS {
            assert!(names.contains(name), "missing builtin {name} in export set");
        }
        for (_, kind) in &set {
            if matches!(kind, ExportSkillKind::Builtin) {
                continue;
            }
        }
    }

    #[test]
    fn parse_generated_returns_code_capabilities_and_schema() {
        let json = json!({
            "code": "defineSkill(async () => 'ok');",
            "capabilities": ["callLlm"],
            "schema": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            },
            "instruction": ""
        })
        .to_string();
        let (code, caps, schema, instruction) = parse_generated(&json).unwrap();
        assert_eq!(code, "defineSkill(async () => 'ok');");
        assert_eq!(caps, vec!["callLlm".to_string()]);
        assert_eq!(schema.get("type").and_then(|v| v.as_str()), Some("object"));
        assert_eq!(instruction, "");
    }

    #[test]
    fn parse_generated_extracts_instruction() {
        let json = json!({
            "code": "defineSkill(async () => 'ok');",
            "capabilities": [],
            "schema": { "type": "object", "properties": {}, "additionalProperties": false },
            "instruction": "do the thing"
        })
        .to_string();
        let (_, _, _, instruction) = parse_generated(&json).unwrap();
        assert_eq!(instruction, "do the thing");
    }

    #[test]
    fn parse_generated_errors_when_schema_missing() {
        let json = json!({
            "code": "defineSkill(async () => 'ok');",
            "capabilities": [],
            "instruction": ""
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
            "schema": "not-an-object",
            "instruction": ""
        })
        .to_string();
        let err = parse_generated(&json).unwrap_err().to_string();
        assert!(err.contains("not a JSON object"), "unexpected error: {err}");
    }

    #[test]
    fn parse_generated_errors_when_instruction_missing() {
        let json = json!({
            "code": "defineSkill(async () => 'ok');",
            "capabilities": [],
            "schema": { "type": "object", "properties": {}, "additionalProperties": false }
        })
        .to_string();
        let err = parse_generated(&json).unwrap_err().to_string();
        assert!(err.contains("instruction"), "unexpected error: {err}");
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
        write_generated_skill(&tmp, "prompt", "code", &["callLlm".into()], &schema, "").unwrap();
        let schema_js = fs::read_to_string(tmp.join("schema.js")).unwrap();
        assert!(schema_js.starts_with("defineSchema("));
        assert!(schema_js.trim_end().ends_with(");"));
        assert!(schema_js.contains("\"userName\""));
        assert!(schema_js.contains("\"additionalProperties\": false"));
        assert!(!tmp.join("INSTRUCTION.md").exists());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn write_generated_skill_writes_instruction_when_non_empty() {
        let tmp = std::env::temp_dir().join(format!(
            "skill-forge-test-instr-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&tmp);
        let schema = json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        });
        write_generated_skill(&tmp, "prompt", "code", &[], &schema, "do the thing").unwrap();
        let instruction = fs::read_to_string(tmp.join("INSTRUCTION.md")).unwrap();
        assert_eq!(instruction, "do the thing");
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
