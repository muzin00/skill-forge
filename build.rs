use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use wasmtime::{Config, Engine};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    println!("cargo:rerun-if-changed=build.rs");
    for path in [
        "runtime/src/index.ts",
        "runtime/build.mjs",
        "runtime/package.json",
        "runtime/package-lock.json",
        "agent/src",
        "agent/build.mjs",
        "agent/package.json",
        "agent/package-lock.json",
        "wit/skill-runtime.wit",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }

    npm_build(&manifest_dir.join("runtime"));
    npm_build(&manifest_dir.join("agent"));

    let runtime_wasm = manifest_dir.join("runtime/dist/skill-runtime.wasm");
    let config = Config::new();
    let engine = Engine::new(&config).expect("failed to create wasmtime Engine for precompile");

    precompile(&engine, &runtime_wasm, &out_dir.join("skill-runtime.cwasm"));
}

fn npm_build(dir: &Path) {
    if !dir.join("node_modules").exists() {
        run_npm(dir, &["ci"]);
    }
    run_npm(dir, &["run", "build"]);
}

fn run_npm(dir: &Path, args: &[&str]) {
    let status = Command::new("npm")
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap_or_else(|e| {
            panic!("failed to spawn `npm {}` in {}: {e}", args.join(" "), dir.display())
        });
    if !status.success() {
        panic!(
            "`npm {}` in {} exited with {}",
            args.join(" "),
            dir.display(),
            status
        );
    }
}

fn precompile(engine: &Engine, src: &PathBuf, dst: &PathBuf) {
    let bytes = fs::read(src).unwrap_or_else(|e| panic!("failed to read {}: {e}", src.display()));
    let cwasm = engine
        .precompile_component(&bytes)
        .unwrap_or_else(|e| panic!("failed to precompile {}: {e}", src.display()));
    fs::write(dst, &cwasm).unwrap_or_else(|e| panic!("failed to write {}: {e}", dst.display()));
}
