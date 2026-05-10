use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

fn run_skill(
    name: &str,
    extra: &[&str],
    stdin_input: Option<&str>,
) -> Output {
    let bin = env!("CARGO_BIN_EXE_forge");
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{name}/skill.js"));

    let stdin_cfg = if stdin_input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    };

    let mut child = Command::new(bin)
        .args(["run", "--skill"])
        .arg(&fixture)
        .args(["--model", "claude-haiku-4-5-20251001"])
        .args(extra)
        .env("ANTHROPIC_API_KEY", "dummy-not-used-by-this-test")
        .stdin(stdin_cfg)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn skill-forge");

    if let Some(input) = stdin_input {
        let stdin = child.stdin.as_mut().expect("stdin should be piped");
        stdin
            .write_all(input.as_bytes())
            .expect("failed to write stdin");
    }
    drop(child.stdin.take());

    child.wait_with_output().expect("failed to wait for child")
}

#[test]
fn upstream_writes_validated_json_to_stdout() {
    let out = run_skill("pipe-upstream", &["--seed", "5"], None);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "expected success\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let v: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("output not JSON: {e}\n{stdout}"));
    assert_eq!(v, serde_json::json!({ "value": 10 }));
}

#[test]
fn downstream_reads_input_from_stdin_json() {
    let out = run_skill(
        "pipe-downstream",
        &[],
        Some(r#"{"value": 7}"#),
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "expected success\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(v, serde_json::json!({ "doubled": 14 }));
}

#[test]
fn cli_flag_overrides_stdin_field() {
    let out = run_skill(
        "pipe-downstream",
        &["--value", "100"],
        Some(r#"{"value": 1}"#),
    );
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(v, serde_json::json!({ "doubled": 200 }));
}

#[test]
fn empty_stdin_treated_as_empty_object_with_cli_flags() {
    let out = run_skill("pipe-downstream", &["--value", "3"], Some(""));
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(v, serde_json::json!({ "doubled": 6 }));
}

#[test]
fn stdin_invalid_json_reports_parse_error() {
    let out = run_skill("pipe-downstream", &["--value", "1"], Some("not-json"));
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Error: stdin: invalid JSON"),
        "stderr did not contain expected message:\n{stderr}"
    );
}

#[test]
fn stdin_violates_input_schema_type() {
    let out = run_skill(
        "pipe-downstream",
        &[],
        Some(r#"{"value": "not-a-number"}"#),
    );
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        stderr.trim_end(),
        "Error: --value: expected integer, got \"not-a-number\""
    );
}

#[test]
fn stdin_missing_required_field_after_merge() {
    let out = run_skill("pipe-downstream", &[], Some(r#"{}"#));
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(stderr.trim_end(), "Error: --value: required");
}

#[test]
fn stdin_unknown_property_blocked_when_additional_properties_false() {
    let out = run_skill(
        "pipe-downstream",
        &[],
        Some(r#"{"value": 1, "extra": "x"}"#),
    );
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(stderr.trim_end(), "Error: --extra: unknown flag");
}

#[test]
fn upstream_output_schema_violation_reports_to_stderr() {
    let out = run_skill("pipe-output-violation", &["--seed", "3"], None);
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stderr.contains("output validation: .result: expected string, got 6"),
        "stderr did not contain expected message\nstderr:\n{stderr}\nstdout:\n{stdout}"
    );
    assert!(
        stdout.is_empty(),
        "stdout should be empty on output violation, got:\n{stdout}"
    );
}

#[test]
fn skill_without_output_schema_skips_output_validation() {
    // schema-arg-valid has no output schema; verify it still runs and outputs JSON normally
    let out = run_skill(
        "schema-arg-valid",
        &[
            "--user-name", "alice",
            "--count", "1",
            "--ratio", "1.0",
            "--color", "red",
        ],
        None,
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "expected success\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(v["userName"], serde_json::json!("alice"));
}
