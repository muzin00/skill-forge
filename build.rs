use std::env;
use std::fs;
use std::path::PathBuf;

use wasmtime::{Config, Engine};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let runtime_wasm = manifest_dir.join("runtime/dist/skill-runtime.wasm");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", runtime_wasm.display());
    for skill in [
        "call-llm",
        "generate-skill-code",
        "echo",
        "error",
        "compose",
        "verify-references",
        "read-file",
        "grep-file",
        "loop-llm",
        "implementation-check",
        "view-issue",
        "echo-task",
    ] {
        println!("cargo:rerun-if-changed=agent/dist/skills/{skill}/skill.js");
        println!("cargo:rerun-if-changed=agent/dist/skills/{skill}/schema.js");
        println!(
            "cargo:rerun-if-changed=agent/src/skills/{skill}/DESCRIPTION.md"
        );
        println!(
            "cargo:rerun-if-changed=agent/src/skills/{skill}/INSTRUCTION.md"
        );
    }

    let config = Config::new();
    let engine = Engine::new(&config).expect("failed to create wasmtime Engine for precompile");

    precompile(&engine, &runtime_wasm, &out_dir.join("skill-runtime.cwasm"));
}

fn precompile(engine: &Engine, src: &PathBuf, dst: &PathBuf) {
    let bytes = fs::read(src).unwrap_or_else(|e| panic!("failed to read {}: {e}", src.display()));
    let cwasm = engine
        .precompile_component(&bytes)
        .unwrap_or_else(|e| panic!("failed to precompile {}: {e}", src.display()));
    fs::write(dst, &cwasm).unwrap_or_else(|e| panic!("failed to write {}: {e}", dst.display()));
}
