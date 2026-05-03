use std::path::PathBuf;
use std::process::Command;

#[test]
fn user_skill_traps_when_calling_anthropic_messages() {
    let bin = env!("CARGO_BIN_EXE_skill-forge");
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/anthropic-trap/skill.js");

    let output = Command::new(bin)
        .args(["run", "--skill"])
        .arg(&fixture)
        .args(["--model", "claude-haiku-4-5-20251001"])
        .env("ANTHROPIC_API_KEY", "dummy-not-used-by-this-test")
        .output()
        .expect("failed to run skill-forge");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !output.status.success(),
        "expected non-zero exit when user skill calls anthropicMessages, got {:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        output.status,
    );
    assert!(
        stderr.contains("capability-denied")
            && stderr.contains("anthropic-host is not available to user skills"),
        "expected capability-denied trap message in stderr; got:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}
