use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{name}/skill.js"))
}

#[test]
fn stdin_does_not_affect_define_skill_output() {
    let bin = env!("CARGO_BIN_EXE_forge");
    let mut child = Command::new(bin)
        .args(["run", "--skill"])
        .arg(fixture_path("schema-arg-valid"))
        .args([
            "--model",
            "claude-haiku-4-5-20251001",
            "--user-name",
            "alice",
            "--count",
            "2",
            "--ratio",
            "0.5",
            "--color",
            "red",
        ])
        .env("ANTHROPIC_API_KEY", "dummy-not-used-by-this-test")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn forge");
    child
        .stdin
        .as_mut()
        .expect("stdin piped")
        .write_all(b"this stdin content should be ignored by defineSkill")
        .expect("write stdin");
    drop(child.stdin.take());

    let out = child.wait_with_output().expect("wait for child");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "expected success\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let v: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("output not JSON: {e}\n{stdout}"));
    assert_eq!(v["userName"], serde_json::json!("alice"));
}
