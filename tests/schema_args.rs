use std::path::PathBuf;
use std::process::{Command, Output};

fn run_fixture(name: &str, extra: &[&str]) -> Output {
    let bin = env!("CARGO_BIN_EXE_skill-forge");
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{name}/skill.js"));

    Command::new(bin)
        .args(["run", "--skill"])
        .arg(&fixture)
        .args(["--model", "claude-haiku-4-5-20251001"])
        .args(extra)
        .env("ANTHROPIC_API_KEY", "dummy-not-used-by-this-test")
        .output()
        .expect("failed to run skill-forge")
}

fn assert_validation_error(out: &Output, expected_stderr: &str) {
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit code 2\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let stderr_trimmed = stderr.trim_end();
    assert_eq!(
        stderr_trimmed, expected_stderr,
        "stderr did not match expected\nstdout:\n{stdout}"
    );
}

#[test]
fn schema_arg_valid_happy_path() {
    let out = run_fixture(
        "schema-arg-valid",
        &[
            "--user-name", "alice",
            "--count", "3",
            "--ratio", "1.5",
            "--enabled",
            "--tags", "a", "--tags", "b",
            "--color", "red",
        ],
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "expected zero exit\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let v: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("output not valid JSON: {e}\n{stdout}"));
    assert_eq!(
        v,
        serde_json::json!({
            "userName": "alice",
            "count": 3,
            "ratio": 1.5,
            "enabled": true,
            "tags": ["a", "b"],
            "color": "red"
        })
    );
}

#[test]
fn schema_arg_valid_boolean_unspecified_becomes_false() {
    let out = run_fixture(
        "schema-arg-valid",
        &[
            "--user-name", "alice",
            "--count", "3",
            "--ratio", "1.5",
            "--tags", "a",
            "--color", "green",
        ],
    );
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(v["enabled"], serde_json::Value::Bool(false));
    assert!(v.get("optionalNote").is_none(), "default must not be applied by host");
}

#[test]
fn schema_type_mismatch_emits_typed_error() {
    let out = run_fixture("schema-type-mismatch", &["--count", "abc"]);
    assert_validation_error(&out, "Error: --count: expected integer, got \"abc\"");
}

#[test]
fn schema_required_missing_emits_required_error() {
    let out = run_fixture("schema-required-missing", &[]);
    assert_validation_error(&out, "Error: --user-name: required");
}

#[test]
fn schema_enum_violation_emits_enum_error() {
    let out = run_fixture("schema-enum-violation", &["--color", "purple"]);
    assert_validation_error(
        &out,
        "Error: --color: must be one of [\"red\", \"green\", \"blue\"], got \"purple\"",
    );
}

#[test]
fn schema_unknown_flag_emits_unknown_error() {
    let out = run_fixture("schema-unknown-flag", &["--mystery", "foo"]);
    assert_validation_error(&out, "Error: --mystery: unknown flag");
}

#[test]
fn legacy_args_flag_is_rejected() {
    let out = run_fixture("schema-arg-valid", &["--args", "{}"]);
    assert_validation_error(&out, "Error: --args: unknown flag");
}
