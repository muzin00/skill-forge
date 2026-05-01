use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use wasmtime::component::{Component, Linker, ResourceTable, bindgen};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView};

bindgen!({
    path: "wit",
    world: "skill-runtime",
});

use skill_forge::runtime::skill_loader_host::Host as SkillLoaderHost;
use skill_forge::runtime::types::Host as TypesHost;

const RUNTIME_WASM: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/runtime/dist/skill-runtime.wasm"
);

#[derive(Parser, Debug)]
#[command(name = "skill-forge", about = "PoC #1: skill-runtime host")]
struct Args {
    #[arg(long)]
    skill: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Run {
        #[arg(long, default_value = "{}")]
        args: String,
    },
    ExtractSchemas,
}

struct State {
    ctx: WasiCtx,
    table: ResourceTable,
    skill_source: String,
}

impl WasiView for State {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.ctx
    }
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

impl SkillLoaderHost for State {
    fn get_source(&mut self) -> String {
        self.skill_source.clone()
    }
}

impl TypesHost for State {}

fn main() -> Result<()> {
    let args = Args::parse();

    let source = fs::read_to_string(&args.skill)
        .with_context(|| format!("failed to read skill source: {}", args.skill.display()))?;

    let mut config = Config::new();
    config.cache_config_load_default()?;
    let engine = Engine::new(&config)?;
    let component = Component::from_file(&engine, RUNTIME_WASM)
        .with_context(|| format!("failed to load runtime wasm: {RUNTIME_WASM}"))?;

    let mut linker: Linker<State> = Linker::new(&engine);
    wasmtime_wasi::add_to_linker_sync(&mut linker)?;
    SkillRuntime::add_to_linker(&mut linker, |state| state)?;

    let state = State {
        ctx: WasiCtxBuilder::new().inherit_stdio().build(),
        table: ResourceTable::new(),
        skill_source: source,
    };
    let mut store = Store::new(&engine, state);

    let runtime = SkillRuntime::instantiate(&mut store, &component, &linker)?;

    match args.command {
        Command::Run { args: args_json } => match runtime.call_run(&mut store, &args_json)? {
            Ok(json) => println!("{json}"),
            Err(err) => {
                print_skill_error(&err);
                std::process::exit(1);
            }
        },
        Command::ExtractSchemas => match runtime.call_extract_schemas(&mut store)? {
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

fn print_skill_error(err: &SkillError) {
    eprintln!("[{:?}] {}", err.code, err.message);
    if let Some(stack) = &err.stack {
        eprintln!("stack:\n{stack}");
    }
}
