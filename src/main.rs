use std::env;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use wasmtime::component::{Component, Linker, ResourceTable, bindgen};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};

bindgen!({
    path: "wit",
    world: "skill-runtime",
});

mod agent_bindings {
    wasmtime::component::bindgen!({
        path: "agent/wit",
        world: "agent-runtime",
    });
}

use skill_forge::runtime::skill_loader_host::Host as SkillLoaderHost;
use skill_forge::runtime::types::Host as TypesHost;

const RUNTIME_WASM: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/runtime/dist/skill-runtime.wasm"
);
const AGENT_WASM: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/agent/dist/agent-runtime.wasm");

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
    },
    ExtractSchemas {
        #[arg(long)]
        skill: PathBuf,
    },
    Agent {
        #[arg(long)]
        prompt: String,
        #[arg(long)]
        model: String,
    },
    Generate {
        #[arg(long)]
        prompt: String,
        #[arg(long)]
        model: String,
    },
    Interpret {
        #[arg(long)]
        prompt: String,
        #[arg(long)]
        model: String,
    },
}

struct SkillState {
    ctx: WasiCtx,
    table: ResourceTable,
    skill_source: String,
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
    fn get_source(&mut self) -> String {
        self.skill_source.clone()
    }
}

impl TypesHost for SkillState {}

struct AgentState {
    ctx: WasiCtx,
    http_ctx: WasiHttpCtx,
    table: ResourceTable,
}

impl WasiView for AgentState {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.ctx
    }
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

impl WasiHttpView for AgentState {
    fn ctx(&mut self) -> &mut WasiHttpCtx {
        &mut self.http_ctx
    }
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

fn main() -> Result<()> {
    let _ = dotenvy::dotenv();

    let args = Args::parse();

    let mut config = Config::new();
    config.cache_config_load_default()?;
    let engine = Engine::new(&config)?;

    match args.command {
        Command::Run {
            skill,
            args: args_json,
        } => run_skill(&engine, &skill, RunMode::Run(args_json)),
        Command::ExtractSchemas { skill } => run_skill(&engine, &skill, RunMode::ExtractSchemas),
        Command::Agent { prompt, model } => run_agent(&engine, &prompt, &model),
        Command::Generate { prompt, model } => run_generate(&engine, &prompt, &model),
        Command::Interpret { prompt, model } => run_interpret(&engine, &prompt, &model),
    }
}

enum RunMode {
    Run(String),
    ExtractSchemas,
}

fn run_skill(engine: &Engine, skill_path: &PathBuf, mode: RunMode) -> Result<()> {
    let source = fs::read_to_string(skill_path)
        .with_context(|| format!("failed to read skill source: {}", skill_path.display()))?;

    let component = Component::from_file(engine, RUNTIME_WASM)
        .with_context(|| format!("failed to load runtime wasm: {RUNTIME_WASM}"))?;

    let mut linker: Linker<SkillState> = Linker::new(engine);
    wasmtime_wasi::add_to_linker_sync(&mut linker)?;
    SkillRuntime::add_to_linker(&mut linker, |state| state)?;

    let state = SkillState {
        ctx: WasiCtxBuilder::new().inherit_stdio().build(),
        table: ResourceTable::new(),
        skill_source: source,
    };
    let mut store = Store::new(engine, state);

    let runtime = SkillRuntime::instantiate(&mut store, &component, &linker)?;

    match mode {
        RunMode::Run(args_json) => match runtime.call_run(&mut store, &args_json)? {
            Ok(json) => println!("{json}"),
            Err(err) => {
                print_skill_error(&err);
                std::process::exit(1);
            }
        },
        RunMode::ExtractSchemas => match runtime.call_extract_schemas(&mut store)? {
            Ok(schemas) => {
                println!("inputs: {}", schemas.inputs);
                println!("output: {}", schemas.output);
            }
            Err(err) => {
                print_skill_error(&err);
                std::process::exit(1);
            }
        },
    }

    Ok(())
}

fn run_agent(engine: &Engine, prompt: &str, model: &str) -> Result<()> {
    let api_key = env::var("ANTHROPIC_API_KEY")
        .context("ANTHROPIC_API_KEY environment variable is required")?;

    let component = Component::from_file(engine, AGENT_WASM)
        .with_context(|| format!("failed to load agent wasm: {AGENT_WASM}"))?;

    let mut linker: Linker<AgentState> = Linker::new(engine);
    wasmtime_wasi::add_to_linker_sync(&mut linker)?;
    wasmtime_wasi_http::add_only_http_to_linker_sync(&mut linker)?;

    let state = AgentState {
        ctx: WasiCtxBuilder::new().inherit_stdio().build(),
        http_ctx: WasiHttpCtx::new(),
        table: ResourceTable::new(),
    };
    let mut store = Store::new(engine, state);

    let runtime = agent_bindings::AgentRuntime::instantiate(&mut store, &component, &linker)?;

    match runtime.call_llm(&mut store, prompt, model, &api_key)? {
        Ok(text) => println!("{text}"),
        Err(msg) => {
            eprintln!("agent error: {msg}");
            std::process::exit(1);
        }
    }

    Ok(())
}

fn run_generate(engine: &Engine, prompt: &str, model: &str) -> Result<()> {
    let api_key = env::var("ANTHROPIC_API_KEY")
        .context("ANTHROPIC_API_KEY environment variable is required")?;

    let component = Component::from_file(engine, AGENT_WASM)
        .with_context(|| format!("failed to load agent wasm: {AGENT_WASM}"))?;

    let mut linker: Linker<AgentState> = Linker::new(engine);
    wasmtime_wasi::add_to_linker_sync(&mut linker)?;
    wasmtime_wasi_http::add_only_http_to_linker_sync(&mut linker)?;

    let state = AgentState {
        ctx: WasiCtxBuilder::new().inherit_stdio().build(),
        http_ctx: WasiHttpCtx::new(),
        table: ResourceTable::new(),
    };
    let mut store = Store::new(engine, state);

    let runtime = agent_bindings::AgentRuntime::instantiate(&mut store, &component, &linker)?;

    match runtime.call_generate_code(&mut store, prompt, model, &api_key)? {
        Ok(generated) => {
            println!("code:");
            println!("{}", generated.code);
            println!("capabilities: {}", generated.capabilities.join(", "));
        }
        Err(msg) => {
            eprintln!("agent error: {msg}");
            std::process::exit(1);
        }
    }

    Ok(())
}

fn run_interpret(engine: &Engine, prompt: &str, model: &str) -> Result<()> {
    let api_key = env::var("ANTHROPIC_API_KEY")
        .context("ANTHROPIC_API_KEY environment variable is required")?;

    let component = Component::from_file(engine, AGENT_WASM)
        .with_context(|| format!("failed to load agent wasm: {AGENT_WASM}"))?;

    let mut linker: Linker<AgentState> = Linker::new(engine);
    wasmtime_wasi::add_to_linker_sync(&mut linker)?;
    wasmtime_wasi_http::add_only_http_to_linker_sync(&mut linker)?;

    let state = AgentState {
        ctx: WasiCtxBuilder::new().inherit_stdio().build(),
        http_ctx: WasiHttpCtx::new(),
        table: ResourceTable::new(),
    };
    let mut store = Store::new(engine, state);

    let runtime = agent_bindings::AgentRuntime::instantiate(&mut store, &component, &linker)?;

    match runtime.call_interpret(&mut store, prompt, model, &api_key)? {
        Ok(interpreted) => {
            println!("final-answer: {}", interpreted.final_answer);
            println!("signature:");
            print!("[");
            for (i, entry) in interpreted.signature.iter().enumerate() {
                if i > 0 {
                    print!(",");
                }
                println!();
                print!(
                    "  {{\"tool\": {}, \"input\": {}, \"output\": {}}}",
                    json_escape_string(&entry.tool),
                    json_escape_string(&entry.input),
                    json_escape_string(&entry.output)
                );
            }
            if !interpreted.signature.is_empty() {
                println!();
            }
            println!("]");
        }
        Err(msg) => {
            eprintln!("agent error: {msg}");
            std::process::exit(1);
        }
    }

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
