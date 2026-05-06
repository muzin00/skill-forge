mod server;
mod skills;
mod tools;

use wasmtime::Engine;

use crate::McpMode;

pub fn run(mode: McpMode, engine: &Engine) -> anyhow::Result<()> {
    match mode {
        McpMode::Codegen => {
            let mut handler = tools::CodegenHandler;
            server::run(&mut handler)
        }
        McpMode::Skills => {
            let mut handler = skills::SkillsHandler::new(engine)?;
            server::run(&mut handler)
        }
    }
}
