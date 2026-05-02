use std::env;
use std::fs;
use std::path::PathBuf;

use wasmtime::{Config, Engine};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let runtime_wasm = manifest_dir.join("runtime/dist/skill-runtime.wasm");
    let agent_wasm = manifest_dir.join("agent/dist/agent-runtime.wasm");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", runtime_wasm.display());
    println!("cargo:rerun-if-changed={}", agent_wasm.display());

    let config = Config::new();
    let engine = Engine::new(&config).expect("failed to create wasmtime Engine for precompile");

    precompile(&engine, &runtime_wasm, &out_dir.join("skill-runtime.cwasm"));
    precompile(&engine, &agent_wasm, &out_dir.join("agent-runtime.cwasm"));
}

fn precompile(engine: &Engine, src: &PathBuf, dst: &PathBuf) {
    let bytes = fs::read(src)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", src.display()));
    let cwasm = engine
        .precompile_component(&bytes)
        .unwrap_or_else(|e| panic!("failed to precompile {}: {e}", src.display()));
    fs::write(dst, &cwasm)
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", dst.display()));
}
