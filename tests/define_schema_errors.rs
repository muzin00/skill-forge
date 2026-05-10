use std::path::PathBuf;
use std::process::Command;

fn run_fixture(name: &str) -> std::process::Output {
    let bin = env!("CARGO_BIN_EXE_forge");
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{name}/skill.js"));

    Command::new(bin)
        .args(["run", "--skill"])
        .arg(&fixture)
        .args(["--model", "claude-haiku-4-5-20251001"])
        .env("ANTHROPIC_API_KEY", "dummy-not-used-by-this-test")
        .output()
        .expect("failed to run skill-forge")
}

fn assert_failure_with(out: &std::process::Output, needle: &str) {
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !out.status.success(),
        "expected non-zero exit\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains(needle),
        "expected stderr to contain {needle:?}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn schema_without_define_schema_call_reports_specific_error() {
    let out = run_fixture("define-schema-not-called");
    assert_failure_with(
        &out,
        "skill must call defineSchema({ ... }) at top level of schema.js",
    );
}

#[test]
fn define_schema_with_non_object_reports_specific_error() {
    let out = run_fixture("define-schema-not-an-object");
    assert_failure_with(&out, "defineSchema argument must be an object");
}

#[test]
fn define_schema_called_twice_reports_specific_error() {
    let out = run_fixture("define-schema-called-twice");
    assert_failure_with(&out, "defineSchema called more than once");
}

#[test]
fn missing_schema_file_reports_failure() {
    let out = run_fixture("schema-file-missing");
    assert_failure_with(&out, "failed to read schema source");
}
