use std::path::PathBuf;
use std::process::Command;

fn run_fixture(name: &str) -> std::process::Output {
    let bin = env!("CARGO_BIN_EXE_skill-forge");
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

fn assert_stderr_contains(out: &std::process::Output, needle: &str) {
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
fn skill_without_define_skill_call_reports_specific_error() {
    let out = run_fixture("define-skill-not-called");
    assert_stderr_contains(
        &out,
        "skill must call defineSkill(async (input) => { ... }) at top level",
    );
}

#[test]
fn define_skill_with_non_function_reports_specific_error() {
    let out = run_fixture("define-skill-not-a-function");
    assert_stderr_contains(&out, "defineSkill argument must be a function");
}

#[test]
fn define_skill_called_twice_reports_specific_error() {
    let out = run_fixture("define-skill-called-twice");
    assert_stderr_contains(&out, "defineSkill called more than once");
}
